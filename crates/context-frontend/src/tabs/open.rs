//! The open path: focus-or-open one surface, its parent project first, and the one URL writer.
//!
//! `focus_or_open` is the identity rule of decision 6. `open_surface` adds the parent-tab rule of
//! decisions 7 and 8 on top of it. `surface_url` is the only place that turns a tab into a URL,
//! and `write_active_url` is the only writer.

use leptos_router::NavigateOptions;

use super::pure::{TabId, TabKey, TabRouteKind};
use super::registry::TabRegistry;

/// Focus the tab that already holds `key`, or open it and focus it (decision 6).
///
/// The lookup runs over the registry's descriptors, so an open surface is never duplicated. The
/// returned id is the existing tab's id in that case.
pub fn focus_or_open(registry: TabRegistry, key: TabKey) -> TabId {
    match registry
        .descriptors()
        .into_iter()
        .find(|(_, open)| open == &key)
    {
        Some((id, _)) => {
            registry.activate(id);
            id
        }
        None => registry.open(key),
    }
}

/// Open a surface and focus it, with its parent project tab open first (decisions 7 and 8).
///
/// A surface that names no project opens parentless. A child names the project it hangs under, so
/// that project's tab opens before the child and the child attaches to it (ruling L).
pub fn open_surface(registry: TabRegistry, key: TabKey, project_id: Option<&str>) -> TabId {
    let Some(parent) = open_parent(&key, project_id) else {
        return focus_or_open(registry, key);
    };
    focus_or_open(registry, parent.clone());
    registry.open_in(key, parent.entity_id())
}

/// Open the surface a card click names, and report the labels the caller must resolve: the parent
/// project tab this click opens first, then the clicked surface.
///
/// `labelled` is the session's answer for an identity. A surface the session already labels costs no
/// request, and a parent that the click only focuses stays quiet, because its label was enqueued when
/// it opened. So re-activating a tab and re-opening a surface with a known label issue nothing
/// (decision 6).
///
/// An unlabelled open asks for one fetch. A fetch that fails for any reason other than 404 stores no
/// label, so clicking the same card again asks once more: the retry is bounded by a click and
/// self-healing, never automatic.
pub fn open_card(
    registry: TabRegistry,
    key: TabKey,
    project_id: Option<&str>,
    labelled: &impl Fn(&TabKey) -> bool,
) -> Vec<TabKey> {
    let parent = open_parent(&key, project_id)
        .filter(|parent| !is_open(registry, parent) && !labelled(parent));
    let clicked = (!labelled(&key)).then(|| key.clone());

    open_surface(registry, key, project_id);

    parent.into_iter().chain(clicked).collect()
}

/// The parent project identity this surface opens with, when it names one (decisions 7 and 8).
fn open_parent(key: &TabKey, project_id: Option<&str>) -> Option<TabKey> {
    match key.kind() {
        TabRouteKind::Project => None,
        TabRouteKind::TaskList | TabRouteKind::Note | TabRouteKind::Repo => project_id
            .filter(|project| !project.is_empty())
            .map(TabKey::project),
    }
}

/// Whether this identity is already open, so its label was enqueued when it opened.
fn is_open(registry: TabRegistry, key: &TabKey) -> bool {
    registry
        .descriptors()
        .into_iter()
        .any(|(_, open)| &open == key)
}

/// The URL of one surface: its identity and the project it hangs under (decisions 11 and 20).
///
/// No list state enters this URL, so a tab switch never writes a page, a query, or a sort order.
pub fn surface_url(key: &TabKey, project_id: Option<&str>) -> String {
    let (segment, entity_id) = match key {
        TabKey::Project { project_id } => return format!("/projects/{project_id}"),
        TabKey::TaskList { task_list_id } => ("task-lists", task_list_id),
        TabKey::Note { note_id } => ("notes", note_id),
        TabKey::Repo { repo_id } => ("repos", repo_id),
    };
    match project_id.filter(|project| !project.is_empty()) {
        Some(project) => format!("/projects/{project}/{segment}/{entity_id}"),
        None => format!("/{segment}/{entity_id}"),
    }
}

/// The only URL writer: one `replace` navigation, so no history entry is created
/// (decisions 11 and 21).
pub fn write_active_url(writer: &impl Fn(&str, NavigateOptions), url: &str) {
    writer(
        url,
        NavigateOptions {
            replace: true,
            ..Default::default()
        },
    );
}
