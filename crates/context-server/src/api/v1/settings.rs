//! Settings endpoints.
//!
//! `GET /api/v1/settings` reports three field sets: `values` is the file on
//! disk, `defaults` is the built-in `Settings`, and `running` is the config this
//! server process started with. `PUT /api/v1/settings` replaces the file: it
//! validates the body first, so a rejected body leaves the file untouched.
//!
//! Neither handler applies values to the running server. A client restarts the
//! server to pick up the stored values; comparing `values` with `running` is how
//! it knows a restart would change something.

use std::net::IpAddr;
use std::path::{Path, PathBuf};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use context_core::Database;
use context_core::get_config_path;
use context_core::settings::{self, Settings, SettingsError, StoredSettings};
use context_sync::GitOps;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use utoipa::ToSchema;

use crate::api::{AppState, RunningSettings};

use super::ErrorResponse;

// =============================================================================
// DTOs
// =============================================================================

/// Stored values: the file shape, also the PUT request body.
///
/// A field left out of the body carries no override: the key is absent from the
/// file and the built-in default applies. PUT replaces the whole file, so an
/// empty body resets every field.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct SettingsValues {
    /// Host address to bind to, as an IP address string.
    #[schema(example = "127.0.0.1")]
    pub host: Option<String>,
    /// Port to listen on.
    #[schema(example = 4040)]
    pub port: Option<u16>,
    /// Logging verbosity (0=warn, 1=info, 2=debug, 3=trace).
    #[schema(example = 2)]
    pub verbosity: Option<u8>,
    /// Serve the OpenAPI documentation endpoint at /docs.
    #[schema(example = true)]
    pub enable_docs: Option<bool>,
    /// Skills cache directory (where attachments are extracted).
    #[schema(example = "/home/user/.local/share/c5t/skills")]
    pub skills_dir: Option<String>,
    /// Start the app at login.
    #[schema(example = true)]
    pub start_at_login: Option<bool>,
}

/// Built-in defaults: every field carries a concrete value.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingsSnapshot {
    /// Host address the default config binds to.
    #[schema(example = "0.0.0.0")]
    pub host: String,
    /// Port the default config listens on.
    #[schema(example = 3738)]
    pub port: u16,
    /// Logging verbosity of the default config.
    #[schema(example = 0)]
    pub verbosity: u8,
    /// Whether the default config serves the OpenAPI documentation endpoint.
    #[schema(example = false)]
    pub enable_docs: bool,
    /// Skills cache directory of the default config.
    pub skills_dir: String,
    /// Whether the app starts at login by default.
    #[schema(example = true)]
    pub start_at_login: bool,
}

/// Settings the running server process started with.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunningSettingsResponse {
    /// Host address this process bound to.
    #[schema(example = "0.0.0.0")]
    pub host: String,
    /// Port this process bound to.
    #[schema(example = 3738)]
    pub port: u16,
    /// Logging verbosity of this process.
    #[schema(example = 0)]
    pub verbosity: u8,
    /// Whether this process serves the OpenAPI documentation endpoint.
    #[schema(example = false)]
    pub enable_docs: bool,
    /// Skills cache directory this process serves from.
    pub skills_dir: String,
}

/// Response of both settings endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingsResponse {
    /// Values stored in the settings file.
    pub values: SettingsValues,
    /// Built-in defaults for every field.
    pub defaults: SettingsSnapshot,
    /// Settings this server process started with.
    pub running: RunningSettingsResponse,
}

// =============================================================================
// Conversions
// =============================================================================

impl From<&StoredSettings> for SettingsValues {
    fn from(stored: &StoredSettings) -> Self {
        Self {
            host: stored.host.map(|host| host.to_string()),
            port: stored.port,
            verbosity: stored.verbosity,
            enable_docs: stored.enable_docs,
            skills_dir: stored
                .skills_dir
                .as_ref()
                .map(|path| path.display().to_string()),
            start_at_login: stored.start_at_login,
        }
    }
}

impl From<&Settings> for SettingsSnapshot {
    fn from(settings: &Settings) -> Self {
        Self {
            host: settings.host.to_string(),
            port: settings.port,
            verbosity: settings.verbosity,
            enable_docs: settings.enable_docs,
            skills_dir: settings.skills_dir.display().to_string(),
            start_at_login: settings.start_at_login,
        }
    }
}

impl RunningSettingsResponse {
    /// Running settings together with the skills directory the state serves.
    fn from_running(running: &RunningSettings, skills_dir: &Path) -> Self {
        Self {
            host: running.host.to_string(),
            port: running.port,
            verbosity: running.verbosity,
            enable_docs: running.enable_docs,
            skills_dir: skills_dir.display().to_string(),
        }
    }
}

impl SettingsValues {
    /// Stored settings these values describe.
    ///
    /// A host that is not an IP address names the field, so the caller can
    /// answer with a 400 before anything reaches the file.
    fn into_stored(self) -> Result<StoredSettings, SettingsError> {
        let host = self
            .host
            .map(|host| {
                host.trim()
                    .parse::<IpAddr>()
                    .map_err(|_| SettingsError::Validation {
                        message: format!("host must be an IP address, got '{host}'"),
                    })
            })
            .transpose()?;

        Ok(StoredSettings {
            host,
            port: self.port,
            verbosity: self.verbosity,
            enable_docs: self.enable_docs,
            skills_dir: self.skills_dir.map(PathBuf::from),
            start_at_login: self.start_at_login,
        })
    }
}

// =============================================================================
// Handlers
// =============================================================================

/// Report the stored values, the built-in defaults and the running settings.
#[utoipa::path(
    get,
    path = "/api/v1/settings",
    tag = "settings",
    responses(
        (status = 200, description = "Stored values, built-in defaults, and the running configuration", body = SettingsResponse),
        (status = 500, description = "Settings file could not be read", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn get_settings<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
) -> Result<Json<SettingsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let stored = settings::load(&get_config_path()).map_err(internal_error)?;

    Ok(Json(settings_response(
        &stored,
        state.running(),
        state.skills_dir(),
    )))
}

/// Validate the body, write the settings file, and report the updated triple.
#[utoipa::path(
    put,
    path = "/api/v1/settings",
    tag = "settings",
    request_body = SettingsValues,
    responses(
        (status = 200, description = "Values written; the response reflects the file", body = SettingsResponse),
        (status = 400, description = "Invalid values; the file is unchanged", body = ErrorResponse),
        (status = 500, description = "Settings file could not be written", body = ErrorResponse)
    )
)]
#[instrument(skip(state))]
pub async fn put_settings<D: Database, G: GitOps + Send + Sync>(
    State(state): State<AppState<D, G>>,
    Json(values): Json<SettingsValues>,
) -> Result<Json<SettingsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let stored = values.into_stored().map_err(bad_request)?;
    settings::validate_stored(&stored).map_err(bad_request)?;

    settings::write(&get_config_path(), &stored).map_err(internal_error)?;

    Ok(Json(settings_response(
        &stored,
        state.running(),
        state.skills_dir(),
    )))
}

// =============================================================================
// Support
// =============================================================================

/// The triple both handlers answer with.
fn settings_response(
    stored: &StoredSettings,
    running: &RunningSettings,
    skills_dir: &Path,
) -> SettingsResponse {
    SettingsResponse {
        values: SettingsValues::from(stored),
        defaults: SettingsSnapshot::from(&Settings::default()),
        running: RunningSettingsResponse::from_running(running, skills_dir),
    }
}

/// Client error: the message names the offending field.
fn bad_request(error: SettingsError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

/// Server error: the settings file could not be read or written.
fn internal_error(error: SettingsError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}
