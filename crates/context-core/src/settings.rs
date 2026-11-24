//! Persistent settings storage in `config.toml`.
//!
//! `StoredSettings` is the file shape: every field is optional, so an absent key
//! means "use the built-in default" and a per-field reset is a key removal.
//! `Settings` is the resolved struct callers work with, and [`resolve`] fills
//! every gap. [`load`] and [`write()`] are plain serde + toml file I/O: no platform
//! APIs, no locking, no async, and no opinion on which process owns the file.

use std::net::IpAddr;
use std::path::{Path, PathBuf};

use miette::Diagnostic;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::paths::get_data_dir;

#[cfg(debug_assertions)]
const DEFAULT_API_PORT: u16 = 3738;

#[cfg(not(debug_assertions))]
const DEFAULT_API_PORT: u16 = 3737;

/// Highest accepted logging verbosity (0=warn, 1=info, 2=debug, 3=trace).
const MAX_VERBOSITY: u8 = 3;

/// Update channels a stored value may name.
const ACCEPTED_UPDATE_CHANNELS: [&str; 2] = ["stable", "pre-release"];

// region:    --- Types

/// Settings as stored in `config.toml`.
///
/// Every field is optional. `None` means the field is absent from the file and
/// the built-in default applies.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StoredSettings {
    /// Host address to bind to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<IpAddr>,
    /// Port to listen on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Logging verbosity (0=warn, 1=info, 2=debug, 3=trace).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<u8>,
    /// Serve the OpenAPI documentation endpoint at /docs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_docs: Option<bool>,
    /// Skills cache directory (where attachments are extracted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills_dir: Option<PathBuf>,
    /// Start the app at login.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_at_login: Option<bool>,
    /// Check for updates when the app starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_update: Option<bool>,
    /// Update channel: "stable" or "pre-release".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_channel: Option<String>,
}

/// Fully resolved settings: every field carries a concrete value.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Host address to bind to.
    pub host: IpAddr,
    /// Port to listen on.
    pub port: u16,
    /// Logging verbosity (0=warn, 1=info, 2=debug, 3=trace).
    pub verbosity: u8,
    /// Serve the OpenAPI documentation endpoint at /docs.
    pub enable_docs: bool,
    /// Skills cache directory (where attachments are extracted).
    pub skills_dir: PathBuf,
    /// Start the app at login.
    pub start_at_login: bool,
    /// Check for updates when the app starts.
    pub auto_update: bool,
    /// Update channel: "stable" or "pre-release".
    pub update_channel: String,
}

/// Settings storage errors.
#[derive(Error, Diagnostic, Debug)]
pub enum SettingsError {
    #[error("Failed to parse settings file '{path}': {source}")]
    #[diagnostic(code(context::settings::parse))]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },

    #[error("Failed to serialize settings: {source}")]
    #[diagnostic(code(context::settings::serialize))]
    Serialize {
        #[source]
        source: toml::ser::Error,
    },

    #[error("Failed to access settings file '{path}': {source}")]
    #[diagnostic(code(context::settings::io))]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Invalid settings: {message}")]
    #[diagnostic(code(context::settings::validation))]
    Validation { message: String },
}

/// Result type for settings operations.
pub type SettingsResult<T> = Result<T, SettingsError>;

// endregion: --- Types

// region:    --- Functions

/// Read the settings file at `path`.
///
/// A missing file is not an error: it resolves to `StoredSettings::default()`,
/// which carries no overrides.
pub fn load(path: &Path) -> SettingsResult<StoredSettings> {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(StoredSettings::default());
        }
        Err(source) => {
            return Err(SettingsError::Io {
                path: path.display().to_string(),
                source,
            });
        }
    };

    toml::from_str(&source).map_err(|source| SettingsError::Parse {
        path: path.display().to_string(),
        source,
    })
}

/// Write `stored` to the settings file at `path`.
///
/// Creates the parent directory when it does not exist. Callers validate with
/// [`validate_stored`] first when they must not create an invalid file.
pub fn write(path: &Path, stored: &StoredSettings) -> SettingsResult<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|source| SettingsError::Io {
            path: parent.display().to_string(),
            source,
        })?;
    }

    let source = toml::to_string(stored).map_err(|source| SettingsError::Serialize { source })?;

    std::fs::write(path, source).map_err(|source| SettingsError::Io {
        path: path.display().to_string(),
        source,
    })
}

/// Reject stored values that fall outside the accepted range.
///
/// A `None` field carries no override and always passes.
pub fn validate_stored(stored: &StoredSettings) -> SettingsResult<()> {
    if stored.port == Some(0) {
        return Err(SettingsError::Validation {
            message: "port must be between 1 and 65535, got 0".to_string(),
        });
    }

    if let Some(verbosity) = stored.verbosity
        && verbosity > MAX_VERBOSITY
    {
        return Err(SettingsError::Validation {
            message: format!("verbosity must be between 0 and {MAX_VERBOSITY}, got {verbosity}"),
        });
    }

    if let Some(channel) = stored.update_channel.as_deref()
        && !ACCEPTED_UPDATE_CHANNELS.contains(&channel)
    {
        return Err(SettingsError::Validation {
            message: format!(
                "update_channel must be \"stable\" or \"pre-release\", got '{channel}'"
            ),
        });
    }

    Ok(())
}

/// Fill every absent field with its built-in default.
pub fn resolve(stored: &StoredSettings) -> Settings {
    let defaults = Settings::default();

    Settings {
        host: stored.host.unwrap_or(defaults.host),
        port: stored.port.unwrap_or(defaults.port),
        verbosity: stored.verbosity.unwrap_or(defaults.verbosity),
        enable_docs: stored.enable_docs.unwrap_or(defaults.enable_docs),
        skills_dir: stored.skills_dir.clone().unwrap_or(defaults.skills_dir),
        start_at_login: stored.start_at_login.unwrap_or(defaults.start_at_login),
        auto_update: stored.auto_update.unwrap_or(defaults.auto_update),
        update_channel: stored
            .update_channel
            .clone()
            .unwrap_or(defaults.update_channel),
    }
}

// endregion: --- Functions

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: IpAddr::from([0, 0, 0, 0]),
            port: DEFAULT_API_PORT,
            verbosity: 0,
            enable_docs: false,
            skills_dir: get_data_dir().join("skills"),
            start_at_login: true,
            auto_update: true,
            update_channel: "stable".to_string(),
        }
    }
}
