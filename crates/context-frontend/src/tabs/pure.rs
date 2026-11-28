//! The tab model: identity, close, cycle, and strip rows over plain structs.
//!
//! This module is pure. It holds no DOM, no wasm, and no reactive runtime, so the host test binary
//! compiles it directly and pins every rule without a browser. The reactive holder lives in
//! `registry.rs`.
//!
//! The rules are decisions 2, 5, 6, 8, 9, 10, and 19 of the tabs rebuild design (note `5b3f7b47`),
//! plus predecessor ruling L: identity is the surface kind plus its entity id, and the owning
//! project travels beside the identity as per-tab linkage, never inside it.

use std::collections::HashMap;

/// The surface a tab shows.
///
/// Decision 4: these four surfaces are tabs. The projects home, the notes and repos lists, skills,
/// skill detail, and settings are chrome and never open a tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TabRouteKind {
    Project,
    TaskList,
    Note,
    Repo,
}

/// Tab identity: the surface kind plus its entity id (decision 6).
///
/// Two tabs share an identity exactly when their keys are equal, so the registry cannot hold a
/// duplicate surface. The owning project is not part of the identity: a note that belongs to two
/// projects is still one tab. That linkage lives beside the key, on [`Tab`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TabKey {
    Project { project_id: String },
    TaskList { task_list_id: String },
    Note { note_id: String },
    Repo { repo_id: String },
}

impl TabKey {
    /// Key of a project dashboard.
    pub fn project(project_id: impl Into<String>) -> Self {
        Self::Project {
            project_id: project_id.into(),
        }
    }

    /// Key of a task list.
    pub fn task_list(task_list_id: impl Into<String>) -> Self {
        Self::TaskList {
            task_list_id: task_list_id.into(),
        }
    }

    /// Key of a note.
    pub fn note(note_id: impl Into<String>) -> Self {
        Self::Note {
            note_id: note_id.into(),
        }
    }

    /// Key of a repo.
    pub fn repo(repo_id: impl Into<String>) -> Self {
        Self::Repo {
            repo_id: repo_id.into(),
        }
    }

    /// The surface kind this key names.
    pub fn kind(&self) -> TabRouteKind {
        match self {
            Self::Project { .. } => TabRouteKind::Project,
            Self::TaskList { .. } => TabRouteKind::TaskList,
            Self::Note { .. } => TabRouteKind::Note,
            Self::Repo { .. } => TabRouteKind::Repo,
        }
    }

    /// The id of the surface itself.
    pub fn entity_id(&self) -> &str {
        match self {
            Self::Project { project_id } => project_id,
            Self::TaskList { task_list_id } => task_list_id,
            Self::Note { note_id } => note_id,
            Self::Repo { repo_id } => repo_id,
        }
    }
}

/// Identifier of an open tab.
///
/// Ids come from a counter inside [`Tabs`], so a closed tab's id never belongs to a later tab. That
/// is what makes a stale id detectable: `activate` and `close` either match an open tab or change
/// nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(u64);

impl Default for TabId {
    /// The detached id. No registry ever issues id 0, so activating or closing it changes nothing.
    fn default() -> Self {
        Self(0)
    }
}

impl std::fmt::Display for TabId {
    /// The id as a DOM token, so a wrapper's `data-tab-id` is the tab's id.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// One open tab: its id, its identity, and the project it is attached to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tab {
    id: TabId,
    key: TabKey,
    /// The project whose child row holds this tab, when the surface names one (ruling L).
    parent: Option<String>,
    /// Whether the tab's surface has been mounted. A tab opened here mounts at once; a restored tab
    /// mounts on its first activation (decisions 13 and 14).
    mounted: bool,
}

impl Tab {
    /// The tab's id, stable for the tab's whole life.
    pub fn id(&self) -> TabId {
        self.id
    }

    /// The tab's identity: kind plus entity id.
    pub fn key(&self) -> &TabKey {
        &self.key
    }

    /// Whether the tab's surface is mounted. It never turns off: a mounted tab hides, it never
    /// unmounts (decision 13).
    pub fn is_mounted(&self) -> bool {
        self.mounted
    }

    /// The project whose child row and cycle group hold this tab, or `None` when it has no
    /// project.
    ///
    /// A project tab's parent project is the project it shows, so row 2 lists its children while
    /// the project itself is active (decision 19). A task list, note, or repo attaches to a project
    /// when its data names one (decision 8, ruling L); a note or a repo that names no project sits in
    /// its own bar, and every other surface that names no project sits in the parentless row
    /// (decision 5).
    pub fn parent_project(&self) -> Option<&str> {
        match self.key() {
            TabKey::Project { project_id } => Some(project_id),
            TabKey::TaskList { .. } | TabKey::Note { .. } | TabKey::Repo { .. } => {
                self.parent.as_deref()
            }
        }
    }
}

/// What [`Tabs::close`] did, so the caller can react to a last close or to a stale id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloseOutcome {
    /// The id named no open tab. The registry is unchanged.
    NotOpen,
    /// These tabs were removed and `active` was moved to its new value in the same update.
    Closed {
        ids: Vec<TabId>,
        active: Option<TabId>,
    },
}

/// The strip of decision 2, the notes bar, the repos bar, plus the derived active project of
/// decision 19.
///
/// Row 1 is `projects` followed by `parentless`; row 2 is `children`; `notes` is the notes bar;
/// `repos` is the repos bar. The rows are disjoint: a tab attached to a project is only ever in a
/// child row, a note that names no project is only ever in the notes row, a repo that names no
/// project is only ever in the repos row, and every other tab that names no project is only ever
/// in the parentless row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StripRows {
    /// Level 1: open project tabs, in strip order.
    pub projects: Vec<TabId>,
    /// The notes bar: open note tabs that name no project, in strip order.
    pub notes: Vec<TabId>,
    /// The repos bar: open repo tabs that name no project, in strip order.
    pub repos: Vec<TabId>,
    /// Level 1: the parentless row, meaning the open free-form tabs that name no project, in strip
    /// order.
    pub parentless: Vec<TabId>,
    /// The project the active tab belongs to, or `None` when the active tab has no project.
    pub active_project: Option<String>,
    /// Level 2: the tabs attached to `active_project`, in strip order.
    pub children: Vec<TabId>,
}

impl StripRows {
    /// Whether any row or a bar holds `id`, so an active tab is never rendered nowhere.
    pub fn contains(&self, id: TabId) -> bool {
        self.projects.contains(&id)
            || self.notes.contains(&id)
            || self.repos.contains(&id)
            || self.parentless.contains(&id)
            || self.children.contains(&id)
    }
}

/// The open tabs and the active one, in strip order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tabs {
    /// Open tabs in strip order: the order they were opened in.
    tabs: Vec<Tab>,
    /// The focused tab, or `None` on a chrome route.
    active: Option<TabId>,
    /// Source of the next [`TabId`]. It only ever grows, so ids never repeat.
    next_id: u64,
    /// The last selected surface of each project, keyed by project id.
    ///
    /// `active` names one surface, so it cannot answer "what was this project showing before?".
    /// This map holds that answer: the project's own tab or one of its task-list children. Notes
    /// and repos move focus without recording. It lives here, beside the tabs, so a close drops
    /// the entry of every tab it removes, and it is RAM-only: it never reaches the session.
    remembered_surface: HashMap<String, TabId>,
}

impl Tabs {
    /// Every open tab, in strip order.
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Number of open tabs.
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// Whether no tab is open, which is chrome home.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// The focused tab id, or `None` on a chrome route.
    pub fn active(&self) -> Option<TabId> {
        self.active
    }

    /// The tab with this id, when it is open.
    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|tab| tab.id == id)
    }

    /// Whether this id names an open tab.
    pub fn contains(&self, id: TabId) -> bool {
        self.tab(id).is_some()
    }

    /// The id of the tab holding this identity, when one is open.
    pub fn find(&self, key: &TabKey) -> Option<TabId> {
        self.tabs.iter().find(|tab| &tab.key == key).map(Tab::id)
    }

    /// The identity of the active tab.
    pub fn active_key(&self) -> Option<&TabKey> {
        self.active.and_then(|id| self.tab(id)).map(Tab::key)
    }

    /// The project the active tab belongs to (decision 19). Derived on every call, never stored.
    pub fn active_project(&self) -> Option<&str> {
        self.active
            .and_then(|id| self.tab(id))
            .and_then(Tab::parent_project)
    }

    /// Open a surface that names no project, and focus it.
    ///
    /// The caller reads the surface's data: an empty project list means the tab is parentless and
    /// opens here, while a named project means [`Tabs::open_in`].
    ///
    /// A surface that is already open is focused, not duplicated (decision 6). The returned id is
    /// the existing tab's id in that case.
    pub fn open(&mut self, key: TabKey) -> TabId {
        self.insert(key, None)
    }

    /// Open a surface inside `project_id`, and focus it.
    ///
    /// The tab becomes a child of that project (ruling L): row 2 lists it while the project is
    /// active, the cycle ring groups it with that project's children, and closing the project
    /// removes it (decision 9).
    ///
    /// Attachment is set once. Opening an already open surface focuses it and keeps its first
    /// attachment, so a note that belongs to two projects stays one tab (decision 6) and closes
    /// with the project it was opened from.
    pub fn open_in(&mut self, key: TabKey, project_id: &str) -> TabId {
        self.insert(key, Some(project_id.to_string()))
    }

    /// Insert or focus a tab, attaching it when it has no attachment yet.
    fn insert(&mut self, key: TabKey, parent: Option<String>) -> TabId {
        let id = match self.find(&key) {
            Some(id) => {
                if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id)
                    && tab.parent.is_none()
                {
                    tab.parent = parent;
                }
                id
            }
            None => {
                self.next_id += 1;
                let id = TabId(self.next_id);
                self.tabs.push(Tab {
                    id,
                    key,
                    parent,
                    mounted: true,
                });
                id
            }
        };
        self.focus(Some(id));
        id
    }

    /// Open a stored surface without focusing it and without mounting it (decision 14).
    ///
    /// The tab lands in strip order and stays unmounted until its first activation, so a restore
    /// mounts only the active tab and the other tabs fetch nothing until they are opened. The caller
    /// focuses the restored active tab afterwards, which mounts it.
    pub fn restore(&mut self, key: TabKey, parent: Option<String>) -> TabId {
        if let Some(id) = self.find(&key) {
            return id;
        }
        self.next_id += 1;
        let id = TabId(self.next_id);
        self.tabs.push(Tab {
            id,
            key,
            parent,
            mounted: false,
        });
        id
    }

    /// Whether this id names an open tab whose surface is mounted.
    pub fn is_mounted(&self, id: TabId) -> bool {
        self.tab(id).is_some_and(Tab::is_mounted)
    }

    /// Focus an open tab, and mount it if it is not mounted yet.
    ///
    /// An id that names no open tab changes nothing, so `active` never points at a removed tab and
    /// no tab is ever hidden in favour of a tab that does not exist.
    pub fn activate(&mut self, id: TabId) -> bool {
        if !self.contains(id) {
            return false;
        }
        self.focus(Some(id));
        true
    }

    /// Clear focus and keep every tab open.
    ///
    /// Chrome navigation deactivates the strip: the tab set survives, no tab paints, and the chrome
    /// view renders alone, so exactly one surface shows. It returns whether it changed anything, so
    /// a strip that is already deactivated costs no update.
    pub fn deactivate(&mut self) -> bool {
        if self.active.is_none() {
            return false;
        }
        self.focus(None);
        true
    }

    /// Focus the surface a project chip names: the last selected surface of that project.
    ///
    /// The four-way rule of the breadcrumb. A project whose own tab is already focused stays
    /// there, so a second click never restores the child behind the user's back. A project whose
    /// task-list child is focused collapses to the overview, so the chip toggles between the two.
    /// Every other state restores the surface that project last selected, and falls back to the
    /// overview when that surface is not open.
    ///
    /// Only the project's own tab and its task lists are remembered, so a chip for a project whose
    /// open tab is a note or a repo restores the surface that project selected before it. A
    /// project id no open tab names focuses nothing, which is what an absent chip asks for.
    pub fn activate_project(&mut self, project_id: &str) -> Option<TabId> {
        let project = self.project_tab(project_id);
        if project.is_some() && self.active == project {
            return project;
        }
        let on_child_task_list = self.active.and_then(|id| self.tab(id)).is_some_and(|tab| {
            tab.key().kind() == TabRouteKind::TaskList && tab.parent_project() == Some(project_id)
        });
        let remembered = self
            .remembered_surface
            .get(project_id)
            .copied()
            .filter(|id| self.contains(*id));
        let target = match (project, on_child_task_list, remembered) {
            (Some(project), true, _) => Some(project),
            (Some(_), false, Some(remembered)) => Some(remembered),
            (Some(project), false, None) => Some(project),
            (None, _, remembered) => remembered,
        };
        if target.is_some() {
            self.focus(target);
        }
        target
    }

    /// Focus a tab and mount it, or clear focus with `None`.
    ///
    /// Every path that moves focus goes through here, so a tab the user reaches is never left
    /// unmounted and hidden: a restored tab mounts the moment it takes focus (decision 14). The
    /// last selected surface is recorded here too, so opening, activating, and cycling all hand
    /// their project the surface they just focused.
    fn focus(&mut self, id: Option<TabId>) {
        let mut remembered = None;
        if let Some(tab) = id.and_then(|id| self.tabs.iter_mut().find(|tab| tab.id == id)) {
            tab.mounted = true;
            if matches!(
                tab.key().kind(),
                TabRouteKind::Project | TabRouteKind::TaskList
            ) && let Some(project) = tab.parent_project()
            {
                remembered = Some((project.to_string(), tab.id));
            }
        }
        if let Some((project, surface)) = remembered {
            self.remembered_surface.insert(project, surface);
        }
        self.active = id;
    }

    /// The tabs a close of `id` removes: the tab itself, and for a project every tab attached to
    /// it — task lists, notes, and repos (ruling L, decision 9).
    ///
    /// Attachment is one link per tab, so a note that belongs to two projects closes with the
    /// project it was opened from and survives the close of the other one. Empty when `id` names no
    /// open tab.
    pub fn closing_set(&self, id: TabId) -> Vec<TabId> {
        let Some(tab) = self.tab(id) else {
            return Vec::new();
        };
        let cascade = match tab.key() {
            TabKey::Project { project_id } => Some(project_id.as_str()),
            TabKey::TaskList { .. } | TabKey::Note { .. } | TabKey::Repo { .. } => None,
        };
        self.tabs
            .iter()
            .filter(|candidate| {
                candidate.id == id || cascade.is_some_and(|project| is_child_of(candidate, project))
            })
            .map(Tab::id)
            .collect()
    }

    /// Close a tab, cascade a project's children, and move `active` in one update.
    ///
    /// A closed active tab hands focus to the next surviving tab in the cycle ring, so the close
    /// never leaves every tab hidden. The last close leaves no active tab: that is chrome home.
    pub fn close(&mut self, id: TabId) -> CloseOutcome {
        let closing = self.closing_set(id);
        if closing.is_empty() {
            return CloseOutcome::NotOpen;
        }
        let active = match self.active {
            Some(active) if closing.contains(&active) => self.survivor_after(&closing),
            active => active,
        };
        self.tabs.retain(|tab| !closing.contains(&tab.id));
        self.remembered_surface
            .retain(|_, surface| !closing.contains(surface));
        self.focus(active);
        CloseOutcome::Closed {
            ids: closing,
            active,
        }
    }

    /// The fixed cycle ring of decision 10.
    ///
    /// Level 1 comes first in strip order: the project tabs, then the tabs that name no project.
    /// Then each project's attached tabs follow, grouped by project order. The order never depends
    /// on the active tab, so stepping the ring cannot oscillate. A tab whose project tab is not
    /// open is appended in strip order, so every open tab appears exactly once.
    pub fn cycle_sequence(&self) -> Vec<TabId> {
        let projects: Vec<TabId> = self
            .tabs
            .iter()
            .filter(|tab| tab.key().kind() == TabRouteKind::Project)
            .map(Tab::id)
            .collect();

        let mut ring = projects.clone();
        ring.extend(
            self.tabs
                .iter()
                .filter(|tab| tab.parent_project().is_none())
                .map(Tab::id),
        );
        for project in &projects {
            let Some(project_id) = self.tab(*project).and_then(Tab::parent_project) else {
                continue;
            };
            ring.extend(
                self.tabs
                    .iter()
                    .filter(|tab| is_child_of(tab, project_id))
                    .map(Tab::id),
            );
        }
        ring.extend(
            self.tabs
                .iter()
                .filter(|tab| {
                    tab.parent_project()
                        .is_some_and(|project| self.project_tab(project).is_none())
                })
                .map(Tab::id),
        );
        ring
    }

    /// Focus the next tab of the ring, wrapping at the end. `None` when no tab is open.
    pub fn cycle_next(&mut self) -> Option<TabId> {
        let ring = self.cycle_sequence();
        if ring.is_empty() {
            self.focus(None);
            return None;
        }
        let next = match self
            .active
            .and_then(|id| ring.iter().position(|open| *open == id))
        {
            Some(position) => ring[(position + 1) % ring.len()],
            None => ring[0],
        };
        self.focus(Some(next));
        Some(next)
    }

    /// Focus the previous tab of the ring, wrapping at the start. `None` when no tab is open.
    pub fn cycle_previous(&mut self) -> Option<TabId> {
        let ring = self.cycle_sequence();
        if ring.is_empty() {
            self.focus(None);
            return None;
        }
        let previous = match self
            .active
            .and_then(|id| ring.iter().position(|open| *open == id))
        {
            Some(position) => ring[(position + ring.len() - 1) % ring.len()],
            None => ring[0],
        };
        self.focus(Some(previous));
        Some(previous)
    }

    /// The strip rows, the notes bar, and the repos bar for the current state (decisions 2, 5, 19).
    pub fn strip_rows(&self) -> StripRows {
        let active_project = self.active_project().map(str::to_string);
        StripRows {
            projects: self
                .tabs
                .iter()
                .filter(|tab| tab.key().kind() == TabRouteKind::Project)
                .map(Tab::id)
                .collect(),
            notes: self
                .tabs
                .iter()
                .filter(|tab| is_free_note(tab))
                .map(Tab::id)
                .collect(),
            repos: self
                .tabs
                .iter()
                .filter(|tab| is_free_repo(tab))
                .map(Tab::id)
                .collect(),
            parentless: self
                .tabs
                .iter()
                .filter(|tab| {
                    tab.parent_project().is_none() && !is_free_note(tab) && !is_free_repo(tab)
                })
                .map(Tab::id)
                .collect(),
            children: match active_project.as_deref() {
                Some(project) => self
                    .tabs
                    .iter()
                    .filter(|tab| is_child_of(tab, project))
                    .map(Tab::id)
                    .collect(),
                None => Vec::new(),
            },
            active_project,
        }
    }

    /// The identity of every open tab, in strip order.
    ///
    /// This is the lookup `focus_or_open` uses to tell "already open" from "open new".
    pub fn descriptors(&self) -> Vec<(TabId, TabKey)> {
        self.tabs
            .iter()
            .map(|tab| (tab.id, tab.key.clone()))
            .collect()
    }

    /// The open project tab of this project, when one exists.
    fn project_tab(&self, project_id: &str) -> Option<TabId> {
        self.tabs
            .iter()
            .find(
                |tab| matches!(tab.key(), TabKey::Project { project_id: open } if open == project_id),
            )
            .map(Tab::id)
    }

    /// The first tab after the active one in the ring that survives the close, wrapping around.
    /// `None` when every tab closes: the last close.
    fn survivor_after(&self, closing: &[TabId]) -> Option<TabId> {
        let ring = self.cycle_sequence();
        let start = ring.iter().position(|id| Some(*id) == self.active)?;
        ring.iter()
            .cycle()
            .skip(start + 1)
            .take(ring.len())
            .find(|id| !closing.contains(id))
            .copied()
    }
}

/// Whether `tab` is attached to `project_id`, which is what row 2 holds.
///
/// A project tab is not its own child, so the kind check keeps it out of its own child row.
fn is_child_of(tab: &Tab, project_id: &str) -> bool {
    tab.key().kind() != TabRouteKind::Project && tab.parent_project() == Some(project_id)
}

/// Whether `tab` belongs in the notes bar: a note that names no project.
///
/// A note attached to a project stays in that project's child row, so only the free note moves.
fn is_free_note(tab: &Tab) -> bool {
    tab.key().kind() == TabRouteKind::Note && tab.parent_project().is_none()
}

/// Whether `tab` belongs in the repos bar: a repo that names no project.
///
/// A repo attached to a project stays in that project's child row, so only the free repo moves.
fn is_free_repo(tab: &Tab) -> bool {
    tab.key().kind() == TabRouteKind::Repo && tab.parent_project().is_none()
}
