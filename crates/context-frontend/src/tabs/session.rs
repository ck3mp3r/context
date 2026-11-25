//! The tab session: the stored schema, the boot plan, and the lazy restore (decision 14).
//!
//! Decision 14 keeps the tab identity, the strip order, and the active tab. A label is cached beside
//! each identity and re-resolved by id after boot, and no per-tab filter is stored. Restore is lazy:
//! only the active tab mounts at boot, and every other tab mounts on its first activation.
//!
//! This module is pure. It holds no DOM, no storage, and no request, so the host test binary compiles
//! it directly and pins the decode, plan, and restore rules without a browser. Storage access and the
//! boot sequence live in `boot.rs`.

use leptos::prelude::{Get, RwSignal, Update};
use leptos_router::NavigateOptions;
use serde::{Deserialize, Serialize};

use super::open::write_active_url;
use super::pure::{Tab, TabId, TabKey, TabRouteKind};
use super::registry::TabRegistry;

// region:    --- Session schema

/// The schema version this build reads and writes. A session of any other version is stale and drops
/// to chrome (decision 14).
pub const SESSION_VERSION: u32 = 1;

/// The storage key, namespaced per environment, so a dev build and a release build never collide.
pub fn storage_key(environment: &str) -> String {
    format!("c5t.tabs.session.{environment}.v1")
}

/// The environment this build stores under: `dev` for a debug build, `release` otherwise.
pub fn environment() -> &'static str {
    if cfg!(debug_assertions) { "dev" } else { "release" }
}

/// The surface kind of a stored tab, as the token the session writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredKind {
    Project,
    TaskList,
    Note,
    Repo,
}

impl StoredKind {
    /// The stored kind of this identity.
    pub fn of(key: &TabKey) -> Self {
        match key {
            TabKey::Project { .. } => Self::Project,
            TabKey::TaskList { .. } => Self::TaskList,
            TabKey::Note { .. } => Self::Note,
            TabKey::Repo { .. } => Self::Repo,
        }
    }

    /// The identity of this kind for `id`.
    pub fn key(self, id: impl Into<String>) -> TabKey {
        let id = id.into();
        match self {
            Self::Project => TabKey::project(id),
            Self::TaskList => TabKey::task_list(id),
            Self::Note => TabKey::note(id),
            Self::Repo => TabKey::repo(id),
        }
    }
}

/// The identity of one stored surface: kind plus entity id (decision 6).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredKey {
    pub kind: StoredKind,
    pub id: String,
}

impl StoredKey {
    /// The stored identity of this key.
    pub fn of(key: &TabKey) -> Self {
        Self {
            kind: StoredKind::of(key),
            id: key.entity_id().to_string(),
        }
    }

    /// The identity as the model names it.
    pub fn key(&self) -> TabKey {
        self.kind.key(self.id.clone())
    }
}

/// One stored tab: its identity, its cached label, and the project that holds its child row.
///
/// The parent is presentation linkage, not part of the identity (ruling L): a note that belongs to
/// two projects is still one tab, and its stored parent reproduces the attachment the strip row and
/// the close cascade need. Restoring it needs no fetch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredTab {
    pub key: StoredKey,
    pub title: String,
    pub parent: Option<String>,
}

/// The stored workspace: its schema version, its strip order, and its active surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredSession {
    pub version: u32,
    /// Every open tab in strip order (decision 14).
    pub tabs: Vec<StoredTab>,
    /// The active surface, or `None` when no tab was active.
    pub active: Option<StoredKey>,
}

impl StoredSession {
    /// Whether the session holds this identity.
    pub fn contains(&self, key: &StoredKey) -> bool {
        self.tabs.iter().any(|tab| &tab.key == key)
    }
}

// endregion: --- Session schema

// region:    --- Encode and decode

/// Read a stored session, or `None` when it is corrupt, stale, or empty (decision 14).
///
/// A `None` is the whole failure policy: boot falls to chrome, silently, with no error and no
/// reload.
pub fn decode(raw: &str) -> Option<StoredSession> {
    let stored: StoredSession = serde_json::from_str(raw).ok()?;
    if stored.version != SESSION_VERSION {
        return None;
    }
    let tabs = sanitize(stored.tabs);
    if tabs.is_empty() {
        return None;
    }
    let active = stored
        .active
        .filter(|active| tabs.iter().any(|tab| &tab.key == active));
    Some(StoredSession {
        version: stored.version,
        tabs,
        active,
    })
}

/// The stored form of a session, or `None` when it cannot be written.
pub fn encode(session: &StoredSession) -> Option<String> {
    serde_json::to_string(session).ok()
}

/// Keep the first tab of each identity and drop the tabs a session cannot mean.
///
/// Identity is unique (decision 6), so a repeated identity keeps its first entry, which is also the
/// attachment the model keeps for a note that belongs to two projects. A tab with no entity id
/// cannot be restored.
fn sanitize(tabs: Vec<StoredTab>) -> Vec<StoredTab> {
    let mut kept: Vec<StoredTab> = Vec::new();
    for tab in tabs {
        if tab.key.id.trim().is_empty() {
            continue;
        }
        if kept.iter().any(|open| open.key == tab.key) {
            continue;
        }
        kept.push(tab);
    }
    kept
}

// endregion: --- Encode and decode

// region:    --- Boot plan

/// The tab surface a boot URL names: its identity and the project the path hangs it under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootSurface {
    pub key: StoredKey,
    pub parent: Option<String>,
}

/// What boot does with the stored session (decision 14).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BootPlan {
    /// No session to restore: boot to chrome and open nothing.
    Fresh,
    /// Restore the strip and focus one stored surface: the boot URL's surface when the session holds
    /// it, otherwise the stored active surface.
    Restore { focus: Option<StoredKey> },
    /// Restore the strip, then open the surface the boot URL names and focus it.
    RestorePlusBoot { boot: BootSurface },
}

/// Decide what boot does, from the stored session and the surface the boot URL names.
pub fn plan_boot(session: Option<&StoredSession>, boot: Option<&BootSurface>) -> BootPlan {
    let Some(session) = session else {
        return BootPlan::Fresh;
    };
    match boot {
        Some(boot) if session.contains(&boot.key) => BootPlan::Restore {
            focus: Some(boot.key.clone()),
        },
        Some(boot) => BootPlan::RestorePlusBoot { boot: boot.clone() },
        None => BootPlan::Restore {
            focus: session.active.clone(),
        },
    }
}

/// The tab surface a boot URL names, or `None` for a chrome route.
///
/// The paths are the ones the single writer produces: a project, and a child under its project. A
/// chrome route (`/`, `/notes`, `/repos`, `/skills`, `/settings`) names no surface, so reloading a
/// chrome view restores the session without opening a tab for it.
pub fn boot_surface(pathname: &str) -> Option<BootSurface> {
    let segments: Vec<&str> = pathname
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let surface = |kind: StoredKind, id: &str, parent: Option<&str>| BootSurface {
        key: StoredKey {
            kind,
            id: id.to_string(),
        },
        parent: parent.map(str::to_string),
    };

    match segments.as_slice() {
        ["projects", project_id] => Some(surface(StoredKind::Project, project_id, None)),
        ["projects", project_id, "task-lists", task_list_id] => {
            Some(surface(StoredKind::TaskList, task_list_id, Some(project_id)))
        }
        ["projects", project_id, "notes", note_id] => {
            Some(surface(StoredKind::Note, note_id, Some(project_id)))
        }
        ["projects", project_id, "repos", repo_id] => {
            Some(surface(StoredKind::Repo, repo_id, Some(project_id)))
        }
        ["task-lists", task_list_id] => Some(surface(StoredKind::TaskList, task_list_id, None)),
        ["notes", note_id] => Some(surface(StoredKind::Note, note_id, None)),
        ["repos", repo_id] => Some(surface(StoredKind::Repo, repo_id, None)),
        _ => None,
    }
}

// endregion: --- Boot plan

// region:    --- Restore

/// Install a stored strip into the registry and focus it (decision 14).
///
/// Every tab lands in stored order with its stored attachment, so a note that belongs to two projects
/// comes back attached to the project it was opened from, without a fetch. `focus` names the surface
/// to activate; when it names no installed tab the first tab takes focus, because a strip with no
/// active tab would render every tab hidden.
///
/// No restored tab is mounted except the focused one, so boot costs no surface fetch (decision 14).
/// A tab whose entity is gone is dropped later, by the label resolve that finds a 404, so the
/// restored strip is the stored one and the drop needs no reload.
pub fn install_restored(
    tabs: TabRegistry,
    session: &StoredSession,
    focus: Option<&StoredKey>,
) -> Vec<TabKey> {
    let mut installed = Vec::new();
    for stored in &session.tabs {
        let key = stored.key.key();
        let parent = match key.kind() {
            TabRouteKind::Project => None,
            TabRouteKind::TaskList | TabRouteKind::Note | TabRouteKind::Repo => stored.parent.clone(),
        };
        tabs.restore(key.clone(), parent);
        installed.push(key);
    }

    let target = focus
        .and_then(|focus| find_id(tabs, &focus.key()))
        .or_else(|| installed.first().and_then(|key| find_id(tabs, key)));
    if let Some(id) = target {
        tabs.activate(id);
    }
    installed
}

/// The session to store: every open tab in strip order, with its label and its attachment.
pub fn snapshot(tabs: TabRegistry, titles: TabTitles) -> StoredSession {
    let labels = titles.labels();
    let stored = tabs
        .tabs()
        .iter()
        .map(|tab| StoredTab {
            key: StoredKey::of(tab.key()),
            title: label_of(&labels, tab.key()),
            parent: attachment(tab),
        })
        .collect();

    StoredSession {
        version: SESSION_VERSION,
        tabs: stored,
        active: tabs.active_key().map(|key| StoredKey::of(&key)),
    }
}

/// The attachment a tab stores: a project tab's parent project is itself, so it stores none.
fn attachment(tab: &Tab) -> Option<String> {
    match tab.key().kind() {
        TabRouteKind::Project => None,
        TabRouteKind::TaskList | TabRouteKind::Note | TabRouteKind::Repo => {
            tab.parent_project().map(str::to_string)
        }
    }
}

/// The cached label of this identity, or the empty label when none is known yet.
fn label_of(labels: &[(TabKey, String)], key: &TabKey) -> String {
    labels
        .iter()
        .find(|(labelled, _)| labelled == key)
        .map(|(_, title)| title.clone())
        .unwrap_or_default()
}

/// The id of the tab holding this identity, when one is open.
fn find_id(tabs: TabRegistry, key: &TabKey) -> Option<TabId> {
    tabs.descriptors()
        .into_iter()
        .find(|(_, open)| open == key)
        .map(|(id, _)| id)
}

// endregion: --- Restore

// region:    --- URL ownership

/// The one URL writer, gated by the URL the app already owns.
///
/// `owned` is the URL the address bar held at boot, read once (decision 11). A write happens only
/// when the target differs, so a restore repeats no navigation and never overwrites a deep link
/// (decision 14).
pub fn write_owned_url(writer: &impl Fn(&str, NavigateOptions), owned: &mut String, url: &str) {
    if owned == url {
        return;
    }
    write_active_url(writer, url);
    owned.clear();
    owned.push_str(url);
}

// endregion: --- URL ownership

// region:    --- Titles

/// The label of every open tab, by identity: what the strip renders and what the session stores.
///
/// A restore fills it from the cached labels before any request lands, so a rebuilt strip shows its
/// titles at once, and the label resolve replaces each one with the label the server holds.
#[derive(Clone, Copy, Debug)]
pub struct TabTitles {
    entries: RwSignal<Vec<(TabKey, String)>>,
}

impl TabTitles {
    /// Start with no label.
    pub fn new() -> Self {
        Self {
            entries: RwSignal::new(Vec::new()),
        }
    }

    /// Every label, in the order the labels were set.
    pub fn labels(&self) -> Vec<(TabKey, String)> {
        self.entries.try_get().unwrap_or_default()
    }

    /// The label of this identity, when one is known.
    pub fn label(&self, key: &TabKey) -> Option<String> {
        self.labels()
            .into_iter()
            .find(|(labelled, _)| labelled == key)
            .map(|(_, title)| title)
    }

    /// Whether a label for this identity is known: a blank label names no surface, so the open path
    /// fetches one.
    pub fn has_label(&self, key: &TabKey) -> bool {
        self.label(key)
            .is_some_and(|label| !label.trim().is_empty())
    }

    /// Label an identity, replacing an earlier label.
    pub fn set(&self, key: TabKey, title: String) {
        self.entries
            .update(|entries| match entries.iter_mut().find(|(labelled, _)| *labelled == key) {
                Some((_, labelled)) => *labelled = title,
                None => entries.push((key, title)),
            });
    }
}

impl Default for TabTitles {
    fn default() -> Self {
        Self::new()
    }
}

// endregion: --- Titles
