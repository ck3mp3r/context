//! The reactive tab registry: the pure model behind one `RwSignal`.

use leptos::prelude::{Get, RwSignal, Update};

use super::pure::{CloseOutcome, StripRows, Tab, TabId, TabKey, Tabs};

/// The tab registry the shell reads.
///
/// One signal holds the whole model, so every open, activate, and close is a single update and no
/// reader can observe a half-closed state. The registry is `Copy`: pass it down as a handle, not as
/// a borrow.
#[derive(Clone, Copy, Debug)]
pub struct TabRegistry {
    state: RwSignal<Tabs>,
}

impl TabRegistry {
    /// Start with no open tab: chrome home.
    pub fn new() -> Self {
        Self {
            state: RwSignal::new(Tabs::default()),
        }
    }

    /// Every open tab, in strip order.
    pub fn tabs(&self) -> Vec<Tab> {
        self.state
            .try_get()
            .map(|tabs| tabs.tabs().to_vec())
            .unwrap_or_default()
    }

    /// Number of open tabs.
    pub fn len(&self) -> usize {
        self.state.try_get().map_or(0, |tabs| tabs.len())
    }

    /// Whether no tab is open, which is chrome home.
    pub fn is_empty(&self) -> bool {
        self.state
            .try_get()
            .map(|tabs| tabs.is_empty())
            .unwrap_or(true)
    }

    /// The focused tab id, or `None` on chrome home.
    pub fn active(&self) -> Option<TabId> {
        self.state.try_get().and_then(|tabs| tabs.active())
    }

    /// The identity of the active tab.
    pub fn active_key(&self) -> Option<TabKey> {
        self.state
            .try_get()
            .and_then(|tabs| tabs.active_key().cloned())
    }

    /// The project the active tab belongs to (decision 19). Derived, never stored.
    pub fn active_project(&self) -> Option<String> {
        self.state
            .try_get()
            .and_then(|tabs| tabs.active_project().map(str::to_string))
    }

    /// Open a surface and focus it, or focus it when it is already open (decision 6).
    ///
    /// A disposed signal yields the detached id, which names no tab anywhere.
    pub fn open(&self, key: TabKey) -> TabId {
        let mut opened = TabId::default();
        self.state.update(|tabs| opened = tabs.open(key));
        opened
    }

    /// Open a surface inside `project_id` and focus it, or focus it when it is already open.
    ///
    /// The tab becomes a child of that project: row 2 lists it while the project is active and a
    /// close of the project removes it (ruling L, decision 9).
    pub fn open_in(&self, key: TabKey, project_id: &str) -> TabId {
        let mut opened = TabId::default();
        self.state
            .update(|tabs| opened = tabs.open_in(key, project_id));
        opened
    }

    /// Focus an open tab. An id that names no open tab changes nothing.
    pub fn activate(&self, id: TabId) -> bool {
        let mut activated = false;
        self.state.update(|tabs| activated = tabs.activate(id));
        activated
    }

    /// Close a tab, cascade a project's children, and move `active` in one update.
    pub fn close(&self, id: TabId) -> CloseOutcome {
        let mut outcome = CloseOutcome::NotOpen;
        self.state.update(|tabs| outcome = tabs.close(id));
        outcome
    }

    /// Focus the next tab of the cycle ring.
    pub fn cycle_next(&self) -> Option<TabId> {
        let mut next = None;
        self.state.update(|tabs| next = tabs.cycle_next());
        next
    }

    /// Focus the previous tab of the cycle ring.
    pub fn cycle_previous(&self) -> Option<TabId> {
        let mut previous = None;
        self.state.update(|tabs| previous = tabs.cycle_previous());
        previous
    }

    /// The two strip rows for the current state.
    pub fn strip_rows(&self) -> StripRows {
        self.state
            .try_get()
            .map(|tabs| tabs.strip_rows())
            .unwrap_or_default()
    }

    /// The identity of every open tab, for the "already open" lookup.
    pub fn descriptors(&self) -> Vec<(TabId, TabKey)> {
        self.state
            .try_get()
            .map(|tabs| tabs.descriptors())
            .unwrap_or_default()
    }
}

impl Default for TabRegistry {
    fn default() -> Self {
        Self::new()
    }
}
