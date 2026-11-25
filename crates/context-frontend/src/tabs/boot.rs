//! The runtime half of the tab session: storage access, the boot sequence, and label resolution.
//!
//! `session.rs` holds the schema and the rules. This module touches `localStorage` and the API, so it
//! is not part of the host test binary; the browser pins in `tests/e2e.rs` cover it.

use gloo_net::http::Request;
use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::Effect;
use serde::Deserialize;

use crate::api;

use super::open::open_surface;
use super::pure::TabKey;
use super::registry::TabRegistry;
use super::session::{
    BootPlan, BootSurface, StoredKey, StoredSession, TabTitles, decode, encode, environment,
    install_restored, plan_boot, snapshot, storage_key,
};

// region:    --- Storage

/// The stored session, or `None` when storage is denied, absent, corrupt, or stale (decision 14).
pub fn load() -> Option<StoredSession> {
    let raw = read_raw()?;
    decode(&raw)
}

/// Store the session, or nothing when the page has no usable storage.
pub fn save(session: &StoredSession) {
    let Some(raw) = encode(session) else {
        return;
    };
    write_raw(&raw);
}

/// Store the session on every change, so a reload rebuilds the workspace the user left (decision 14).
///
/// One effect over the registry and the labels: every open, close, activation, and resolved label
/// writes the session once. It is installed after the boot restore, so the first write stores the
/// restored workspace rather than an empty one.
pub fn start_session_persistence(tabs: TabRegistry, titles: TabTitles) {
    Effect::new(move |_| {
        save(&snapshot(tabs, titles));
    });
}

// endregion: --- Storage

// region:    --- Boot

/// Boot: read the session once, restore or open the boot surface, and return the identities whose
/// labels still need resolving.
///
/// Nothing here writes a URL. The one writer adopts the boot URL as the URL it owns, so a restore
/// repeats no navigation and cannot overwrite a deep link (decisions 11 and 14).
pub fn restore_or_boot(
    tabs: TabRegistry,
    titles: TabTitles,
    boot: Option<BootSurface>,
) -> Vec<TabKey> {
    let session = load();
    let plan = plan_boot(session.as_ref(), boot.as_ref());

    let Some(session) = session else {
        return match boot {
            Some(boot) => vec![open_boot(tabs, &boot)],
            None => Vec::new(),
        };
    };

    let mut unresolved = install_restored(tabs, &session, focus_of(&plan));
    for stored in &session.tabs {
        titles.set(stored.key.key(), stored.title.clone());
    }

    if let BootPlan::RestorePlusBoot { boot } = plan {
        unresolved.push(open_boot(tabs, &boot));
    }
    unresolved
}

/// The surface the plan focuses, when it focuses one.
fn focus_of(plan: &BootPlan) -> Option<&StoredKey> {
    match plan {
        BootPlan::Restore { focus } => focus.as_ref(),
        BootPlan::Fresh | BootPlan::RestorePlusBoot { .. } => None,
    }
}

/// Open the surface a boot URL names and focus it, so a deep link keeps its own tab (decisions 8
/// and 14).
fn open_boot(tabs: TabRegistry, boot: &BootSurface) -> TabKey {
    let key = boot.key.key();
    open_surface(tabs, key.clone(), boot.parent.as_deref());
    key
}

// endregion: --- Boot

// region:    --- Labels

/// Resolve one label per restored tab, and drop the surfaces the server no longer has.
///
/// One GET per identity. A 404 means the entity is gone, so its tab is closed without a page reload
/// (decision 14). Any other failure keeps the cached label, so a flaky network never closes a tab.
pub async fn resolve_titles(tabs: TabRegistry, titles: TabTitles, keys: Vec<TabKey>) {
    for key in keys {
        match fetch_label(&key).await {
            LabelOutcome::Found(title) => titles.set(key, title),
            LabelOutcome::Gone => drop_tab(tabs, &key),
            LabelOutcome::Unresolved => {}
        }
    }
}

/// What one label request found.
enum LabelOutcome {
    /// The surface exists: its label.
    Found(String),
    /// The surface is gone: the request answered 404.
    Gone,
    /// Any other outcome: no label, and the tab stays.
    Unresolved,
}

/// One GET for the label of one surface.
async fn fetch_label(key: &TabKey) -> LabelOutcome {
    let url = format!("{}/{}", api::base(), label_path(key));
    let Ok(response) = Request::get(&url).send().await else {
        return LabelOutcome::Unresolved;
    };

    match response.status() {
        200..=299 => match label_payload(key, response).await {
            Some(label) => LabelOutcome::Found(label),
            None => LabelOutcome::Unresolved,
        },
        404 => LabelOutcome::Gone,
        _ => LabelOutcome::Unresolved,
    }
}

/// The label a payload holds: a project, task list, and note carry a title, a repo carries its remote.
async fn label_payload(key: &TabKey, response: gloo_net::http::Response) -> Option<String> {
    match key {
        TabKey::Repo { .. } => response.json::<RepoPayload>().await.ok().map(|repo| repo.remote),
        TabKey::Project { .. } | TabKey::TaskList { .. } | TabKey::Note { .. } => response
            .json::<TitledPayload>()
            .await
            .ok()
            .map(|titled| titled.title),
    }
}

/// The API path of one surface's record.
fn label_path(key: &TabKey) -> String {
    match key {
        TabKey::Project { project_id } => format!("projects/{project_id}"),
        TabKey::TaskList { task_list_id } => format!("task-lists/{task_list_id}"),
        TabKey::Note { note_id } => format!("notes/{note_id}"),
        TabKey::Repo { repo_id } => format!("repos/{repo_id}"),
    }
}

/// Close the tab of a surface the server no longer has. A cascade follows the model's close rule.
fn drop_tab(tabs: TabRegistry, key: &TabKey) {
    if let Some((id, _)) = tabs
        .descriptors()
        .into_iter()
        .find(|(_, open)| open == key)
    {
        tabs.close(id);
    }
}

/// A payload that carries a title.
#[derive(Deserialize)]
struct TitledPayload {
    title: String,
}

/// A repo payload, whose label is its remote.
#[derive(Deserialize)]
struct RepoPayload {
    remote: String,
}

// endregion: --- Labels

// region:    --- Support

/// The stored session string, or `None` when this page cannot read storage.
fn read_raw() -> Option<String> {
    if !storage_available() {
        return None;
    }
    LocalStorage::raw()
        .get_item(&storage_key(environment()))
        .ok()
        .flatten()
}

/// Write the session string, or nothing when this page cannot write storage.
fn write_raw(raw: &str) {
    if !storage_available() {
        return;
    }
    let _ = LocalStorage::raw().set_item(&storage_key(environment()), raw);
}

/// Whether this page can use `localStorage`.
///
/// A denied store must not break boot: `LocalStorage::raw` throws when the browser refuses it, so
/// every read and write asks this first and falls back to chrome (decision 14).
fn storage_available() -> bool {
    web_sys::window()
        .and_then(|window| window.local_storage().ok())
        .flatten()
        .is_some()
}

// endregion: --- Support
