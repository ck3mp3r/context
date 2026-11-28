use leptos::prelude::*;
use leptos::task::spawn_local;
use thaw::{Spinner, SpinnerSize};

use crate::api::{self, ApiClientError};
use crate::components::sync_indicator::settle as settle_indicator;
use crate::models::{
    SettingsDefaults, SettingsPayload, SettingsRunning, SettingsValues, SyncResponseEnvelope,
    SyncStatusData, UpdateCheckOutcome, UpdateStatusPayload,
};
use crate::sync_status::SyncIndicator as SyncState;

/// `/settings` page: edit the stored configuration, defaults in view.
#[component]
pub fn Settings() -> impl IntoView {
    view! {
        <div data-page="settings" class="flex flex-col min-h-[calc(100vh-8rem)]">
            <div class="container mx-auto p-6 flex-1">
                <SettingsEditor/>
            </div>
        </div>
    }
}

#[component]
fn SettingsEditor() -> impl IntoView {
    let (loading, set_loading) = signal(true);
    let (load_error, set_load_error) = signal(None::<String>);
    let (save_error, set_save_error) = signal(None::<String>);
    let (saving, set_saving) = signal(false);

    // Last payload the server answered with: the file, plus what it runs.
    let (stored, set_stored) = signal(SettingsValues::default());
    let (defaults, set_defaults) = signal(None::<SettingsDefaults>);
    let (running, set_running) = signal(None::<SettingsRunning>);

    // Draft the user edits; text fields stay strings until save parses them.
    let host = RwSignal::new(String::new());
    let port = RwSignal::new(String::new());
    let verbosity = RwSignal::new(0u8);
    let enable_docs = RwSignal::new(false);
    let skills_dir = RwSignal::new(String::new());
    let start_at_login = RwSignal::new(true);
    let auto_update = RwSignal::new(true);
    let pre_release = RwSignal::new(false);

    // Manual check: the request is in flight. The button shows that, and the pill reports every
    // outcome: the page renders no status text at all.
    let check_pending = RwSignal::new(false);
    let update_status = expect_context::<RwSignal<Option<UpdateStatusPayload>>>();
    // Outcome of a check that leaves no state to render: the pill shows it, because the page
    // renders no status of its own.
    let check_outcome = expect_context::<RwSignal<Option<UpdateCheckOutcome>>>();

    // One payload update refreshes the draft and the restart markers.
    let apply_payload = move |payload: SettingsPayload| {
        let effective = effective_values(&payload.values, &payload.defaults);

        host.set(effective.host);
        port.set(effective.port.to_string());
        verbosity.set(effective.verbosity);
        enable_docs.set(effective.enable_docs);
        skills_dir.set(effective.skills_dir);
        start_at_login.set(effective.start_at_login);
        auto_update.set(effective.auto_update);
        pre_release.set(effective.update_channel == PRE_RELEASE_CHANNEL);

        set_stored.set(payload.values);
        set_defaults.set(Some(payload.defaults));
        set_running.set(Some(payload.running));
    };

    // Fetch once on mount; the payload seeds every draft field.
    Effect::new(move || {
        set_loading.set(true);

        spawn_local(async move {
            match api::settings::get().await {
                Ok(payload) => {
                    apply_payload(payload);
                    set_load_error.set(None);
                }
                Err(error) => set_load_error.set(Some(error.to_string())),
            }
            set_loading.set(false);
        });
    });

    // Stored values the running process has not picked up yet.
    let restart = Signal::derive(move || match (defaults.get(), running.get()) {
        (Some(defaults), Some(running)) => {
            RestartFields::between(&effective_values(&stored.get(), &defaults), &running)
        }
        _ => RestartFields::default(),
    });

    let restart_host = Signal::derive(move || restart.get().host);
    let restart_port = Signal::derive(move || restart.get().port);
    let restart_verbosity = Signal::derive(move || restart.get().verbosity);
    let restart_enable_docs = Signal::derive(move || restart.get().enable_docs);
    let restart_skills_dir = Signal::derive(move || restart.get().skills_dir);

    let default_host = Signal::derive(move || defaults.get().map(|d| d.host).unwrap_or_default());
    let default_port = Signal::derive(move || {
        defaults
            .get()
            .map(|d| d.port.to_string())
            .unwrap_or_default()
    });
    let default_verbosity = Signal::derive(move || {
        defaults
            .get()
            .map(|d| format!("{} ({})", d.verbosity, verbosity_label(d.verbosity)))
            .unwrap_or_default()
    });
    let default_enable_docs = Signal::derive(move || {
        defaults
            .get()
            .map(|d| d.enable_docs.to_string())
            .unwrap_or_default()
    });
    let default_skills_dir =
        Signal::derive(move || defaults.get().map(|d| d.skills_dir).unwrap_or_default());
    let default_start_at_login = Signal::derive(move || {
        defaults
            .get()
            .map(|d| d.start_at_login.to_string())
            .unwrap_or_default()
    });
    let default_auto_update = Signal::derive(move || {
        defaults
            .get()
            .map(|d| d.auto_update.to_string())
            .unwrap_or_default()
    });
    let default_update_channel =
        Signal::derive(move || defaults.get().map(|d| d.update_channel).unwrap_or_default());

    // The app runner binds a fixed host: the stored host never applies to it.
    let host_fixed = Signal::derive(move || running.get().is_some_and(|r| r.host_fixed));
    let hint_host = Signal::derive(move || match running.get() {
        Some(running) if running.host_fixed => {
            format!("fixed by the running service: {}", running.host)
        }
        _ => format!("default: {}", default_host.get()),
    });

    let reset_host = Callback::new(move |()| {
        if let Some(defaults) = defaults.get_untracked()
            && !host_fixed.get_untracked()
        {
            host.set(defaults.host);
        }
    });
    let reset_port = Callback::new(move |()| {
        if let Some(defaults) = defaults.get_untracked() {
            port.set(defaults.port.to_string());
        }
    });
    let reset_verbosity = Callback::new(move |()| {
        if let Some(defaults) = defaults.get_untracked() {
            verbosity.set(defaults.verbosity);
        }
    });
    let reset_enable_docs = Callback::new(move |()| {
        if let Some(defaults) = defaults.get_untracked() {
            enable_docs.set(defaults.enable_docs);
        }
    });
    let reset_skills_dir = Callback::new(move |()| {
        if let Some(defaults) = defaults.get_untracked() {
            skills_dir.set(defaults.skills_dir);
        }
    });
    let reset_start_at_login = Callback::new(move |()| {
        if let Some(defaults) = defaults.get_untracked() {
            start_at_login.set(defaults.start_at_login);
        }
    });
    let reset_auto_update = Callback::new(move |()| {
        if let Some(defaults) = defaults.get_untracked() {
            auto_update.set(defaults.auto_update);
        }
    });
    let reset_update_channel = Callback::new(move |()| {
        if let Some(defaults) = defaults.get_untracked() {
            pre_release.set(defaults.update_channel == PRE_RELEASE_CHANNEL);
        }
    });

    // Save writes only the fields that differ from their default: a reset
    // field drops its key from the file.
    let on_save = Callback::new(move |()| {
        let Some(defaults) = defaults.get_untracked() else {
            return;
        };

        let port_draft = port.get_untracked().trim().to_string();
        let Ok(port_number) = port_draft.parse::<u16>() else {
            set_save_error.set(Some(format!(
                "Port must be a whole number between 1 and 65535, got '{port_draft}'"
            )));
            return;
        };

        // A fixed host never applies to the running process, so the save keeps
        // whatever the file stored instead of comparing the draft.
        let host_value = if host_fixed.get_untracked() {
            stored.get_untracked().host
        } else {
            override_string(host.get_untracked(), &defaults.host)
        };

        let values = SettingsValues {
            host: host_value,
            port: override_u16(port_number, defaults.port),
            verbosity: override_u8(verbosity.get_untracked(), defaults.verbosity),
            enable_docs: override_bool(enable_docs.get_untracked(), defaults.enable_docs),
            skills_dir: override_string(skills_dir.get_untracked(), &defaults.skills_dir),
            start_at_login: override_bool(start_at_login.get_untracked(), defaults.start_at_login),
            auto_update: override_bool(auto_update.get_untracked(), defaults.auto_update),
            update_channel: override_string(
                channel_draft(pre_release.get_untracked()),
                &defaults.update_channel,
            ),
        };

        set_saving.set(true);
        set_save_error.set(None);

        spawn_local(async move {
            match api::settings::put(&values).await {
                Ok(payload) => {
                    apply_payload(payload);
                    set_save_error.set(None);
                }
                Err(error) => set_save_error.set(Some(error_message(&error))),
            }
            set_saving.set(false);
        });
    });

    // Manual check: trigger it, then read the state into the shared pill signals.
    //
    // The pill is the one status surface, so this flow writes the signals the pill reads: the
    // first read may still say "checking" and shows that at once, and one bounded second read
    // settles the state, so the pill cannot freeze on the interim one. At most two reads per
    // click, and no timer. A settled `idle` state renders no payload worth seeing, so it becomes
    // the shared no-update outcome: a check that found nothing must say so, or it looks exactly
    // like a failure the client swallowed. A transport failure goes to the pill's failure view
    // with the client's own wording.
    let on_check = Callback::new(move |()| {
        if check_pending.get_untracked() {
            return;
        }

        check_pending.set(true);
        // The click supersedes whatever the last one reported.
        check_outcome.set(None);

        // The button returns to its idle label.
        let settle = move || check_pending.set(false);

        // Show a settled payload, or record that the check found nothing to install.
        let apply_settled = move |payload: UpdateStatusPayload| {
            let idle = payload.status == "idle";
            update_status.set(Some(payload));
            check_outcome.set(idle.then_some(UpdateCheckOutcome::NoUpdate));
        };

        // Read the status once: show it, or follow a still-running check once.
        let read_status = move || {
            spawn_local(async move {
                match api::update::status().await {
                    Ok(payload) if payload.status == "checking" => {
                        update_status.set(Some(payload));

                        set_timeout(
                            move || {
                                spawn_local(async move {
                                    match api::update::status().await {
                                        Ok(payload) => apply_settled(payload),
                                        Err(error) => {
                                            report_check_failure(check_outcome, &error);
                                        }
                                    }

                                    settle();
                                });
                            },
                            CHECK_SETTLE_DELAY,
                        );
                    }
                    Ok(payload) => {
                        apply_settled(payload);
                        settle();
                    }
                    Err(error) => {
                        report_check_failure(check_outcome, &error);
                        settle();
                    }
                }
            });
        };

        spawn_local(async move {
            match api::update::check(true).await {
                Ok(()) => set_timeout(read_status, CHECK_SETTLE_DELAY),
                Err(error) => {
                    report_check_failure(check_outcome, &error);
                    settle();
                }
            }
        });
    });

    view! {
        {move || {
            if loading.get() {
                view! { <p class="text-ctp-subtext0">"Loading settings..."</p> }.into_any()
            } else if let Some(message) = load_error.get() {
                view! {
                    <div class="bg-ctp-red/10 border border-ctp-red rounded p-4">
                        <p class="text-ctp-red font-semibold">"Error loading settings"</p>
                        <p class="text-ctp-subtext0 text-sm mt-2">{message}</p>
                    </div>
                }
                    .into_any()
            } else {
                view! {
                    <div class="max-w-3xl">
                        <h2 class="text-2xl font-bold text-ctp-text">"Settings"</h2>
                        <p class="text-ctp-subtext0 text-sm mt-1">
                            "Values are stored in config.toml. Saving does not change the running"
                            " server: restart it to apply the stored values."
                        </p>

                        {move || {
                            let fields = restart.get().labels();
                            (!fields.is_empty())
                                .then(|| {
                                    view! {
                                        <div class="bg-ctp-yellow/10 border border-ctp-yellow/40 rounded p-4 mt-4">
                                            <p class="text-ctp-yellow font-semibold">"Restart to apply"</p>
                                            <p class="text-ctp-subtext0 text-sm mt-1">
                                                "Stored values differ from the running server: "
                                                {fields.join(", ")}
                                            </p>
                                        </div>
                                    }
                                })
                        }}

                        <div class="mt-4">
                            <SettingsField
                                title="Host"
                                hint=hint_host
                                restart=restart_host
                                reset_disabled=host_fixed
                                on_reset=reset_host
                            >
                                <input
                                    type="text"
                                    disabled=move || host_fixed.get()
                                    prop:value=move || host.get()
                                    on:input=move |ev| host.set(event_target_value(&ev))
                                    class="w-full rounded-lg border border-ctp-surface1 bg-ctp-surface0 px-4 py-2 text-ctp-text placeholder-ctp-subtext0 focus:border-ctp-blue focus:ring-2 focus:ring-ctp-blue focus:outline-none disabled:opacity-50 disabled:cursor-not-allowed"
                                />
                            </SettingsField>

                            <SettingsField
                                title="Port"
                                hint=default_line(default_port)
                                restart=restart_port
                                reset_disabled=Signal::derive(|| false)
                                on_reset=reset_port
                            >
                                <input
                                    type="text"
                                    inputmode="numeric"
                                    prop:value=move || port.get()
                                    on:input=move |ev| port.set(event_target_value(&ev))
                                    class="w-full rounded-lg border border-ctp-surface1 bg-ctp-surface0 px-4 py-2 text-ctp-text placeholder-ctp-subtext0 focus:border-ctp-blue focus:ring-2 focus:ring-ctp-blue focus:outline-none"
                                />
                            </SettingsField>

                            <SettingsField
                                title="Verbosity"
                                hint=default_line(default_verbosity)
                                restart=restart_verbosity
                                reset_disabled=Signal::derive(|| false)
                                on_reset=reset_verbosity
                            >
                                <select
                                    on:change=move |ev| {
                                        if let Ok(level) = event_target_value(&ev).parse::<u8>() {
                                            verbosity.set(level);
                                        }
                                    }
                                    class="w-full rounded-lg border border-ctp-surface1 bg-ctp-surface0 px-4 py-2 text-ctp-text focus:border-ctp-blue focus:ring-2 focus:ring-ctp-blue focus:outline-none"
                                >
                                    {(0..=3u8)
                                        .map(|level| {
                                            view! {
                                                <option
                                                    value=level.to_string()
                                                    selected=move || verbosity.get() == level
                                                >
                                                    {format!("{level} ({})", verbosity_label(level))}
                                                </option>
                                            }
                                        })
                                        .collect_view()}
                                </select>
                            </SettingsField>

                            <SettingsField
                                title="Enable docs"
                                hint=default_line(default_enable_docs)
                                restart=restart_enable_docs
                                reset_disabled=Signal::derive(|| false)
                                on_reset=reset_enable_docs
                            >
                                <input
                                    type="checkbox"
                                    prop:checked=move || enable_docs.get()
                                    on:change=move |ev| enable_docs.set(event_target_checked(&ev))
                                    class="h-4 w-4 accent-ctp-blue"
                                />
                            </SettingsField>

                            <SettingsField
                                title="Skills directory"
                                hint=default_line(default_skills_dir)
                                restart=restart_skills_dir
                                reset_disabled=Signal::derive(|| false)
                                on_reset=reset_skills_dir
                            >
                                <input
                                    type="text"
                                    prop:value=move || skills_dir.get()
                                    on:input=move |ev| skills_dir.set(event_target_value(&ev))
                                    class="w-full rounded-lg border border-ctp-surface1 bg-ctp-surface0 px-4 py-2 text-ctp-text placeholder-ctp-subtext0 focus:border-ctp-blue focus:ring-2 focus:ring-ctp-blue focus:outline-none"
                                />
                            </SettingsField>

                            <SettingsField
                                title="Start at login"
                                hint=default_line(default_start_at_login)
                                restart=Signal::derive(|| false)
                                reset_disabled=Signal::derive(|| false)
                                on_reset=reset_start_at_login
                            >
                                <div class="flex items-center gap-3">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || start_at_login.get()
                                        on:change=move |ev| {
                                            start_at_login.set(event_target_checked(&ev))
                                        }
                                        class="h-4 w-4 accent-ctp-blue"
                                    />
                                    <span class="text-xs text-ctp-overlay0">
                                        "Stored in config.toml; this build does not apply it."
                                    </span>
                                </div>
                            </SettingsField>

                            <SettingsField
                                title="Auto-update"
                                hint=default_line(default_auto_update)
                                restart=Signal::derive(|| false)
                                reset_disabled=Signal::derive(|| false)
                                on_reset=reset_auto_update
                            >
                                <div class="flex items-center gap-3">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || auto_update.get()
                                        on:change=move |ev| {
                                            auto_update.set(event_target_checked(&ev))
                                        }
                                        class="h-4 w-4 accent-ctp-blue"
                                    />
                                    <span class="text-xs text-ctp-overlay0">
                                        "Auto-checked when the app opens. Applies on the next check."
                                    </span>
                                </div>
                            </SettingsField>

                            <SettingsField
                                title="Update channel"
                                hint=default_line(default_update_channel)
                                restart=Signal::derive(|| false)
                                reset_disabled=Signal::derive(|| false)
                                on_reset=reset_update_channel
                            >
                                <div class="flex items-center gap-3">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || pre_release.get()
                                        on:change=move |ev| {
                                            pre_release.set(event_target_checked(&ev))
                                        }
                                        class="h-4 w-4 accent-ctp-blue"
                                    />
                                    <span class="text-ctp-text">"Use pre-release channel"</span>
                                    <span class="text-xs text-ctp-overlay0">
                                        "Stable channel by default. Applies on the next check."
                                    </span>
                                </div>
                            </SettingsField>
                        </div>

                        <div class="mt-6 flex items-center gap-4">
                            <button
                                type="button"
                                disabled=move || saving.get()
                                on:click=move |_| on_save.run(())
                                class="rounded-lg bg-ctp-blue px-4 py-2 font-medium text-ctp-base transition-colors hover:bg-ctp-sapphire disabled:opacity-50 disabled:cursor-not-allowed"
                            >
                                {move || if saving.get() { "Saving..." } else { "Save" }}
                            </button>

                            <button
                                type="button"
                                disabled=move || check_pending.get()
                                on:click=move |_| on_check.run(())
                                class="rounded-lg border border-ctp-surface1 px-4 py-2 font-medium text-ctp-text transition-colors hover:border-ctp-blue disabled:opacity-50 disabled:cursor-not-allowed"
                            >
                                {move || {
                                    if check_pending.get() {
                                        view! { <Spinner size=SpinnerSize::Small/> }.into_any()
                                    } else {
                                        view! { "Check for updates" }.into_any()
                                    }
                                }}
                            </button>
                        </div>

                        {move || {
                            save_error
                                .get()
                                .map(|message| {
                                    view! {
                                        <div class="bg-ctp-red/10 border border-ctp-red rounded p-4 mt-4">
                                            <p class="text-ctp-red font-semibold">"Save failed"</p>
                                            <p class="text-ctp-subtext0 text-sm mt-2">{message}</p>
                                        </div>
                                    }
                                })
                        }}

                        <SyncSection/>
                    </div>
                }
                    .into_any()
            }
        }}
    }
}

/// One labelled field with its hint line, reset button, and restart marker.
#[component]
fn SettingsField(
    title: &'static str,
    /// Line rendered under the control.
    hint: Signal<String>,
    /// Whether the stored value waits for a restart.
    restart: Signal<bool>,
    /// Whether the reset button is unavailable.
    reset_disabled: Signal<bool>,
    /// Put the draft back to the built-in default.
    on_reset: Callback<()>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="border-b border-ctp-surface1 py-4">
            <div class="flex items-center justify-between gap-4">
                <div class="flex items-center gap-3">
                    <span class="font-medium text-ctp-text">{title}</span>
                    {move || {
                        restart
                            .get()
                            .then(|| {
                                view! {
                                    <span class="rounded bg-ctp-yellow/20 px-2 py-0.5 text-xs text-ctp-yellow">
                                        "restart to apply"
                                    </span>
                                }
                            })
                    }}
                </div>
                <button
                    type="button"
                    title="Reset this field to its built-in default"
                    disabled=move || reset_disabled.get()
                    on:click=move |_| on_reset.run(())
                    class="rounded border border-ctp-surface1 px-2 py-1 text-xs text-ctp-subtext1 transition-colors hover:border-ctp-blue hover:text-ctp-text disabled:opacity-50 disabled:cursor-not-allowed"
                >
                    "Reset"
                </button>
            </div>

            <div class="mt-2">{children()}</div>

            <p class="mt-1 text-xs text-ctp-overlay0">{move || hint.get()}</p>
        </div>
    }
}

/// Sync section: the stored sync state, the one-time setup while sync is absent, the initialized
/// remote and pause controls, and the resume control while sync is disabled.
///
/// The backend offers init, import, export, a remote update or removal, a disable that pauses sync
/// without touching the data or the git history, and an enable that resumes it; this section drives
/// the setup, the remote edit, and the pause and resume. Triggering a cycle belongs to the header
/// indicator, which owns the shared sync state.
#[component]
fn SyncSection() -> impl IntoView {
    // `None` while the mount-time status read has not answered yet.
    let (state, set_state) = signal(None::<SyncView>);
    let (load_error, set_load_error) = signal(None::<String>);
    let (setup_error, set_setup_error) = signal(None::<String>);
    let (pending, set_pending) = signal(false);
    let remote = RwSignal::new(String::new());

    // The initialized view edits the remote: the checkbox says whether a remote is in use at all,
    // the draft starts at the stored URL, and each write carries its own pending flag and error so
    // one failure never blocks another control.
    let use_remote = RwSignal::new(false);
    let remote_present = RwSignal::new(false);
    let remote_draft = RwSignal::new(String::new());
    let (remote_pending, set_remote_pending) = signal(false);
    let (remote_error, set_remote_error) = signal(None::<String>);
    let (disable_pending, set_disable_pending) = signal(false);
    let (disable_error, set_disable_error) = signal(None::<String>);
    let (enable_pending, set_enable_pending) = signal(false);
    let (enable_error, set_enable_error) = signal(None::<String>);
    // One click asks, the next pauses sync: no timer, no second control to dismiss.
    let (confirming_disable, set_confirming_disable) = signal(false);

    // The header indicator owns the same state; a successful disable clears it, so the icon goes
    // away without a reload, and a successful enable restores it from the same status mapping.
    let sync_state = expect_context::<RwSignal<Option<SyncState>>>();

    // One status read decides the view and seeds the remote row, so the controls start at what the
    // server stored rather than at a guess.
    let apply_view = move |fetched: SyncView| {
        if let SyncView::Initialized(data) = &fetched {
            let stored = data.remote_url.clone();
            use_remote.set(stored.is_some());
            remote_present.set(stored.is_some());
            remote_draft.set(stored.unwrap_or_default());
        }
        set_state.set(Some(fetched));
    };

    let apply_fetched = move |fetched: Result<SyncView, String>| match fetched {
        Ok(view) => {
            apply_view(view);
            set_load_error.set(None);
        }
        Err(detail) => set_load_error.set(Some(detail)),
    };

    // The mount-time read decides which view the section renders.
    Effect::new(move |_| {
        spawn_local(async move {
            apply_fetched(fetch_view().await);
        });
    });

    // Setup runs the first init, then re-reads the status, so the view reports what the server
    // stored instead of what the click asked for. A blank draft asks for a local-only setup.
    let on_setup = move |_| {
        if pending.get_untracked() {
            return;
        }

        set_pending.set(true);
        set_setup_error.set(None);

        let draft = remote.get_untracked().trim().to_string();
        let remote_url = (!draft.is_empty()).then_some(draft);

        spawn_local(async move {
            match api::sync::init(remote_url).await {
                Ok(()) => apply_fetched(fetch_view().await),
                Err(error) => set_setup_error.set(Some(error_message(&error))),
            }
            set_pending.set(false);
        });
    };

    // The remote row has one action: write the draft when the box is checked, remove the remote
    // when it is not. The read after the write is what updates the lines, so the section reports
    // what the server stored, not what the click asked for.
    let on_remote_action = move |_| {
        if remote_pending.get_untracked() {
            return;
        }

        let checked = use_remote.get_untracked();
        let draft = remote_draft.get_untracked().trim().to_string();
        let payload = if checked && !draft.is_empty() {
            Some(draft)
        } else if !checked && remote_present.get_untracked() {
            None
        } else {
            return;
        };

        set_remote_pending.set(true);
        set_remote_error.set(None);

        spawn_local(async move {
            match api::sync::set_remote(payload).await {
                Ok(()) => apply_fetched(fetch_view().await),
                Err(error) => set_remote_error.set(Some(error_message(&error))),
            }
            set_remote_pending.set(false);
        });
    };

    // The action is live only when it would change something: a checked box with a non-blank
    // draft, or an unchecked box whose stored remote can go.
    let remote_action_ready = move || {
        if use_remote.get() {
            !remote_draft.get().trim().is_empty()
        } else {
            remote_present.get()
        }
    };

    // Two clicks: the first only arms the control, the second pauses sync. A success leaves the
    // sync initialized but disabled, so the shared state drops to `None` and the header icon
    // disappears while the data and the git history stay in place.
    let on_disable = move |_| {
        if disable_pending.get_untracked() {
            return;
        }

        if !confirming_disable.get_untracked() {
            set_disable_error.set(None);
            set_confirming_disable.set(true);
            return;
        }

        set_disable_pending.set(true);
        set_disable_error.set(None);

        spawn_local(async move {
            match api::sync::disable().await {
                Ok(()) => {
                    sync_state.set(None);
                    set_confirming_disable.set(false);
                    // The server keeps the directory, so the read reports a disabled sync; the
                    // section renders that answer rather than the setup view.
                    apply_fetched(fetch_view().await);
                }
                Err(error) => {
                    set_disable_error.set(Some(error_message(&error)));
                    set_confirming_disable.set(false);
                }
            }
            set_disable_pending.set(false);
        });
    };

    // Enabling is the mirror of disabling: the server resumes on the intact directory, and one
    // status read restores both the section and the header icon.
    let on_enable = move |_| {
        if enable_pending.get_untracked() {
            return;
        }

        set_enable_pending.set(true);
        set_enable_error.set(None);

        spawn_local(async move {
            match api::sync::enable().await {
                Ok(()) => match api::sync::status().await {
                    Ok(envelope) => {
                        sync_state.set(settle_indicator(&envelope));
                        apply_view(view_from(&envelope));
                        set_load_error.set(None);
                    }
                    Err(error) => set_load_error.set(Some(error_message(&error))),
                },
                Err(error) => set_enable_error.set(Some(error_message(&error))),
            }
            set_enable_pending.set(false);
        });
    };

    view! {
        {move || {
            if let Some(detail) = load_error.get() {
                return view! {
                    <div class="bg-ctp-red/10 border border-ctp-red rounded p-4 mt-6">
                        <p class="text-ctp-red font-semibold">"Sync status unavailable"</p>
                        <p class="text-ctp-subtext0 text-sm mt-2">{detail}</p>
                    </div>
                }
                    .into_any();
            }

            let Some(current) = state.get() else {
                return view! { <p class="mt-6 text-ctp-subtext0">"Loading sync…"</p> }
                    .into_any();
            };

            match current {
                SyncView::Initialized(data) => {
                    view! {
                        <div class="mt-6 border-t border-ctp-surface1 pt-4">
                            <h3 class="text-lg font-semibold text-ctp-text">"Sync"</h3>
                            <p class="text-ctp-subtext0 text-sm mt-2">{remote_line(&data)}</p>
                            <p class="text-ctp-subtext0 text-sm mt-1">{last_synced_line(&data)}</p>

                            <div class="mt-3">
                                <div class="flex items-center gap-3">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || use_remote.get()
                                        on:change=move |ev| {
                                            use_remote.set(event_target_checked(&ev))
                                        }
                                        class="h-4 w-4 accent-ctp-blue"
                                    />
                                    <span class="text-ctp-text">"Use a remote"</span>
                                </div>

                                <span class="font-medium text-ctp-text">"Remote URL"</span>
                                <input
                                    type="text"
                                    placeholder="https://example.com/context.git"
                                    prop:value=move || remote_draft.get()
                                    on:input=move |ev| remote_draft.set(event_target_value(&ev))
                                    disabled=move || !use_remote.get()
                                    class="mt-2 w-full rounded-lg border border-ctp-surface1 bg-ctp-surface0 px-4 py-2 text-ctp-text placeholder-ctp-subtext0 focus:border-ctp-blue focus:ring-2 focus:ring-ctp-blue focus:outline-none disabled:opacity-50 disabled:cursor-not-allowed"
                                />
                                <p class="mt-1 text-xs text-ctp-overlay0">
                                    "Local-only sync — leave the box unchecked to keep sync local."
                                </p>
                                <div class="mt-2">
                                    <button
                                        type="button"
                                        disabled=move || {
                                            remote_pending.get() || !remote_action_ready()
                                        }
                                        on:click=on_remote_action
                                        class="rounded-lg border border-ctp-surface1 px-4 py-2 font-medium text-ctp-text transition-colors hover:border-ctp-blue disabled:opacity-50 disabled:cursor-not-allowed"
                                    >
                                        {move || {
                                            if remote_pending.get() {
                                                view! { <Spinner size=SpinnerSize::Small/> }
                                                    .into_any()
                                            } else if use_remote.get() {
                                                view! { "Update remote" }.into_any()
                                            } else {
                                                view! { "Remove remote" }.into_any()
                                            }
                                        }}
                                    </button>
                                </div>

                                {move || {
                                    remote_error
                                        .get()
                                        .map(|detail| {
                                            view! {
                                                <div class="bg-ctp-red/10 border border-ctp-red rounded p-4 mt-4">
                                                    <p class="text-ctp-red font-semibold">
                                                        "Remote update failed"
                                                    </p>
                                                    <p class="text-ctp-subtext0 text-sm mt-2">
                                                        {detail}
                                                    </p>
                                                </div>
                                            }
                                        })
                                }}
                            </div>

                            <p class="mt-3 text-xs text-ctp-overlay0">
                                "Run a sync from the sync icon in the header."
                            </p>

                            <div class="mt-4">
                                <button
                                    type="button"
                                    title="Disable sync — pauses sync without deleting your data or git history"
                                    disabled=move || disable_pending.get()
                                    on:click=on_disable
                                    class="rounded-lg border border-ctp-red px-4 py-2 font-medium text-ctp-red transition-colors hover:bg-ctp-red/20 disabled:opacity-50 disabled:cursor-not-allowed"
                                >
                                    {move || {
                                        if disable_pending.get() {
                                            view! { <Spinner size=SpinnerSize::Small/> }.into_any()
                                        } else if confirming_disable.get() {
                                            view! {
                                                "Confirm disable — pauses sync; your data and git history stay in place"
                                            }
                                                .into_any()
                                        } else {
                                            view! { "Disable sync" }.into_any()
                                        }
                                    }}
                                </button>

                                {move || {
                                    confirming_disable
                                        .get()
                                        .then(|| {
                                            view! {
                                                <button
                                                    type="button"
                                                    on:click=move |_| set_confirming_disable.set(false)
                                                    class="ml-3 text-xs text-ctp-subtext1 transition-colors hover:text-ctp-text"
                                                >
                                                    "Cancel"
                                                </button>
                                            }
                                        })
                                }}
                            </div>

                            {move || {
                                disable_error
                                    .get()
                                    .map(|detail| {
                                        view! {
                                            <div class="bg-ctp-red/10 border border-ctp-red rounded p-4 mt-4">
                                                <p class="text-ctp-red font-semibold">"Disable failed"</p>
                                                <p class="text-ctp-subtext0 text-sm mt-2">{detail}</p>
                                            </div>
                                        }
                                    })
                            }}
                        </div>
                    }
                        .into_any()
                }
                SyncView::Disabled(data) => {
                    view! {
                        <div class="mt-6 border-t border-ctp-surface1 pt-4">
                            <h3 class="text-lg font-semibold text-ctp-text">"Sync"</h3>
                            <p class="text-ctp-subtext0 text-sm mt-2">
                                "Sync is disabled — your data and git history are intact."
                            </p>
                            <p class="text-ctp-subtext0 text-sm mt-1">{remote_line(&data)}</p>
                            <p class="text-ctp-subtext0 text-sm mt-1">{last_synced_line(&data)}</p>

                            <div class="mt-4">
                                <button
                                    type="button"
                                    disabled=move || enable_pending.get()
                                    on:click=on_enable
                                    class="rounded-lg border border-ctp-blue px-4 py-2 font-medium text-ctp-text transition-colors hover:bg-ctp-blue/20 disabled:opacity-50 disabled:cursor-not-allowed"
                                >
                                    {move || {
                                        if enable_pending.get() {
                                            view! { <Spinner size=SpinnerSize::Small/> }.into_any()
                                        } else {
                                            view! { "Enable sync" }.into_any()
                                        }
                                    }}
                                </button>
                            </div>

                            {move || {
                                enable_error
                                    .get()
                                    .map(|detail| {
                                        view! {
                                            <div class="bg-ctp-red/10 border border-ctp-red rounded p-4 mt-4">
                                                <p class="text-ctp-red font-semibold">"Enable failed"</p>
                                                <p class="text-ctp-subtext0 text-sm mt-2">{detail}</p>
                                            </div>
                                        }
                                    })
                            }}
                        </div>
                    }
                        .into_any()
                }
                SyncView::Uninitialized => {
                    view! {
                        <div class="mt-6 border-t border-ctp-surface1 pt-4">
                            <h3 class="text-lg font-semibold text-ctp-text">"Sync"</h3>
                            <p class="text-ctp-subtext0 text-sm mt-1">
                                "Sync keeps a git directory of the stored entities. Set it up once;"
                                " the header icon then runs a sync."
                            </p>

                            <div class="mt-3">
                                <span class="font-medium text-ctp-text">"Remote URL"</span>
                                <input
                                    type="text"
                                    placeholder="https://example.com/context.git"
                                    prop:value=move || remote.get()
                                    on:input=move |ev| remote.set(event_target_value(&ev))
                                    class="mt-2 w-full rounded-lg border border-ctp-surface1 bg-ctp-surface0 px-4 py-2 text-ctp-text placeholder-ctp-subtext0 focus:border-ctp-blue focus:ring-2 focus:ring-ctp-blue focus:outline-none"
                                />
                                <p class="mt-1 text-xs text-ctp-overlay0">
                                    "Leave it empty to set up a local-only sync."
                                </p>
                            </div>

                            <div class="mt-4">
                                <button
                                    type="button"
                                    disabled=move || pending.get()
                                    on:click=on_setup
                                    class="rounded-lg border border-ctp-surface1 px-4 py-2 font-medium text-ctp-text transition-colors hover:border-ctp-blue disabled:opacity-50 disabled:cursor-not-allowed"
                                >
                                    {move || {
                                        if pending.get() {
                                            view! { <Spinner size=SpinnerSize::Small/> }.into_any()
                                        } else {
                                            view! { "Set up sync" }.into_any()
                                        }
                                    }}
                                </button>
                            </div>

                            {move || {
                                setup_error
                                    .get()
                                    .map(|detail| {
                                        view! {
                                            <div class="bg-ctp-red/10 border border-ctp-red rounded p-4 mt-4">
                                                <p class="text-ctp-red font-semibold">"Set up failed"</p>
                                                <p class="text-ctp-subtext0 text-sm mt-2">{detail}</p>
                                            </div>
                                        }
                                    })
                            }}
                        </div>
                    }
                        .into_any()
                }
            }
        }}
    }
}

// =============================================================================
// Support
// =============================================================================

/// Channel name the pre-release checkbox stands for.
const PRE_RELEASE_CHANNEL: &str = "pre-release";
/// Channel name the unchecked checkbox stands for.
const STABLE_CHANNEL: &str = "stable";
/// How long the service gets to advance the check before each status read.
const CHECK_SETTLE_DELAY: std::time::Duration = std::time::Duration::from_secs(2);
/// Envelope status the server sends before the first init.
const NOT_INITIALIZED: &str = "not_initialized";

/// The shape of the sync section a status envelope selects.
#[derive(Debug, Clone)]
enum SyncView {
    /// Sync has no repository yet: the section offers the setup.
    Uninitialized,
    /// Sync is set up: the section reports where it syncs and when it last did.
    Initialized(SyncStatusData),
    /// Sync is set up but paused on this machine: the section offers the resume and nothing that
    /// touches the stored data or the remote.
    Disabled(SyncStatusData),
}

/// Report a dropped manual check on the pill.
///
/// The page renders no status: the pill owns that. A failure the client cannot turn into a payload
/// is still an answer to the click, so it takes the pill's failure view with the client's own
/// wording — the reject causes name the reason, such as a check that is already running.
fn report_check_failure(outcome: RwSignal<Option<UpdateCheckOutcome>>, error: &ApiClientError) {
    outcome.set(Some(UpdateCheckOutcome::Failed(error.to_string())));
}

/// Stored channel the checkbox drafts.
fn channel_draft(pre_release: bool) -> String {
    if pre_release {
        PRE_RELEASE_CHANNEL.to_string()
    } else {
        STABLE_CHANNEL.to_string()
    }
}

/// Stored values with every gap filled by the built-in default.
fn effective_values(values: &SettingsValues, defaults: &SettingsDefaults) -> SettingsDefaults {
    SettingsDefaults {
        host: values.host.clone().unwrap_or_else(|| defaults.host.clone()),
        port: values.port.unwrap_or(defaults.port),
        verbosity: values.verbosity.unwrap_or(defaults.verbosity),
        enable_docs: values.enable_docs.unwrap_or(defaults.enable_docs),
        skills_dir: values
            .skills_dir
            .clone()
            .unwrap_or_else(|| defaults.skills_dir.clone()),
        start_at_login: values.start_at_login.unwrap_or(defaults.start_at_login),
        auto_update: values.auto_update.unwrap_or(defaults.auto_update),
        update_channel: values
            .update_channel
            .clone()
            .unwrap_or_else(|| defaults.update_channel.clone()),
    }
}

/// Stored fields the running process has not picked up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RestartFields {
    host: bool,
    port: bool,
    verbosity: bool,
    enable_docs: bool,
    skills_dir: bool,
}

impl RestartFields {
    /// Compare the effective stored values with what the process runs.
    ///
    /// A fixed host is never read from the file, so it never waits for a restart.
    fn between(stored: &SettingsDefaults, running: &SettingsRunning) -> Self {
        Self {
            host: !running.host_fixed && stored.host != running.host,
            port: stored.port != running.port,
            verbosity: stored.verbosity != running.verbosity,
            enable_docs: stored.enable_docs != running.enable_docs,
            skills_dir: stored.skills_dir != running.skills_dir,
        }
    }

    /// Names of the fields awaiting a restart, in form order.
    fn labels(self) -> Vec<&'static str> {
        let mut labels = Vec::new();
        if self.host {
            labels.push("host");
        }
        if self.port {
            labels.push("port");
        }
        if self.verbosity {
            labels.push("verbosity");
        }
        if self.enable_docs {
            labels.push("enable docs");
        }
        if self.skills_dir {
            labels.push("skills directory");
        }
        labels
    }
}

/// The hint line of a row that falls back to its built-in default.
fn default_line(value: Signal<String>) -> Signal<String> {
    Signal::derive(move || format!("default: {}", value.get()))
}

/// A string override, or `None` when the draft equals the default.
fn override_string(draft: String, default: &str) -> Option<String> {
    (draft != default).then_some(draft)
}

/// A port override, or `None` when the draft equals the default.
fn override_u16(draft: u16, default: u16) -> Option<u16> {
    (draft != default).then_some(draft)
}

/// A verbosity override, or `None` when the draft equals the default.
fn override_u8(draft: u8, default: u8) -> Option<u8> {
    (draft != default).then_some(draft)
}

/// A boolean override, or `None` when the draft equals the default.
fn override_bool(draft: bool, default: bool) -> Option<bool> {
    (draft != default).then_some(draft)
}

/// Logging level name of `verbosity`.
fn verbosity_label(verbosity: u8) -> &'static str {
    match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    }
}

/// Message to show the user for a failed save.
///
/// A 400 carries the field-naming text the server wrote; other errors keep
/// their client-side description.
fn error_message(error: &ApiClientError) -> String {
    match error {
        ApiClientError::Server(server) => server.error.clone(),
        other => other.to_string(),
    }
}

/// Read the status as the view the sync section renders.
async fn fetch_view() -> Result<SyncView, String> {
    let envelope = api::sync::status()
        .await
        .map_err(|error| error_message(&error))?;

    Ok(view_from(&envelope))
}

/// Map a status envelope to the view the section renders.
///
/// The envelope reports uninitialized twice, through its status token and through the payload, so
/// either one selects the setup view. A status without a payload knows no stored state either. An
/// initialized sync that reports itself disabled on this machine selects the paused view: the
/// directory stays, so the section never offers the setup for it.
fn view_from(envelope: &SyncResponseEnvelope) -> SyncView {
    match &envelope.data {
        Some(data) if data.initialized && envelope.status != NOT_INITIALIZED => {
            if data.enabled == Some(false) {
                SyncView::Disabled(data.clone())
            } else {
                SyncView::Initialized(data.clone())
            }
        }
        _ => SyncView::Uninitialized,
    }
}

/// Line naming the remote sync pushes to.
fn remote_line(data: &SyncStatusData) -> String {
    match data.remote_url.as_deref() {
        Some(url) => format!("Remote: {url}"),
        None => "Remote: not set".to_string(),
    }
}

/// Line naming the last completed sync, or that none completed yet.
fn last_synced_line(data: &SyncStatusData) -> String {
    match data.last_synced_at.as_deref() {
        Some(at) => format!("Last synced: {at}"),
        None => "Last synced: never".to_string(),
    }
}
