//! The keep-alive container: one stable wrapper per open tab.
//!
//! Decision 12: the wrapper is one keyed element in a list, hidden with `display:none` on itself,
//! and a tab's dialogs and overlays render inside that wrapper rather than under `document.body`.
//! Decision 13: a hidden tab stays mounted, so a switch hides and never unmounts.
//!
//! The wrapper is also the observation surface every later pin asserts against: `data-tab-id` is
//! the tab's id, and `data-tab-active` is `"true"` on the active wrapper only.

use leptos::prelude::*;

use super::pure::{TabId, TabKey, TabRouteKind};
use super::registry::TabRegistry;
use crate::pages::{NoteDetail, ProjectDetail, RepoDetail, TaskListDetail};

// region:    --- Types

/// The tab a wrapper renders for: its identity and whether it is the active tab.
///
/// A tab surface reads this through [`use_tab_scope`] instead of the route params, because one
/// route can mount several tabs and a hidden tab has no route of its own.
#[derive(Clone, Debug)]
pub struct TabScope {
    id: TabId,
    key: TabKey,
    parent: Option<String>,
    active: Signal<bool>,
}

impl TabScope {
    /// Bundle a tab's identity with the reactive visibility of its wrapper.
    pub fn new(id: TabId, key: TabKey, parent: Option<String>, active: Signal<bool>) -> Self {
        Self {
            id,
            key,
            parent,
            active,
        }
    }

    /// The tab's id, as it appears in `data-tab-id`.
    pub fn id(&self) -> TabId {
        self.id
    }

    /// The tab's identity: surface kind plus entity id.
    pub fn key(&self) -> &TabKey {
        &self.key
    }

    /// The surface kind this tab shows.
    pub fn kind(&self) -> TabRouteKind {
        self.key.kind()
    }

    /// The id of the entity this tab shows.
    pub fn entity_id(&self) -> &str {
        self.key.entity_id()
    }

    /// The project whose child row holds this tab, when the surface names one.
    pub fn parent_project(&self) -> Option<&str> {
        match self.key.kind() {
            TabRouteKind::Project => Some(self.key.entity_id()),
            TabRouteKind::TaskList | TabRouteKind::Note | TabRouteKind::Repo => {
                self.parent.as_deref()
            }
        }
    }

    /// Whether this tab is the active one, which is what its wrapper's `display` follows.
    pub fn is_active(&self) -> bool {
        self.active.get()
    }
}

/// The tab scope of the wrapper a view renders in, or `None` outside the container.
pub fn use_tab_scope() -> Option<TabScope> {
    use_context::<TabScope>()
}

// endregion: --- Types

// region:    --- Components

/// The keep-alive container: one keyed wrapper per open tab, in strip order.
///
/// The key is the tab id, so opening or closing a tab never rebuilds the wrappers that survive it.
#[component]
pub fn TabContainer(registry: TabRegistry) -> impl IntoView {
    view! {
        <div class="flex flex-col" data-tab-container="">
            <For
                each=move || {
                    registry
                        .tabs()
                        .into_iter()
                        .map(|tab| (tab.id(), tab.key().clone()))
                }
                key=|(id, _)| *id
                children=move |(id, tab_key)| {
                    view! { <TabWrapper registry=registry id=id tab_key=tab_key/> }
                }
            />
        </div>
    }
}

/// One tab's wrapper: the tab's fixed mount point, hidden while the tab is not active.
///
/// The wrapper always renders, so a restored tab has its place in the strip and the container order
/// before it is ever opened. Its surface renders only once the tab is mounted: a tab opened here
/// mounts at once, a restored tab mounts on its first activation (decisions 13 and 14).
#[component]
fn TabWrapper(registry: TabRegistry, id: TabId, tab_key: TabKey) -> impl IntoView {
    let active = Signal::derive(move || registry.active() == Some(id));
    // A memo, not a derived signal: the mount view must re-render only when the flag changes, or
    // every registry update would remount the surface and throw its state away (decision 13).
    let mounted = Memo::new(move |_| registry.mounted(id));
    let parent = registry
        .tabs()
        .into_iter()
        .find(|tab| tab.id() == id)
        .and_then(|tab| tab.parent_project().map(str::to_string));
    provide_context(TabScope::new(id, tab_key, parent, active));

    view! {
        <div
            data-tab-id=id.to_string()
            data-tab-active=move || active_label(active.get())
            style=move || wrapper_style(active.get())
        >
            {move || mounted.get().then(|| view! { <TabMount/> })}
        </div>
    }
}

/// The mount slot every tab surface renders into.
///
/// The surface reads its identity from [`TabScope`], never from route params: one route can mount
/// several tabs, and a hidden tab has no route of its own.
#[component]
fn TabMount() -> impl IntoView {
    let scope = use_tab_scope();
    let kind = scope
        .as_ref()
        .map(|scope| kind_name(scope.kind()))
        .unwrap_or_default();
    let entity_id = scope
        .as_ref()
        .map(|scope| scope.entity_id().to_string())
        .unwrap_or_default();

    view! {
        <div
            class="h-full"
            data-tab-mount=""
            data-tab-view=kind
            data-tab-entity-id=entity_id
        >
            {scope.map(surface_view)}
        </div>
    }
}

/// The real surface of one tab, chosen by its kind.
fn surface_view(scope: TabScope) -> AnyView {
    match scope.kind() {
        TabRouteKind::Project => view! { <ProjectDetail scope=scope/> }.into_any(),
        TabRouteKind::TaskList => view! { <TaskListDetail scope=scope/> }.into_any(),
        TabRouteKind::Note => view! { <NoteDetail scope=scope/> }.into_any(),
        TabRouteKind::Repo => view! { <RepoDetail scope=scope/> }.into_any(),
    }
}

// endregion: --- Components

// region:    --- Support

/// The DOM token a wrapper's view uses for its surface kind.
fn kind_name(kind: TabRouteKind) -> &'static str {
    match kind {
        TabRouteKind::Project => "project",
        TabRouteKind::TaskList => "task-list",
        TabRouteKind::Note => "note",
        TabRouteKind::Repo => "repo",
    }
}

/// The wrapper's `display`: a hidden tab stays mounted, it only stops painting (decision 13).
fn wrapper_style(active: bool) -> &'static str {
    if active {
        "display:block"
    } else {
        "display:none"
    }
}

/// The wrapper's `data-tab-active` token.
fn active_label(active: bool) -> &'static str {
    if active { "true" } else { "false" }
}

// endregion: --- Support
