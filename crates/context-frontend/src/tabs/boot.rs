//! The runtime half of the tab session: storage access, the boot sequence, and label resolution.
//!
//! `session.rs` holds the schema and the rules. This module touches `localStorage` and the API, so it
//! is not part of the host test binary; the browser pins in `tests/e2e.rs` cover it.

use gloo_net::http::Request;
use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::Effect;
use leptos::task::spawn_local;
use serde::Deserialize;

use crate::api;

use super::open::open_card;
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

/// What boot leaves pending once the stored session is installed: the labels the restore owes, and
/// the surface the boot URL names.
///
/// The boot surface is not opened at once, because a `/notes/{note_id}` deep link must learn whether
/// the note is a subnote before it may open a tab. [`finish_boot`] settles it.
pub struct BootPending {
    /// The labels the restored strip still needs.
    pub labels: Vec<TabKey>,
    /// The surface to open, when the plan opens one.
    pub surface: Option<BootSurface>,
}

/// Boot: read the session once and install it, and report the labels to resolve and the surface to
/// open.
///
/// Nothing here writes a URL. The one writer adopts the boot URL as the URL it owns, so a restore
/// repeats no navigation and cannot overwrite a deep link (decisions 11 and 14).
pub fn restore_or_boot(
    tabs: TabRegistry,
    titles: TabTitles,
    boot: Option<BootSurface>,
) -> BootPending {
    let session = load();
    let plan = plan_boot(session.as_ref(), boot.as_ref());

    let Some(session) = session else {
        return BootPending {
            labels: Vec::new(),
            surface: boot,
        };
    };

    let labels = install_restored(tabs, &session, focus_of(&plan));
    for stored in &session.tabs {
        titles.set(stored.key.key(), stored.title.clone());
    }

    let surface = match plan {
        BootPlan::RestorePlusBoot { boot } => Some(boot),
        BootPlan::Fresh | BootPlan::Restore { .. } => None,
    };
    BootPending { labels, surface }
}

/// Finish the boot once the page can await: resolve the labels the restore owes, then open and label
/// the surface the boot URL names, when that surface may open.
pub async fn finish_boot(tabs: TabRegistry, titles: TabTitles, pending: BootPending) {
    resolve_titles(tabs, titles, pending.labels).await;
    let Some(surface) = pending.surface else {
        return;
    };
    let labels = open_boot(tabs, titles, &surface).await;
    resolve_titles(tabs, titles, labels).await;
}

/// The surface the plan focuses, when it focuses one.
fn focus_of(plan: &BootPlan) -> Option<&StoredKey> {
    match plan {
        BootPlan::Restore { focus } => focus.as_ref(),
        BootPlan::Fresh | BootPlan::RestorePlusBoot { .. } => None,
    }
}

/// Open the surface a boot URL names and focus it, so a deep link keeps its own tab, and report the
/// labels to resolve: the parent project tab this boot opens first, then the boot surface (decisions
/// 8 and 14).
///
/// The pair comes from `open_card`, so a boot and a card click enqueue the same labels by the same
/// rule and cannot drift. A `/notes/{note_id}` deep link opens nothing when the note is a subnote:
/// only a top-level note has a tab, and a subnote's home is the tab of its parent note.
async fn open_boot(tabs: TabRegistry, titles: TabTitles, boot: &BootSurface) -> Vec<TabKey> {
    let key = boot.key.key();
    match boot_note(&key, boot.parent.as_deref()).await {
        BootNote::Skipped => return Vec::new(),
        BootNote::Labelled(label) => titles.set(key.clone(), label),
        BootNote::Unlabelled => {}
    }
    open_card(tabs, key, boot.parent.as_deref(), &|key| {
        titles.has_label(key)
    })
}

/// What the boot note fetch decided about a `/notes/{note_id}` deep link.
enum BootNote {
    /// The note names a parent, so boot opens no tab for it.
    Skipped,
    /// The note is top-level: its label came with the fetch, so no second request is needed.
    Labelled(String),
    /// No usable payload: open the tab, and let the label resolve ask again.
    Unlabelled,
}

/// Whether a boot surface may open, fetching the note of a `/notes/{note_id}` deep link.
///
/// Only a top-level note has a tab, so a note that names a parent opens none. A note under a project
/// path and every other surface are eligible with no fetch. A failed request or a non-200 answer
/// keeps the old behavior: the tab opens, and the label resolve decides what the surface is.
async fn boot_note(key: &TabKey, parent: Option<&str>) -> BootNote {
    let (TabKey::Note { note_id }, None) = (key, parent) else {
        return BootNote::Unlabelled;
    };
    let Some(note) = fetch_note(note_id).await else {
        return BootNote::Unlabelled;
    };
    if note.parent_id.is_some() {
        return BootNote::Skipped;
    }
    BootNote::Labelled(note.title)
}

/// One GET for the note a boot URL names, so the gate can read its parent.
async fn fetch_note(note_id: &str) -> Option<BootNotePayload> {
    let url = format!("{}/notes/{note_id}", api::base());
    let response = Request::get(&url).send().await.ok()?;
    if !(200..=299).contains(&response.status()) {
        return None;
    }
    response.json::<BootNotePayload>().await.ok()
}

/// A note payload, as the boot gate reads it: the label, and the parent that makes it a subnote.
#[derive(Deserialize)]
struct BootNotePayload {
    title: String,
    parent_id: Option<String>,
}

// endregion: --- Boot

// region:    --- Labels

/// Resolve one label per restored tab, and drop the surfaces the server no longer has.
///
/// One GET per identity. A 404 means the entity is gone, so its tab is closed without a page reload
/// (decision 14). Any other failure keeps the cached label, so a flaky network never closes a tab.
pub async fn resolve_titles(tabs: TabRegistry, titles: TabTitles, keys: Vec<TabKey>) {
    for key in keys {
        resolve_title(tabs, titles, key).await;
    }
}

/// Open the surface a card click names, and resolve the labels it opened without one.
///
/// A card click and a boot open report their labels through the same `open_card` rule, so the parent
/// project tab is fetched once, by whichever path opened it. The resolves run in the order `open_card`
/// names them, the parent project tab before the surface it holds, and on the local task queue, so
/// the click handler never waits for one.
pub fn open_from_card(tabs: TabRegistry, titles: TabTitles, key: TabKey, project_id: Option<&str>) {
    let pending = open_card(tabs, key, project_id, &|key| titles.has_label(key));
    if pending.is_empty() {
        return;
    }
    spawn_local(async move { resolve_titles(tabs, titles, pending).await });
}

/// Resolve one label: a 2xx labels the tab, a 404 closes it, any other failure keeps the fallback.
async fn resolve_title(tabs: TabRegistry, titles: TabTitles, key: TabKey) {
    match fetch_label(&key).await {
        LabelOutcome::Found(title) => titles.set(key, title),
        LabelOutcome::Gone => drop_tab(tabs, &key),
        LabelOutcome::Unresolved => {}
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
        TabKey::Repo { .. } => response
            .json::<RepoPayload>()
            .await
            .ok()
            .map(|repo| repo.remote),
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
    if let Some((id, _)) = tabs.descriptors().into_iter().find(|(_, open)| open == key) {
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
