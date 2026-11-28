//! The consent install sequence: download, verify, swap.
//!
//! The updater plugin downloads the signed artifact and swaps the bundle;
//! everything around it — the guards and the phase transitions — lives here, so
//! a fake executor drives the whole sequence in tests. The sequence ends at
//! `ready`: the bundle is staged on disk and this process keeps serving it. The
//! restart that runs it is [`super::ServiceUpdateFlow::restart`].

use std::path::Path;
use std::process::{Command, Stdio};

use chrono::Utc;
use context_core::update::{UpdateState, UpdateStatus, load as load_state};
use tauri_plugin_updater::{Error, Update, UpdaterExt};

use super::releases::manifest_url;
use super::state_with;

/// Why an install request did not go through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InstallError {
    /// The state says a check, download or install already runs.
    Busy,
    /// The state offers no candidate to install.
    NotAvailable,
    /// The sequence failed; the state records the reason.
    Failed(String),
}

/// Executor that downloads and swaps the bundle of a release.
pub(crate) trait BundleExecutor: Send + Sync {
    /// Install the release behind `manifest_url`.
    ///
    /// `on_phase` reports `downloading` once the transfer starts and `swapping`
    /// once the download is verified and the bundle is being replaced.
    fn swap(&self, manifest_url: &str, on_phase: impl Fn(&str) + Send + Sync)
    -> Result<(), String>;
}

/// Executor backed by the updater plugin registered in `app`.
pub(crate) struct TauriExecutor {
    app: tauri::AppHandle,
}

impl TauriExecutor {
    /// Executor that drives the updater plugin of `app`.
    pub(crate) fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl BundleExecutor for TauriExecutor {
    fn swap(
        &self,
        manifest_url: &str,
        on_phase: impl Fn(&str) + Send + Sync,
    ) -> Result<(), String> {
        let app = self.app.clone();

        // The plugin API is async; the sequence runs on a blocking task, so this
        // wait cannot stall the async runtime.
        tauri::async_runtime::block_on(async move {
            let endpoint = tauri::Url::parse(manifest_url).map_err(|error| {
                format!("Invalid update manifest URL '{manifest_url}': {error}")
            })?;

            let updater = app
                .updater_builder()
                .endpoints(vec![endpoint])
                .map_err(|error| format!("Failed to configure the updater: {error}"))?
                .build()
                .map_err(|error| format!("Failed to build the updater: {error}"))?;

            let update = check_outcome(updater.check().await)?;

            on_phase("downloading");

            update
                .download_and_install(|_chunk, _total| {}, || on_phase("swapping"))
                .await
                .map_err(|error| format!("Failed to download and install the update: {error}"))
        })
    }
}

/// Classify a check outcome into the update to install, or the message that says
/// why this run installs nothing.
///
/// The plugin answers two variants for three conditions: `Ok(None)` means the
/// manifest version is not greater than the running version, while a manifest
/// with no artifact for this platform arrives as `Err(TargetsNotFound)`. Every
/// other failure stays under the manifest wording. Each condition gets its own
/// text, so a drift between the plugin and the release selection names its real
/// cause instead of blaming another one.
pub(crate) fn check_outcome(outcome: Result<Option<Update>, Error>) -> Result<Update, String> {
    match outcome {
        Ok(Some(update)) => Ok(update),
        Ok(None) => Err(no_newer_release_message()),
        Err(Error::TargetsNotFound(targets)) => Err(missing_platform_message(&targets)),
        Err(Error::TargetNotFound(target)) => {
            Err(missing_platform_message(std::slice::from_ref(&target)))
        }
        Err(error) => Err(format!("Failed to read the update manifest: {error}")),
    }
}

/// Message for a check that found no release newer than the running version.
///
/// An older or equal manifest version is the only thing `Ok(None)` reports, so
/// the text names the version comparison and leaves the platform out of it.
fn no_newer_release_message() -> String {
    "The manifest offers no release newer than the running version".to_string()
}

/// Message for a manifest that carries no artifact for this platform.
///
/// `keys` are the target keys the plugin searched, in its own order, so an
/// operator reads which entry the manifest lacks instead of guessing at it.
fn missing_platform_message(keys: &[String]) -> String {
    let keys: Vec<String> = keys.iter().map(|key| format!("'{key}'")).collect();

    format!(
        "The release carries no artifact for this platform: the manifest has no entry for {}",
        keys.join(" or ")
    )
}

/// Manifest the state offers to install, or why it offers none.
pub(crate) fn install_target(state: &UpdateState) -> Result<String, InstallError> {
    match state.status {
        UpdateStatus::Available => state
            .candidate_tag
            .as_deref()
            .map(manifest_url)
            .ok_or(InstallError::NotAvailable),
        UpdateStatus::Checking | UpdateStatus::Downloading | UpdateStatus::Installing => {
            Err(InstallError::Busy)
        }
        UpdateStatus::Idle | UpdateStatus::Ready | UpdateStatus::Failed => {
            Err(InstallError::NotAvailable)
        }
    }
}

/// Run the install sequence for the candidate the state offers.
///
/// `publish` records each phase — the flow writes the file and broadcasts it.
/// The sequence ends at `ready`: nothing spawns the swapped service and nothing
/// exits this process, because the running binary keeps serving the staged
/// bundle from its open inode. Nothing runs while the state offers no candidate.
pub(crate) fn install<E, P>(executor: &E, state_path: &Path, publish: P) -> Result<(), InstallError>
where
    E: BundleExecutor,
    P: Fn(UpdateState) -> Result<(), String> + Sync,
{
    let state = load_state(state_path).map_err(|error| InstallError::Failed(error.to_string()))?;
    let manifest = install_target(&state)?;

    publish(state_with(UpdateStatus::Downloading)).map_err(InstallError::Failed)?;

    let on_phase = |phase: &str| {
        let status = match phase {
            "swapping" => UpdateStatus::Installing,
            _ => UpdateStatus::Downloading,
        };

        if let Err(error) = publish(state_with(status)) {
            eprintln!("context: failed to record the update phase: {error}");
        }
    };

    if let Err(message) = executor.swap(&manifest, on_phase) {
        record_failure(&publish, &message);
        return Err(InstallError::Failed(message));
    }

    publish(ready_state(&state)).map_err(InstallError::Failed)?;

    Ok(())
}

/// Command that starts the service again after the swap.
///
/// The new service binary sits inside the swapped bundle, and the old process
/// still holds the single-instance socket, so the shell sleeps a second before
/// it execs the binary. Spawn it and do not wait: the old process exits while
/// the child is still sleeping.
pub(crate) fn restart_command(service_path: &Path) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(format!(
            "sleep 1; exec {}",
            shell_quote(&service_path.display().to_string())
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    command
}

/// Quote `value` for the shell: single quotes, embedded quotes escaped.
///
/// The POSIX form for a literal single quote inside single quotes: close the
/// quoted run, emit an escaped quote, and reopen it.
pub(crate) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// State a finished swap records.
///
/// The candidate fields stay, so the surfaces can name what was installed, and
/// `current_version` becomes the candidate: the next process runs it, which is
/// what lets the startup reconciliation settle the state.
fn ready_state(state: &UpdateState) -> UpdateState {
    let installed = state
        .candidate_version
        .clone()
        .unwrap_or_else(|| state.current_version.clone());

    UpdateState {
        status: UpdateStatus::Ready,
        current_version: installed,
        updated_at: Some(Utc::now().to_rfc3339()),
        ..state.clone()
    }
}

/// Record a failure the operator can read while the old bundle keeps serving.
fn record_failure(publish: &impl Fn(UpdateState) -> Result<(), String>, message: &str) {
    let state = UpdateState {
        message: Some(message.to_string()),
        ..state_with(UpdateStatus::Failed)
    };

    if let Err(error) = publish(state) {
        eprintln!("context: failed to record the update failure: {error}");
    }
}
