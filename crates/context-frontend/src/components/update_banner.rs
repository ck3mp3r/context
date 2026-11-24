//! Shell update surfaces: the banner above the navigation and the version badge.
//!
//! The banner is the only new chrome. It renders nothing while the state is idle,
//! shows the phase while a check or install runs, offers the install once a
//! candidate exists, and follows the websocket announcements afterwards.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::models::{UpdateMessage, UpdateStatusPayload};
use crate::websocket::use_websocket_updates;

/// How long the install request gets to record its first phase.
const INSTALL_SETTLE_DELAY: std::time::Duration = std::time::Duration::from_secs(1);

/// Global update banner, rendered above the navigation on every page.
#[component]
pub fn UpdateBanner(status: RwSignal<Option<UpdateStatusPayload>>) -> impl IntoView {
    // A dismissal lasts until the next state change.
    let dismissed = RwSignal::new(false);
    let installing = RwSignal::new(false);

    // Seed once on mount: a closed GUI loses nothing, it reads the state again.
    Effect::new(move |_| refetch(status));

    // Every announced state change re-reads the state and brings the banner back.
    let updates = use_websocket_updates();
    Effect::new(move |_| {
        if matches!(
            updates.get(),
            Some(UpdateMessage::UpdateStateChanged { .. })
        ) {
            dismissed.set(false);
            installing.set(false);
            refetch(status);
        }
    });

    let on_install = move |_| {
        if installing.get_untracked() {
            return;
        }

        installing.set(true);

        spawn_local(async move {
            match api::update::install().await {
                Ok(()) => {
                    // The service announces each phase; read once soon after so
                    // the banner leaves the candidate even if a frame is lost.
                    set_timeout(move || refetch(status), INSTALL_SETTLE_DELAY);
                }
                Err(error) => {
                    report(&format!("Failed to start the install: {error}"));
                    installing.set(false);
                }
            }
        });
    };

    let on_dismiss = move |_| dismissed.set(true);

    view! {
        {move || {
            if dismissed.get() {
                return ().into_any();
            }

            let Some(payload) = status.get() else {
                return ().into_any();
            };

            match payload.status.as_str() {
                "available" => {
                    let version = payload.candidate_version.clone().unwrap_or_default();
                    let title = if version.is_empty() {
                        "Update available".to_string()
                    } else {
                        format!("Update {version} available")
                    };
                    let notes = payload.candidate_notes.clone();

                    view! {
                        <div class="flex items-center gap-3 border-b border-ctp-green/40 bg-ctp-green/10 px-6 py-2 text-sm">
                            <span class="shrink-0 font-medium text-ctp-text">{title}</span>
                            {notes
                                .map(|notes| {
                                    let title = notes.clone();

                                    view! {
                                        <span class="truncate text-ctp-subtext0" title=title>
                                            {notes}
                                        </span>
                                    }
                                })}
                            <div class="ml-auto flex shrink-0 items-center gap-2">
                                <button
                                    type="button"
                                    disabled=move || installing.get()
                                    on:click=on_install
                                    class="rounded border border-ctp-green px-2 py-1 text-xs text-ctp-green transition-colors hover:bg-ctp-green/20 disabled:cursor-not-allowed disabled:opacity-50"
                                >
                                    {move || if installing.get() { "Installing…" } else { "Install" }}
                                </button>
                                <button
                                    type="button"
                                    on:click=on_dismiss
                                    class="rounded border border-ctp-surface1 px-2 py-1 text-xs text-ctp-subtext1 transition-colors hover:border-ctp-blue hover:text-ctp-text"
                                >
                                    "Dismiss"
                                </button>
                            </div>
                        </div>
                    }
                        .into_any()
                }
                "checking" | "downloading" | "installing" => {
                    let phase = match payload.status.as_str() {
                        "checking" => "Checking for updates…",
                        "downloading" => "Downloading the update…",
                        _ => "Installing the update…",
                    };

                    view! {
                        <div class="border-b border-ctp-blue/40 bg-ctp-blue/10 px-6 py-2 text-sm text-ctp-text">
                            {phase}
                        </div>
                    }
                        .into_any()
                }
                "ready" => {
                    view! {
                        <div class="border-b border-ctp-mauve/40 bg-ctp-mauve/10 px-6 py-2 text-sm text-ctp-text">
                            "Update installed — restarting…"
                        </div>
                    }
                        .into_any()
                }
                "failed" => {
                    let message = payload
                        .message
                        .clone()
                        .unwrap_or_else(|| "The update failed.".to_string());

                    view! {
                        <div class="px-6 py-2">
                            <div class="flex items-center gap-3 rounded border border-ctp-red bg-ctp-red/10 p-4">
                                <div class="min-w-0">
                                    <p class="font-semibold text-ctp-red">"Update failed"</p>
                                    <p class="mt-1 truncate text-sm text-ctp-subtext0">{message}</p>
                                </div>
                                <button
                                    type="button"
                                    on:click=on_dismiss
                                    class="ml-auto shrink-0 rounded border border-ctp-surface1 px-2 py-1 text-xs text-ctp-subtext1 transition-colors hover:border-ctp-blue hover:text-ctp-text"
                                >
                                    "Dismiss"
                                </button>
                            </div>
                        </div>
                    }
                        .into_any()
                }
                _ => ().into_any(),
            }
        }}
    }
}

/// Candidate version shown beside the running version in the navigation.
#[component]
pub fn UpdateBadge(status: RwSignal<Option<UpdateStatusPayload>>) -> impl IntoView {
    view! {
        {move || {
            status
                .get()
                .and_then(|payload| payload.candidate_version)
                .map(|version| {
                    view! {
                        <span class="font-mono text-xs text-ctp-mauve">{format!("→ {version}")}</span>
                    }
                })
        }}
    }
}

/// Read the persisted update state into the shared banner signal.
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
