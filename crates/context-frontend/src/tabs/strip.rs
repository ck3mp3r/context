//! The two-level strip: level one holds the project tabs and the parentless row; level two holds
//! the children of the active project only (decisions 2, 5, and 19).
//!
//! A tab chip holds two siblings: the activate control that fills the chip, and the `×` close control
//! beside it. The close control is never a descendant of the activate control, so a close click
//! cannot also activate the tab it removes — the mechanism that blanked the content area of the first
//! build.
//!
//! Close writes the registry only. The shell's one URL writer names whatever tab survives, and chrome
//! home when none does, so no close path navigates. The one navigation here is the `+` control, which
//! is decision 16's route to the Projects chrome view.

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use super::open::write_active_url;
use super::pure::{Tab, TabId, TabKey, TabRouteKind};
use super::registry::TabRegistry;
use super::session::TabTitles;

/// The `MouseEvent::button` value of a middle click (decision 9).
const MIDDLE_BUTTON: i16 = 1;

// region:    --- Components

/// The tab strip. It hides itself while no tab is open, so chrome home renders unchanged.
#[component]
pub fn TabStrip(registry: TabRegistry, titles: TabTitles) -> impl IntoView {
    let rows = Signal::derive(move || registry.strip_rows());
    let projects = Signal::derive(move || rows.get().projects);
    let parentless = Signal::derive(move || rows.get().parentless);
    let children = Signal::derive(move || rows.get().children);

    let navigate = use_navigate();
    let open_projects = move |_| write_active_url(&navigate, "/");

    view! {
        <div
            data-strip=""
            class="flex flex-col gap-1 border-b border-ctp-surface1 bg-ctp-mantle px-4 py-2"
            style=move || strip_style(!registry.is_empty())
        >
            <div data-strip-level="1" class="flex items-center gap-2 overflow-x-auto">
                <Show when=move || !projects.get().is_empty()>
                    <div data-strip-row="projects" class="flex items-center gap-1">
                        <For
                            each=move || projects.get()
                            key=|id| *id
                            children=move |id| tab_chip(registry, titles, id)
                        />
                    </div>
                </Show>
                <Show when=move || !parentless.get().is_empty()>
                    <div data-strip-row="parentless" class="flex items-center gap-1">
                        <For
                            each=move || parentless.get()
                            key=|id| *id
                            children=move |id| tab_chip(registry, titles, id)
                        />
                    </div>
                </Show>
            </div>
            <Show when=move || !children.get().is_empty()>
                <div data-strip-level="2" class="flex items-center gap-2 overflow-x-auto">
                    <div data-strip-row="children" class="flex items-center gap-1 pl-4">
                        <For
                            each=move || children.get()
                            key=|id| *id
                            children=move |id| tab_chip(registry, titles, id)
                        />
                    </div>
                </div>
            </Show>
            <button
                type="button"
                data-strip-add=""
                class="self-start px-3 py-1 rounded-lg text-sm font-medium text-ctp-subtext1 hover:bg-ctp-surface1 hover:text-ctp-text transition-colors"
                title="New tab"
                aria-label="New tab"
                on:click=open_projects
            >
                "+"
            </button>
        </div>
    }
}

/// One tab chip: the activate control and, beside it, the `×` close control.
///
/// The chip itself closes on a middle click (decision 9). A middle click activates nothing, so the
/// close never shares a control with the activate path.
#[component]
fn StripTab(registry: TabRegistry, titles: TabTitles, id: TabId, tab_key: TabKey) -> impl IntoView {
    let active = Signal::derive(move || registry.active() == Some(id));
    let kind = kind_token(tab_key.kind());
    let label = move || chip_label(&titles, &tab_key);

    let on_activate = move |_| {
        registry.activate(id);
    };
    let on_close = move |_| {
        registry.close(id);
    };
    let on_auxclick = move |event: leptos::ev::MouseEvent| {
        if event.button() == MIDDLE_BUTTON {
            event.prevent_default();
            registry.close(id);
        }
    };

    view! {
        <div
            data-strip-tab=id.to_string()
            data-strip-kind=kind
            data-strip-active=move || active_token(active.get())
            class=move || chip_class(active.get())
            on:auxclick=on_auxclick
        >
            <button
                type="button"
                data-tab-activate=id.to_string()
                class="px-3 py-1 text-sm font-medium text-left truncate max-w-[16rem]"
                on:click=on_activate
            >
                {label}
            </button>
            <button
                type="button"
                data-tab-close=id.to_string()
                class="px-2 py-1 text-sm text-ctp-overlay1 hover:text-ctp-red rounded-r-lg"
                title="Close tab"
                aria-label="Close tab"
                on:click=on_close
            >
                "×"
            </button>
        </div>
    }
}

// endregion: --- Components

// region:    --- Support

/// The chip of one strip id, or nothing when the id names no open tab.
fn tab_chip(registry: TabRegistry, titles: TabTitles, id: TabId) -> AnyView {
    match open_tab(registry, id) {
        Some(tab) => view! {
            <StripTab registry=registry titles=titles id=id tab_key=tab.key().clone()/>
        }
        .into_any(),
        None => ().into_any(),
    }
}

/// The open tab with this id.
fn open_tab(registry: TabRegistry, id: TabId) -> Option<Tab> {
    registry.tabs().into_iter().find(|tab| tab.id() == id)
}

/// The chip's label: the resolved title, or the identity when no title has arrived yet.
///
/// A tab opened in this session has no title until the label resolve answers, so the fallback keeps
/// the chip readable and clickable instead of blank.
fn chip_label(titles: &TabTitles, key: &TabKey) -> String {
    match titles.label(key).filter(|label| !label.trim().is_empty()) {
        Some(label) => label,
        None => format!("{} {}", kind_title(key.kind()), key.entity_id()),
    }
}

/// The readable name of a surface kind, for the label fallback.
fn kind_title(kind: TabRouteKind) -> &'static str {
    match kind {
        TabRouteKind::Project => "Project",
        TabRouteKind::TaskList => "Task list",
        TabRouteKind::Note => "Note",
        TabRouteKind::Repo => "Repo",
    }
}

/// The DOM token a chip's `data-strip-kind` carries.
fn kind_token(kind: TabRouteKind) -> &'static str {
    match kind {
        TabRouteKind::Project => "project",
        TabRouteKind::TaskList => "task-list",
        TabRouteKind::Note => "note",
        TabRouteKind::Repo => "repo",
    }
}

/// The chip's `data-strip-active` token.
fn active_token(active: bool) -> &'static str {
    if active { "true" } else { "false" }
}

/// The chip's classes: the active chip reads as raised surface.
fn chip_class(active: bool) -> &'static str {
    if active {
        "flex items-stretch rounded-lg bg-ctp-surface2 text-ctp-text"
    } else {
        "flex items-stretch rounded-lg bg-ctp-surface0 text-ctp-subtext1 hover:bg-ctp-surface1 hover:text-ctp-text transition-colors"
    }
}

/// The strip's `display`: an empty strip is chrome home, and chrome renders no strip.
fn strip_style(open: bool) -> &'static str {
    if open { "display:flex" } else { "display:none" }
}

// endregion: --- Support
