//! The nav-header sync indicator.
//!
//! One control reports git sync and runs the one cycle the app offers: a click imports the sync
//! files, exports the database, then re-reads the status. The state lives in the shell so a second
//! surface reads the same signal, which is why this control owns no signal of its own.

use leptos::prelude::*;
use leptos::task::spawn_local;
use thaw::{Spinner, SpinnerSize};

use crate::api::ApiClientError;
use crate::models::{SyncResponseEnvelope, SyncStatusData, UpdateMessage};
use crate::sync_status::{SyncIndicator as SyncState, SyncPhase, dirty_from_status, sync_title};
use crate::websocket::use_websocket_updates;

// region:    --- Constants

/// Envelope status the server sends before the first init.
const NOT_INITIALIZED: &str = "not_initialized";

// endregion: --- Constants

// region:    --- Components

/// The git-sync control: one icon, colored by state, that runs one sync cycle when clicked.
#[component]
pub fn SyncIndicator() -> impl IntoView {
    // The shell owns the state; this control only reads and writes it.
    let state = expect_context::<RwSignal<Option<SyncState>>>();

    // Seed once on mount. A failed read leaves the signal untouched, so an app whose status can
    // not be read keeps rendering nothing instead of a guessed state.
    Effect::new(move |_| {
        spawn_local(async move {
            match fetch_state().await {
                Ok(fetched) => state.set(fetched),
                Err(error) => log_failure("Failed to read the sync status", &error),
            }
        });
    });

    // An entity change leaves local work for the next export. The updater's own announcement is
    // not entity work, so it never marks the indicator dirty.
    let updates = use_websocket_updates();
    Effect::new(move |_| {
        let Some(message) = updates.get() else {
            return;
        };

        if matches!(message, UpdateMessage::UpdateStateChanged { .. }) {
            return;
        }

        if let Some(current) = state.get_untracked()
            && current.phase == SyncPhase::Idle
        {
            state.set(Some(SyncState {
                dirty: true,
                ..current
            }));
        }
    });

    let on_click = move |_| {
        let Some(current) = state.get_untracked() else {
            return;
        };

        // The button is disabled while a cycle runs; the guard keeps a queued click from starting
        // a second one. A click that ends in a failure starts the cycle again from here.
        if current.phase == SyncPhase::Syncing {
            return;
        }

        spawn_local(async move {
            state.set(Some(SyncState {
                phase: SyncPhase::Syncing,
                dirty: current.dirty,
                message: None,
            }));

            if let Err(error) = run_cycle().await {
                fail(state, current.dirty, &error);
                return;
            }

            // The confirming read is part of the cycle: without it the control can not say what
            // the two sides hold, so a failed read ends the cycle as a failure rather than a
            // spinner that never stops.
            match fetch_state().await {
                Ok(fetched) => state.set(fetched),
                Err(error) => fail(state, current.dirty, &error),
            }
        });
    };

    view! {
        {move || {
            let Some(current) = state.get() else {
                return ().into_any();
            };

            let syncing = current.phase == SyncPhase::Syncing;
            let clean = current.phase == SyncPhase::Idle && !current.dirty;
            let pending = current.phase == SyncPhase::Idle && current.dirty;
            let failed = current.phase == SyncPhase::Failed;
            let title = sync_title(&current);

            view! {
                <button
                    type="button"
                    aria-label="Sync status"
                    title=title
                    disabled=syncing
                    on:click=on_click
                    class="flex items-center transition-opacity hover:opacity-80 cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
                    class:text-ctp-green=move || clean
                    class:text-ctp-yellow=move || pending
                    class:text-ctp-red=move || failed
                >
                    {syncing.then(|| view! { <Spinner size=SpinnerSize::Small/> })}
                    {(!syncing)
                        .then(|| {
                            view! {
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    class="w-5 h-5"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    aria-hidden="true"
                                >
                                    <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"></path>
                                    <path d="M21 3v5h-5"></path>
                                    <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"></path>
                                    <path d="M8 16H3v5"></path>
                                </svg>
                            }
                        })}
                </button>
            }
                .into_any()
        }}
    }
}

// endregion: --- Components

// region:    --- Support

/// Read the status into the state the control renders, or `None` when sync is not set up.
///
/// The envelope reports uninitialized twice, through its status token and through the payload, so
/// either one hides the control. A read that returns no payload knows no state either.
async fn fetch_state() -> Result<Option<SyncState>, ApiClientError> {
    let envelope = crate::api::sync::status().await?;

    Ok(settle(&envelope))
}

/// One cycle: the sync files come in first, so the export writes the merged result back out.
async fn run_cycle() -> Result<(), ApiClientError> {
    crate::api::sync::import().await?;
    crate::api::sync::export().await
}

/// Map a status envelope to the state the control renders, or `None` when it renders nothing.
///
/// The envelope reports uninitialized twice, through its status token and through the payload, so
/// either one hides the control. A read that reports the sync disabled on this machine hides it the
/// same way, and a read that returns no payload knows no state either. The settings section reads
/// the same mapping after it re-enables sync, so the header icon comes back without a reload.
pub(crate) fn settle(envelope: &SyncResponseEnvelope) -> Option<SyncState> {
    let data = envelope.data.as_ref()?;

    if !data.initialized || envelope.status == NOT_INITIALIZED || data.enabled == Some(false) {
        return None;
    }

    Some(SyncState {
        phase: SyncPhase::Idle,
        dirty: dirty(data),
        message: None,
    })
}

/// Whether the reported status leaves work for the next export.
///
/// A status read that could not inspect the sync directory reports no git state; assume work waits
/// rather than claim the two sides agree.
fn dirty(data: &SyncStatusData) -> bool {
    let git_clean = data.git.as_ref().is_some_and(|git| git.clean);
    let database_total = data.database.as_ref().map_or(0, |counts| counts.total);
    let tombstones_total = data.tombstones.as_ref().map_or(0, |counts| counts.total);
    let files_total = data.sync_files.as_ref().map_or(0, |counts| counts.total);

    dirty_from_status(database_total, tombstones_total, files_total, git_clean)
}

/// Report a rejected cycle through the shared state, so the settings surface reads the same result.
fn fail(state: RwSignal<Option<SyncState>>, dirty: bool, error: &ApiClientError) {
    state.set(Some(SyncState {
        phase: SyncPhase::Failed,
        dirty,
        message: Some(error.to_string()),
    }));
}

/// Leave a console record for a rejected call the shared state does not report.
fn log_failure(context: &str, error: &ApiClientError) {
    web_sys::console::error_1(&format!("{context}: {error}").into());
}

// endregion: --- Support
