//! The strip: level one holds the project tabs and the parentless row, level two holds the children
//! of the active project only, the notes bar holds the notes that name no project, and the repos bar
//! holds the repos that name no project (decisions 2, 5, and 19).
//!
//! The strip is route-aware. A chrome-global view (`/skills`, `/settings`) hides it entirely. The
//! projects overview (`/`) renders row one only: the opened project tabs, with no active chip and no
//! row two, because no tab is active there. The notes home (`/notes`) renders the notes bar only, and
//! the repos home (`/repos`) renders the repos bar only, again with no active chip. A tab surface
//! renders the bar the active tab sits in when it sits in one, and the project rows otherwise.
//!
//! A tab chip holds two siblings: the activate control that fills the chip, and the `×` close control
//! beside it. The close control is never a descendant of the activate control, so a close click
//! cannot also activate the tab it removes — the mechanism that blanked the content area of the first
//! build.
//!
//! Close writes the registry only. The shell's one URL writer names whatever tab survives, and chrome
//! home when none does, so no close path navigates, and no control here navigates either.

use leptos::prelude::*;
use leptos_router::hooks::use_location;

use super::pure::{Tab, TabId, TabKey, TabRouteKind};
use super::registry::TabRegistry;
use super::route::{RouteKind, route_kind};
use super::session::TabTitles;

/// The `MouseEvent::button` value of a middle click (decision 9).
const MIDDLE_BUTTON: i16 = 1;

// region:    --- Components

/// The tab strip. Its route decides whether it shows, and which rows it shows.
#[component]
pub fn TabStrip(registry: TabRegistry, titles: TabTitles) -> impl IntoView {
    let location = use_location();
    let kind = Signal::derive(move || route_kind(&location.pathname.get()));

    let rows = Signal::derive(move || registry.strip_rows());
    let projects = Signal::derive(move || rows.get().projects);
    let notes = Signal::derive(move || rows.get().notes);
    let repos = Signal::derive(move || rows.get().repos);
    let parentless = Signal::derive(move || rows.get().parentless);
    let children = Signal::derive(move || rows.get().children);

    // The notes bar belongs to the notes system: the notes home shows it alone, and a tab surface
    // shows it exactly when the active tab sits in it.
    let notes_shown = Signal::derive(move || match kind.get() {
        RouteKind::NotesHome => !notes.get().is_empty(),
        RouteKind::TabSurface | RouteKind::Unknown => registry
            .active()
            .is_some_and(|active| notes.get().contains(&active)),
        RouteKind::ProjectsHome | RouteKind::ReposHome | RouteKind::ChromeGlobal => false,
    });

    // The repos bar belongs to the repos system, by the same rule as the notes bar.
    let repos_shown = Signal::derive(move || match kind.get() {
        RouteKind::ReposHome => !repos.get().is_empty(),
        RouteKind::TabSurface | RouteKind::Unknown => registry
            .active()
            .is_some_and(|active| repos.get().contains(&active)),
        RouteKind::ProjectsHome | RouteKind::NotesHome | RouteKind::ChromeGlobal => false,
    });

    // The project rows are the other half of the one bar: a tab surface whose active tab is a free
    // note or a free repo shows that bar instead, so the two never paint together.
    let projects_shown = Signal::derive(move || match kind.get() {
        RouteKind::ProjectsHome => true,
        RouteKind::TabSurface | RouteKind::Unknown => !notes_shown.get() && !repos_shown.get(),
        RouteKind::NotesHome | RouteKind::ReposHome | RouteKind::ChromeGlobal => false,
    });

    // Row two belongs to the active project, and neither home has an active tab, so it renders a
    // tab surface alone. Every chrome-global view hides the strip, and keeps every tab open.
    let second_row = Signal::derive(move || {
        projects_shown.get() && matches!(kind.get(), RouteKind::TabSurface | RouteKind::Unknown)
    });
    let shown = Signal::derive(move || match kind.get() {
        RouteKind::ChromeGlobal => false,
        RouteKind::NotesHome => notes_shown.get(),
        RouteKind::ReposHome => repos_shown.get(),
        RouteKind::ProjectsHome => !projects.get().is_empty(),
        RouteKind::TabSurface | RouteKind::Unknown => !registry.is_empty(),
    });

    view! {
        <div
            data-strip=""
            class="flex flex-col gap-1 border-b border-ctp-surface1 bg-ctp-mantle px-4 py-2"
            style=move || strip_style(shown.get())
        >
            <div data-strip-level="1" class="flex items-center gap-2 overflow-x-auto">
                <Show when=move || notes_shown.get()>
                    <div data-strip-row="notes" class="flex items-center gap-1">
                        <For
                            each=move || notes.get()
                            key=|id| *id
                            children=move |id| tab_chip(registry, titles, id)
                        />
                    </div>
                </Show>
                <Show when=move || repos_shown.get()>
                    <div data-strip-row="repos" class="flex items-center gap-1">
                        <For
                            each=move || repos.get()
                            key=|id| *id
                            children=move |id| tab_chip(registry, titles, id)
                        />
                    </div>
                </Show>
                <Show when=move || projects_shown.get() && !projects.get().is_empty()>
                    <div data-strip-row="projects" class="flex items-center gap-1">
                        <For
                            each=move || projects.get()
                            key=|id| *id
                            children=move |id| tab_chip(registry, titles, id)
                        />
                    </div>
                </Show>
                <Show when=move || second_row.get() && !parentless.get().is_empty()>
                    <div data-strip-row="parentless" class="flex items-center gap-1">
                        <For
                            each=move || parentless.get()
                            key=|id| *id
                            children=move |id| tab_chip(registry, titles, id)
                        />
                    </div>
                </Show>
            </div>
            <Show when=move || second_row.get() && !children.get().is_empty()>
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
        </div>
    }
}

/// One tab chip: the activate control and, beside it, the `×` close control.
///
/// The chip itself closes on a middle click (decision 9). A middle click activates nothing, so the
/// close never shares a control with the activate path.
#[component]
fn StripTab(registry: TabRegistry, titles: TabTitles, id: TabId, tab_key: TabKey) -> impl IntoView {
    let project = match &tab_key {
        TabKey::Project { project_id } => Some(project_id.clone()),
        TabKey::TaskList { .. } | TabKey::Note { .. } | TabKey::Repo { .. } => None,
    };
    let is_project = project.is_some();
    let active = Signal::derive(move || registry.active() == Some(id));
    // The breadcrumb's second reading: this chip is the project that owns the active task-list
    // child. It is its own attribute, never a second meaning of `data-strip-active`, so "where am I"
    // and "whose parent is that" stay apart.
    let ancestor = Signal::derive({
        let project = project.clone();
        move || {
            let Some(project) = project.as_deref() else {
                return false;
            };
            registry
                .active_key()
                .is_some_and(|key| key.kind() == TabRouteKind::TaskList)
                && registry.active_project().as_deref() == Some(project)
        }
    });
    let kind = kind_token(tab_key.kind());
    let label = move || tab_label(&titles, &tab_key);

    let activate_project = project.clone();
    let on_activate = move |_| match activate_project.as_deref() {
        Some(project) => {
            registry.activate_project(project);
        }
        None => {
            registry.activate(id);
        }
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
            data-tab-ancestor=move || ancestor_token(ancestor.get())
            class=chip_class(is_project)
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

/// The label of one tab: the resolved title, or the identity when no title has arrived yet.
///
/// A tab opened in this session has no title until the label resolve answers, so the fallback keeps
/// the chip readable and clickable instead of blank. The window title reads this same rule, so the
/// chip and the window never disagree about a tab.
pub(crate) fn tab_label(titles: &TabTitles, key: &TabKey) -> String {
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

/// The chip's `data-tab-ancestor` token: present only on the project chip that owns the active
/// task-list child, so the attribute marks the parent and never the location.
fn ancestor_token(ancestor: bool) -> Option<&'static str> {
    ancestor.then_some("true")
}

/// The chip's classes: a project chip keeps its own base surface and a child chip its own, so row
/// one reads as projects and row two as children.
///
/// The active and ancestor states are attribute selectors in `input.css`, so no state enters this
/// class and a chip's base colour never depends on it.
fn chip_class(project: bool) -> &'static str {
    if project {
        "tab-chip tab-chip--project"
    } else {
        "tab-chip tab-chip--child"
    }
}

/// The strip's `display`: a hidden strip paints nothing, and every tab stays open behind it.
fn strip_style(shown: bool) -> &'static str {
    if shown {
        "display:flex"
    } else {
        "display:none"
    }
}

// endregion: --- Support
