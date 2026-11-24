//! Update machinery shared by the service and the GUI.
//!
//! [`releases`] is the pure GitHub release lookup: the updater plugin has no
//! channel logic, so the winning release is chosen here. [`ServiceUpdateFlow`]
//! is the flow the service attaches to the REST layer: a check runs on the
//! flow's own task and records every phase in the persisted update state, which
//! the server serves and the websocket broadcasts.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use context_core::settings::Settings;
use context_core::update::{
    UpdateFlow, UpdateState, UpdateStateError, UpdateStateResult, UpdateStatus, load as load_state,
    update_state_path, write as write_state,
};
use context_server::api::notifier::{ChangeNotifier, UpdateMessage};

use self::releases::Candidate;

pub(crate) mod install;
pub mod releases;

#[cfg(test)]
#[path = "../../test/update/install.rs"]
mod install_test;
#[cfg(test)]
#[path = "../../test/update/mod.rs"]
mod mod_test;
#[cfg(test)]
#[path = "../../test/update/releases.rs"]
mod releases_test;

/// How long the GitHub request may take before the check reports a failure.
const GITHUB_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// The service's update flow.
///
/// [`UpdateFlow::check`] records `checking`, returns, and the flow's own task
/// writes the outcome and broadcasts it. Clones share one busy flag, so a second
/// check or install is refused while one runs.
///
/// The check gate reads the settings source the constructor was handed, so the
/// decision is injected rather than read from the process-global configuration.
#[derive(Clone)]
pub struct ServiceUpdateFlow {
    app: tauri::AppHandle,
    notifier: ChangeNotifier,
    busy: Arc<Mutex<bool>>,
    gate: CheckGate<fn() -> Settings>,
}

/// What a check trigger does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CheckDecision {
    /// Claim the flow, record `checking`, and run the check on this channel.
    Run { channel: String },
    /// Decline without claiming: a decline is not contention.
    Decline,
}

/// The check gate: decides from its settings source whether a check runs.
///
/// The gate holds no state and touches no global, so a test drives it directly.
#[derive(Clone)]
pub(crate) struct CheckGate<P> {
    settings: P,
}

impl<P> CheckGate<P>
where
    P: Fn() -> Settings,
{
    /// Gate that reads its settings from `settings`.
    pub(crate) fn new(settings: P) -> Self {
        Self { settings }
    }

    /// What the trigger for `manual` does next.
    pub(crate) fn decide(&self, manual: bool) -> CheckDecision {
        let settings = (self.settings)();

        if should_check(manual, settings.auto_update) {
            CheckDecision::Run {
                channel: settings.update_channel,
            }
        } else {
            CheckDecision::Decline
        }
    }
}

impl ServiceUpdateFlow {
    /// Flow that drives `app`'s updater plugin and broadcasts on `notifier`.
    ///
    /// `settings` is the check gate's source: the service passes its resolved
    /// settings reader, so the gate never reaches for the configuration itself.
    pub fn new(
        app: tauri::AppHandle,
        notifier: ChangeNotifier,
        settings: fn() -> Settings,
    ) -> Self {
        Self {
            app,
            notifier,
            busy: Arc::new(Mutex::new(false)),
            gate: CheckGate::new(settings),
        }
    }

    /// Handle of the app the flow drives the updater plugin through.
    pub fn app(&self) -> &tauri::AppHandle {
        &self.app
    }

    /// Reset a finished install to idle.
    ///
    /// The service calls this before it starts serving: when the state still says
    /// the previous process was installing the version this process runs, that
    /// install is done and the state starts clean.
    pub fn reconcile_startup(&self) {
        let state = match load_state(&update_state_path()) {
            Ok(state) => state,
            Err(error) => {
                eprintln!("context: failed to read the update state: {error}");
                return;
            }
        };

        if !settled_after_install(&state) {
            return;
        }

        if let Err(error) = transition(&self.notifier, state_with(UpdateStatus::Idle)) {
            eprintln!("context: failed to reset the update state: {error}");
        }
    }

    /// Run one check and record its outcome.
    async fn run_check(&self, channel: String) {
        let state = match check_candidate(&channel).await {
            Ok(Some(candidate)) => state_available(candidate),
            Ok(None) => state_with(UpdateStatus::Idle),
            Err(message) => UpdateState {
                message: Some(message),
                ..state_with(UpdateStatus::Failed)
            },
        };

        if let Err(error) = transition(&self.notifier, state) {
            eprintln!("context: failed to record the update state: {error}");
        }
    }
}

impl UpdateFlow for ServiceUpdateFlow {
    fn check(&self, manual: bool) -> UpdateStateResult<()> {
        // The gate declines before the claim: a decline is not contention.
        let channel = match self.gate.decide(manual) {
            CheckDecision::Decline => return Ok(()),
            CheckDecision::Run { channel } => channel,
        };

        claim(&self.busy)?;

        if let Err(error) = transition(&self.notifier, state_with(UpdateStatus::Checking)) {
            release(&self.busy);
            return Err(error);
        }

        let flow = self.clone();
        tauri::async_runtime::spawn(async move {
            let _release = BusyRelease(Arc::clone(&flow.busy));
            flow.run_check(channel).await;
        });

        Ok(())
    }

    fn install(&self) -> UpdateStateResult<()> {
        if let Some(refusal) = dev_build_refusal() {
            return Err(refusal);
        }

        claim(&self.busy)?;

        let state_path = update_state_path();
        let target = match load_state(&state_path) {
            Ok(state) => install::install_target(&state),
            Err(error) => Err(install::InstallError::Failed(error.to_string())),
        };
        if let Err(error) = target {
            release(&self.busy);
            return Err(install_rejection(error));
        }

        let notifier = self.notifier.clone();
        let executor = install::TauriExecutor::new(self.app.clone());
        let app = self.app.clone();
        let busy = Arc::clone(&self.busy);

        // The plugin API is async, and a blocking wait inside an axum worker
        // would stall the runtime: the sequence runs on a blocking task.
        tauri::async_runtime::spawn_blocking(move || {
            let _release = BusyRelease(busy);

            let outcome = install::install(
                &executor,
                &state_path,
                move |state| transition(&notifier, state).map_err(|error| error.to_string()),
                |service| {
                    install::restart_command(service)
                        .spawn()
                        .map(|_child| ())
                        .map_err(|error| format!("Failed to start the new service: {error}"))
                },
                move || app.exit(0),
            );

            if let Err(error) = outcome {
                eprintln!("context: update install stopped: {error:?}");
            }
        });

        Ok(())
    }
}

/// Write `state` and tell the subscribers what changed.
///
/// Every state change goes through here, so the file and the broadcast never
/// disagree.
fn transition(notifier: &ChangeNotifier, state: UpdateState) -> UpdateStateResult<()> {
    write_state(&update_state_path(), &state)?;

    notifier.notify(UpdateMessage::UpdateStateChanged {
        status: status_name(state.status).to_string(),
        version: state.candidate_version.clone(),
    });

    Ok(())
}

/// Whether a check runs: a manual one always does, an automatic one follows the
/// stored preference.
fn should_check(manual: bool, auto_update: bool) -> bool {
    manual || auto_update
}

/// Claim the flow, or report that a run is already in flight.
fn claim(busy: &Mutex<bool>) -> UpdateStateResult<()> {
    let mut busy = busy.lock().unwrap();

    if *busy {
        return Err(UpdateStateError::Busy);
    }

    *busy = true;
    Ok(())
}

/// Release the flow when its run finished.
fn release(busy: &Mutex<bool>) {
    *busy.lock().unwrap() = false;
}

/// Refuse an install in a debug build: there is no bundle to swap.
fn dev_build_refusal() -> Option<UpdateStateError> {
    cfg!(debug_assertions).then(|| UpdateStateError::Invalid {
        message: "update install is disabled in development builds".to_string(),
    })
}

/// Clears the flow's busy flag when a spawned run ends, a panic included.
struct BusyRelease(Arc<Mutex<bool>>);

impl Drop for BusyRelease {
    fn drop(&mut self) {
        release(&self.0);
    }
}

/// Error the REST layer reports for an install the state does not allow.
fn install_rejection(error: install::InstallError) -> UpdateStateError {
    match error {
        install::InstallError::Busy => UpdateStateError::Busy,
        install::InstallError::NotAvailable => UpdateStateError::Invalid {
            message: "no update candidate to install".to_string(),
        },
        install::InstallError::Failed(message) => UpdateStateError::Invalid { message },
    }
}

/// Whether a freshly started service replaced the binary it just installed.
fn settled_after_install(state: &UpdateState) -> bool {
    matches!(state.status, UpdateStatus::Installing | UpdateStatus::Ready)
        && state.candidate_version.as_deref() == Some(state.current_version.as_str())
}

/// Ask GitHub for the release `channel` should install.
async fn check_candidate(channel: &str) -> Result<Option<Candidate>, String> {
    let client = github_client()?;
    let list = releases::fetch_releases(&client).await?;

    Ok(releases::select_candidate(
        &list,
        channel,
        &current_version(),
    ))
}

/// Client for the GitHub API.
fn github_client() -> Result<reqwest::Client, String> {
    // Install ring as the default crypto provider for rustls, as the health
    // probe does: workspace builds enable rustls without a provider.
    let _ = rustls::crypto::ring::default_provider().install_default();

    reqwest::Client::builder()
        .timeout(GITHUB_REQUEST_TIMEOUT)
        .build()
        .map_err(|error| format!("Failed to create the GitHub client: {error}"))
}

/// Version this build runs.
fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// State that records `status` with every candidate field cleared.
fn state_with(status: UpdateStatus) -> UpdateState {
    UpdateState {
        status,
        current_version: current_version(),
        updated_at: Some(Utc::now().to_rfc3339()),
        ..UpdateState::default()
    }
}

/// State that offers `candidate` for consent.
fn state_available(candidate: Candidate) -> UpdateState {
    UpdateState {
        status: UpdateStatus::Available,
        current_version: current_version(),
        candidate_version: Some(candidate.version.to_string()),
        candidate_tag: Some(candidate.tag),
        candidate_notes: candidate.notes,
        candidate_is_prerelease: Some(candidate.is_prerelease),
        message: None,
        updated_at: Some(Utc::now().to_rfc3339()),
    }
}

/// Status name the broadcast carries, matching the stored spelling.
fn status_name(status: UpdateStatus) -> &'static str {
    match status {
        UpdateStatus::Idle => "idle",
        UpdateStatus::Checking => "checking",
        UpdateStatus::Available => "available",
        UpdateStatus::Downloading => "downloading",
        UpdateStatus::Installing => "installing",
        UpdateStatus::Ready => "ready",
        UpdateStatus::Failed => "failed",
    }
}
