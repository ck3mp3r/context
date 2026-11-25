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
    let parent = match key.kind() {
        TabRouteKind::Project => None,
        TabRouteKind::TaskList | TabRouteKind::Note | TabRouteKind::Repo => {
            project_id.filter(|project| !project.is_empty())
        }
    };
    let Some(parent) = parent else {
        return focus_or_open(registry, key);
    };
    focus_or_open(registry, TabKey::project(parent));
    registry.open_in(key, parent)
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
