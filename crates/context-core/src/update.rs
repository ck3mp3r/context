//! Persisted update state and the update-flow seam.
//!
//! The service process owns the update flow, so progress survives a closed GUI.
//! [`UpdateState`] is the file shape written to `update.json` in the profile
//! config directory: plain serde JSON, no platform APIs, no locking, and no
//! opinion on which process reads it. [`UpdateFlow`] is the seam the REST layer
//! talks to, so no Tauri type ever reaches `context-server`.

use std::path::{Path, PathBuf};

use miette::Diagnostic;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::paths::get_config_dir;

/// Name of the update state file inside the configuration directory.
const UPDATE_STATE_FILE_NAME: &str = "update.json";

// region:    --- Types

/// Phase of the update flow.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateStatus {
    /// No check runs and no candidate is known.
    #[default]
    Idle,
    /// A check is in flight.
    Checking,
    /// A candidate is known and waits for consent.
    Available,
    /// The candidate downloads.
    Downloading,
    /// The candidate installs.
    Installing,
    /// The candidate is installed; the process restarts on the new binary.
    Ready,
    /// The last attempt failed; `message` says why.
    Failed,
}

/// Update progress as stored in `update.json`.
///
/// Every optional field is absent from the file when it carries no value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateState {
    /// Phase of the update flow.
    #[serde(default)]
    pub status: UpdateStatus,
    /// Version this build runs.
    pub current_version: String,
    /// Version of the candidate awaiting consent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_version: Option<String>,
    /// Release tag the candidate came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_tag: Option<String>,
    /// Release notes of the candidate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_notes: Option<String>,
    /// Whether the candidate is a pre-release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_is_prerelease: Option<bool>,
    /// Human-readable detail for the current status, such as a failure reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// RFC 3339 timestamp of the last state change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Update state storage errors.
#[derive(Error, Diagnostic, Debug)]
pub enum UpdateStateError {
    #[error("No update flow is attached")]
    #[diagnostic(code(context::update_state::unavailable))]
    Unavailable,

    #[error("An update check or install is already running")]
    #[diagnostic(code(context::update_state::busy))]
    Busy,

    #[error("Invalid update state: {message}")]
    #[diagnostic(code(context::update_state::invalid))]
    Invalid { message: String },

    #[error("Failed to access update state: {message}")]
    #[diagnostic(code(context::update_state::io))]
    Io { message: String },
}

/// Result type for update state operations.
pub type UpdateStateResult<T> = Result<T, UpdateStateError>;

/// Seam between the REST layer and the process that runs the update flow.
///
/// Both methods start work on the flow's own task and return immediately; the
/// flow reports progress through the persisted [`UpdateState`].
pub trait UpdateFlow: Send + Sync {
    /// Start a check; `manual` marks a check the user asked for.
    fn check(&self, manual: bool) -> UpdateStateResult<()>;

    /// Start installing the candidate that waits for consent.
    fn install(&self) -> UpdateStateResult<()>;
}

/// Flow attached when the process has no updater, such as a CLI-only build.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoUpdateFlow;

// endregion: --- Types

// region:    --- Functions

/// Read the update state at `path`.
///
/// A missing file is not an error: it resolves to `UpdateState::default()`.
pub fn load(path: &Path) -> UpdateStateResult<UpdateState> {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(UpdateState::default());
        }
        Err(error) => {
            return Err(UpdateStateError::Io {
                message: format!(
                    "Failed to read update state file '{}': {error}",
                    path.display()
                ),
            });
        }
    };

    serde_json::from_str(&source).map_err(|error| UpdateStateError::Invalid {
        message: format!(
            "Failed to parse update state file '{}': {error}",
            path.display()
        ),
    })
}

/// Write `state` to the update state file at `path`.
///
/// Creates the parent directory when it does not exist.
pub fn write(path: &Path, state: &UpdateState) -> UpdateStateResult<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| UpdateStateError::Io {
            message: format!(
                "Failed to create update state directory '{}': {error}",
                parent.display()
            ),
        })?;
    }

    let source = serde_json::to_string(state).map_err(|error| UpdateStateError::Invalid {
        message: format!("Failed to serialize update state: {error}"),
    })?;

    std::fs::write(path, source).map_err(|error| UpdateStateError::Io {
        message: format!(
            "Failed to write update state file '{}': {error}",
            path.display()
        ),
    })
}

/// Get the update state file path (config_dir/update.json).
///
/// # Returns
/// Path to the state file: `{config_dir}/update.json`
pub fn update_state_path() -> PathBuf {
    get_config_dir().join(UPDATE_STATE_FILE_NAME)
}

// endregion: --- Functions

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            status: UpdateStatus::Idle,
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            candidate_version: None,
            candidate_tag: None,
            candidate_notes: None,
            candidate_is_prerelease: None,
            message: None,
            updated_at: None,
        }
    }
}

impl UpdateFlow for NoUpdateFlow {
    fn check(&self, _manual: bool) -> UpdateStateResult<()> {
        Err(UpdateStateError::Unavailable)
    }

    fn install(&self) -> UpdateStateResult<()> {
        Err(UpdateStateError::Unavailable)
    }
}
