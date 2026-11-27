//! The app-level update pill.
//!
//! One surface reports an update: a pill fixed to the bottom right, mounted once in `app.rs`, so it
//! is on every screen. It renders every [`crate::models::UpdateStatusPayload`] state, offers the one
//! restart code path the service owns, retries a failure with a fresh check, reports the outcome of
//! a manual check that left no state to render, and hides itself per candidate version.

use leptos::prelude::*;
use leptos::task::spawn_local;
use thaw::{Spinner, SpinnerSize};

use crate::api;
use crate::models::{UpdateCheckOutcome, UpdateMessage, UpdateStatusPayload};
use crate::update_status::{check_outcome_phrase, update_dismissed, update_failure, update_phrase};
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
    /// Whether the **Check again** call to action shows.
    check: bool,
    /// Whether the dismiss control shows.
    dismiss: bool,
}

// endregion: --- Types

// region:    --- Components

/// The one update surface: fixed bottom right, above the app chrome and below an open surface.
#[component]
pub fn UpdatePill(
    /// The state the service keeps.
    status: RwSignal<Option<UpdateStatusPayload>>,
    /// The outcome of a manual check that reported no state of its own.
    outcome: RwSignal<Option<UpdateCheckOutcome>>,
) -> impl IntoView {
    // The candidate version the user dismissed. In memory, so a GUI relaunch clears it, and keyed
    // to the version, so an install's Ready state stays dismissed and a newer version comes back.
    let dismissed = RwSignal::new(None::<String>);
    // A failure one of the calls to action reported. It outranks the state the service keeps, so a
    // rejected restart reads as a failure instead of a spinner that never ends.
    let local_failure = RwSignal::new(None::<String>);
    // A view the user dismissed that names no candidate version. A dismissal is keyed to a
    // version, and these views carry none, so the hide lasts only until the next announcement.
    let hidden = RwSignal::new(false);
    let restart_pending = RwSignal::new(false);
    // The candidate the pill last saw named. An install announces `downloading` and `installing`
    // from a state with the candidate fields cleared, so this is how a dismissal stays attached to
    // the version it names for the whole install.
    let last_candidate = RwSignal::new(None::<String>);

    // Seed once on mount: the service owns the state, the pill only reads it.
    Effect::new(move |_| refetch(status, local_failure));

    Effect::new(move |_| {
        if let Some(version) = status.get().and_then(|payload| payload.candidate_version) {
            last_candidate.set(Some(version));
        }
    });

    // Every announced change re-reads the state. The version key is untouched: an install ends at
    // `ready` without restarting anything, so a dismissed candidate stays hidden through it. The
    // ephemeral hide ends here, because it belongs to the state that was on screen when it was
    // made and that state is now history. A check outcome belongs to the click that produced it,
    // so an announcement supersedes it too.
    let updates = use_websocket_updates();
    Effect::new(move |_| {
        if matches!(
            updates.get(),
            Some(UpdateMessage::UpdateStateChanged { .. })
        ) {
            restart_pending.set(false);
            local_failure.set(None);
            outcome.set(None);
            hidden.set(false);
            refetch(status, local_failure);
        }
    });

    let on_dismiss = move |_| {
        let dismissed_a_failure = local_failure.get_untracked().is_some();
        local_failure.set(None);
        restart_pending.set(false);

        // A check outcome names no candidate either, so its hide cannot be keyed to a version.
        // Dismissing it drops the report, which hands the pill back to the state the service
        // keeps: a settled `idle` state renders nothing, so the pill goes away.
        let dismissed_an_outcome = outcome.get_untracked().is_some();
        outcome.set(None);

        let payload = status.get_untracked();
        // A failure names no candidate, so its hide cannot be keyed to a version: an empty key
        // would hide every later payload that also names none. It lasts until the next
        // announcement. Dismissing a local failure instead drops the report, which hands the pill
        // back to the state the service keeps: an offer that is still live shows again at once.
        if payload
            .as_ref()
            .is_some_and(|payload| payload.status == "failed")
        {
            hidden.set(true);
            return;
        }

        if dismissed_a_failure || dismissed_an_outcome {
            return;
        }

        match payload.and_then(|payload| payload.candidate_version) {
            Some(version) => dismissed.set(Some(version)),
            // A bare notice has no version to key, so its hide is ephemeral too.
            None => hidden.set(true),
        }
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
            let result = api::update::restart().await;
            restart_pending.set(false);
            local_failure.set(
                result
                    .err()
                    .map(|error| format!("The restart failed: {error}")),
            );
        });
    };

    // The one retry the state machine allows after a failure: a new check, the same call the boot
    // path makes. An install is POST-and-forget, so nothing else can start one from `failed`.
    // A rejection is a result the user asked for, so it renders as a failure instead of a
    // console line nobody reads.
    let on_check = move |_| {
        // The retry supersedes whatever the last click reported.
        outcome.set(None);

        spawn_local(async move {
            if let Err(error) = api::update::check(true).await {
                local_failure.set(Some(format!("Failed to start an update check: {error}")));
            }
        });
    };

    let content = move || {
        pill_content(
            status.get(),
            last_candidate.get(),
            local_failure.get(),
            outcome.get(),
            dismissed.get(),
            hidden.get(),
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
                                        "Restart now"
                                    </button>
                                }
                            })}
                        {pill
                            .check
                            .then(|| {
                                view! {
                                    <button
                                        type="button"
                                        data-update-check=""
                                        on:click=on_check
                                        class="rounded border border-ctp-blue px-2 py-1 text-xs text-ctp-blue transition-colors hover:bg-ctp-blue/20"
                                    >
                                        "Check again"
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
/// `hidden` is the dismissal of a view that names no version: a failure, or a bare notice. It ends
/// at the next announcement, so it never outlives the state that produced it. A check outcome
/// carries what the user's own click reported, so it outranks the state the service keeps: a
/// rejected trigger answers "already running" while an older `available` or `checking` state is
/// still on screen, and that answer is the click's result. It outranks `hidden` for the same
/// reason.
fn pill_content(
    payload: Option<UpdateStatusPayload>,
    last_candidate: Option<String>,
    local_failure: Option<String>,
    outcome: Option<UpdateCheckOutcome>,
    dismissed: Option<String>,
    hidden: bool,
) -> Option<PillContent> {
    if let Some(message) = local_failure {
        return Some(PillContent {
            phase: "failed".to_string(),
            text: update_failure(Some(&message)),
            spinner: false,
            update: false,
            restart: false,
            check: true,
            dismiss: true,
        });
    }

    if let Some(outcome) = outcome {
        return Some(outcome_content(outcome));
    }

    if hidden {
        return None;
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
            check: false,
            dismiss: true,
        },
        "checking" | "downloading" | "installing" => PillContent {
            phase: phase.to_string(),
            text: update_phrase(phase, None).unwrap_or_default(),
            spinner: true,
            update: false,
            restart: false,
            check: false,
            dismiss: false,
        },
        "ready" => PillContent {
            phase: phase.to_string(),
            text: update_phrase(phase, None).unwrap_or_default(),
            spinner: false,
            update: false,
            restart: true,
            check: false,
            dismiss: true,
        },
        "failed" => PillContent {
            phase: phase.to_string(),
            text: update_failure(payload.message.as_deref()),
            spinner: false,
            update: false,
            restart: false,
            check: true,
            dismiss: true,
        },
        _ => return None,
    };

    Some(content)
}

/// What the pill shows for a manual check that reported no state of its own.
///
/// A failure keeps the `failed` phase and offers the one retry the state machine allows. A check
/// that found nothing keeps its own `no-update` phase: it is a result, and a Dismiss control ends
/// it without pretending the update failed.
fn outcome_content(outcome: UpdateCheckOutcome) -> PillContent {
    let (phase, text, check) = match &outcome {
        UpdateCheckOutcome::Failed(detail) => {
            ("failed", check_outcome_phrase(Some(detail.as_str())), true)
        }
        UpdateCheckOutcome::NoUpdate => ("no-update", check_outcome_phrase(None), false),
    };

    PillContent {
        phase: phase.to_string(),
        text,
        spinner: false,
        update: false,
        restart: false,
        check,
        dismiss: true,
    }
}

/// Read the persisted update state into the shared signal the pill reads.
fn refetch(status: RwSignal<Option<UpdateStatusPayload>>, local_failure: RwSignal<Option<String>>) {
    spawn_local(async move {
        match api::update::status().await {
            Ok(payload) => status.set(Some(payload)),
            Err(error) => {
                local_failure.set(Some(format!("Failed to read the update state: {error}")))
            }
        }
    });
}

// endregion: --- Support
