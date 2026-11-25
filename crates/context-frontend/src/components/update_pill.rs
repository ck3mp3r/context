//! The app-level update pill.
//!
//! One surface reports an update: a pill fixed to the bottom right, mounted once in `app.rs`, so it
//! is on every screen. It renders every [`crate::models::UpdateStatusPayload`] state, offers the one
//! restart code path the service owns, and hides itself per candidate version.

use leptos::prelude::*;
use leptos::task::spawn_local;
use thaw::{Spinner, SpinnerSize};

use crate::api;
use crate::models::{UpdateMessage, UpdateStatusPayload};
use crate::update_status::{update_dismissed, update_failure, update_phrase};
use crate::websocket::use_websocket_updates;

// region:    --- Types

/// What the pill shows for the signals it reads.
#[derive(Clone, Debug, PartialEq)]
struct PillContent {
    /// Phase the pill reports, which is also its `data-update-phase` token.
    phase: String,
    /// The sentence in the pill.
    text: String,
    /// Whether a phase spinner sits beside the sentence.
    spinner: bool,
    /// Whether the **Update** call to action shows.
    update: bool,
    /// Whether the **Restart** call to action shows.
    restart: bool,
    /// Whether the dismiss control shows.
    dismiss: bool,
}

// endregion: --- Types

// region:    --- Components

/// The one update surface: fixed bottom right, above the app chrome and below an open surface.
#[component]
pub fn UpdatePill(status: RwSignal<Option<UpdateStatusPayload>>) -> impl IntoView {
    // The candidate version the user dismissed. In memory, so a GUI relaunch clears it, and keyed
    // to the version, so an install's Ready state stays dismissed and a newer version comes back.
    let dismissed = RwSignal::new(None::<String>);
    // A failure one of the calls to action reported. It outranks the state the service keeps, so a
    // rejected restart reads as a failure instead of a spinner that never ends.
    let local_failure = RwSignal::new(None::<String>);
    let restart_pending = RwSignal::new(false);
    // The candidate the pill last saw named. An install announces `downloading` and `installing`
    // from a state with the candidate fields cleared, so this is how a dismissal stays attached to
    // the version it names for the whole install.
    let last_candidate = RwSignal::new(None::<String>);

    // Seed once on mount: the service owns the state, the pill only reads it.
    Effect::new(move |_| refetch(status));

    Effect::new(move |_| {
        if let Some(version) = status.get().and_then(|payload| payload.candidate_version) {
            last_candidate.set(Some(version));
        }
    });

    // Every announced change re-reads the state. The dismissal is untouched: an install ends at
    // `ready` without restarting anything, so a dismissed candidate stays hidden through it.
    let updates = use_websocket_updates();
    Effect::new(move |_| {
        if matches!(
            updates.get(),
            Some(UpdateMessage::UpdateStateChanged { .. })
        ) {
            restart_pending.set(false);
            local_failure.set(None);
            refetch(status);
        }
    });

    let on_dismiss = move |_| {
        let version = status
            .get_untracked()
            .and_then(|payload| payload.candidate_version)
            .unwrap_or_default();
        dismissed.set(Some(version));
        local_failure.set(None);
        restart_pending.set(false);
    };

    let on_install = move |_| {
        spawn_local(async move {
            if let Err(error) = api::update::install().await {
                local_failure.set(Some(format!("The install failed: {error}")));
            }
        });
    };

    let on_restart = move |_| {
        if restart_pending.get_untracked() {
            return;
        }

        restart_pending.set(true);

        spawn_local(async move {
            let outcome = api::update::restart().await;
            restart_pending.set(false);
            local_failure.set(
                outcome
                    .err()
                    .map(|error| format!("The restart failed: {error}")),
            );
        });
    };

    let content = move || {
        pill_content(
            status.get(),
            last_candidate.get(),
            local_failure.get(),
            dismissed.get(),
        )
    };

    view! {
        {move || {
            let Some(pill) = content() else {
                return ().into_any();
            };

            view! {
                <div class="update-pill" data-update-pill="" data-update-phase=pill.phase>
                    {pill
                        .spinner
                        .then(|| {
                            view! {
                                <span class="shrink-0" data-update-spinner="">
                                    <Spinner size=SpinnerSize::Small/>
                                </span>
                            }
                        })}
                    <span class="min-w-0" role="status" aria-live="polite" data-update-text="">
                        {pill.text}
                    </span>
                    <div class="ml-auto flex shrink-0 items-center gap-2">
                        {pill
                            .update
                            .then(|| {
                                view! {
                                    <button
                                        type="button"
                                        data-update-install=""
                                        on:click=on_install
                                        class="rounded border border-ctp-green px-2 py-1 text-xs text-ctp-green transition-colors hover:bg-ctp-green/20"
                                    >
                                        "Update"
                                    </button>
                                }
                            })}
                        {pill
                            .restart
                            .then(|| {
                                view! {
                                    <button
                                        type="button"
                                        data-update-restart=""
                                        disabled=move || restart_pending.get()
                                        on:click=on_restart
                                        class="rounded border border-ctp-mauve px-2 py-1 text-xs text-ctp-mauve transition-colors hover:bg-ctp-mauve/20 disabled:cursor-not-allowed disabled:opacity-50"
                                    >
                                        "Restart"
                                    </button>
                                }
                            })}
                        {pill
                            .dismiss
                            .then(|| {
                                view! {
                                    <button
                                        type="button"
                                        data-update-dismiss=""
                                        on:click=on_dismiss
                                        class="rounded border border-ctp-surface1 px-2 py-1 text-xs text-ctp-subtext1 transition-colors hover:border-ctp-blue hover:text-ctp-text"
                                    >
                                        "Dismiss"
                                    </button>
                                }
                            })}
                    </div>
                </div>
            }
                .into_any()
        }}
    }
}

// endregion: --- Components

// region:    --- Support

/// Decide what the pill shows, or `None` when it shows nothing.
///
/// A local failure wins over the service's state: the call to action reported it, so it is the
/// newest fact. A dismissal hides the pill for one candidate version. An install announces
/// `downloading` and `installing` from a state with no candidate, so those two phases inherit the
/// last candidate the pill saw named; every other phase is keyed by the version it carries.
fn pill_content(
    payload: Option<UpdateStatusPayload>,
    last_candidate: Option<String>,
    local_failure: Option<String>,
    dismissed: Option<String>,
) -> Option<PillContent> {
    if let Some(message) = local_failure {
        return Some(PillContent {
            phase: "failed".to_string(),
            text: update_failure(Some(&message)),
            spinner: false,
            update: false,
            restart: false,
            dismiss: true,
        });
    }

    let payload = payload?;
    let phase = payload.status.as_str();
    let version = match phase {
        "downloading" | "installing" => payload.candidate_version.clone().or(last_candidate),
        _ => payload.candidate_version.clone(),
    };

    if update_dismissed(dismissed.as_deref(), version.as_deref()) {
        return None;
    }

    let content = match phase {
        "available" => PillContent {
            phase: phase.to_string(),
            text: update_phrase(phase, version.as_deref()).unwrap_or_default(),
            spinner: false,
            update: true,
            restart: false,
            dismiss: true,
        },
        "checking" | "downloading" | "installing" => PillContent {
            phase: phase.to_string(),
            text: update_phrase(phase, None).unwrap_or_default(),
            spinner: true,
            update: false,
            restart: false,
            dismiss: false,
        },
        "ready" => PillContent {
            phase: phase.to_string(),
            text: update_phrase(phase, None).unwrap_or_default(),
            spinner: false,
            update: false,
            restart: true,
            dismiss: true,
        },
        "failed" => PillContent {
            phase: phase.to_string(),
            text: update_failure(payload.message.as_deref()),
            spinner: false,
            update: false,
            restart: false,
            dismiss: true,
        },
        _ => return None,
    };

    Some(content)
}

/// Read the persisted update state into the shared signal the pill reads.
fn refetch(status: RwSignal<Option<UpdateStatusPayload>>) {
    spawn_local(async move {
        match api::update::status().await {
            Ok(payload) => status.set(Some(payload)),
            Err(error) => report(&format!("Failed to read the update state: {error}")),
        }
    });
}

/// Log a client-side failure the way the websocket module does.
fn report(message: &str) {
    web_sys::console::error_1(&message.into());
}

// endregion: --- Support
