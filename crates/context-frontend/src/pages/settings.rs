use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{self, ApiClientError};
use crate::components::{Breadcrumb, BreadcrumbItem};
use crate::models::{SettingsDefaults, SettingsPayload, SettingsRunning, SettingsValues};

/// `/settings` page: edit the stored configuration, defaults in view.
#[component]
pub fn Settings() -> impl IntoView {
    view! {
        <div class="flex flex-col min-h-[calc(100vh-8rem)]">
            <Breadcrumb items=vec![BreadcrumbItem::new("Settings")]/>

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

    // One payload update refreshes the draft and the restart markers.
    let apply_payload = move |payload: SettingsPayload| {
        let effective = effective_values(&payload.values, &payload.defaults);

        host.set(effective.host);
        port.set(effective.port.to_string());
        verbosity.set(effective.verbosity);
        enable_docs.set(effective.enable_docs);
        skills_dir.set(effective.skills_dir);
        start_at_login.set(effective.start_at_login);

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

// =============================================================================
// Support
// =============================================================================

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
