//! Pins the tab model: identity, close, cycle, and strip rows.
//!
//! The wasm crate cannot host-link its own test harness, so the module tree is included by path and
//! compiled into this binary: no wasm, no browser. The registry is exercised through the same public
//! API the shell uses, so the signal path is pinned headless too.

#[path = "../src/tabs/mod.rs"]
#[allow(dead_code)]
mod tabs;

use tabs::{
    CloseOutcome, StripRows, Tab, TabId, TabKey, TabRegistry, TabRouteKind, Tabs, parent_project,
};

/// Two projects with children, one note, and one repo: the smallest state that fills both rows and
/// the whole ring.
///
/// Strip order is `p1, p2, n1, r1, t1, t2, t3`, so the ring is `[p1, p2, n1, r1, t1, t2, t3]` and
/// the active tab is the last opened one.
struct Fixture {
    tabs: Tabs,
    p1: TabId,
    p2: TabId,
    note: TabId,
    repo: TabId,
    p1_first: TabId,
    p1_second: TabId,
    p2_task: TabId,
}

fn fixture() -> Fixture {
    let mut tabs = Tabs::default();
    let p1 = tabs.open(TabKey::project("p1"));
    let p2 = tabs.open(TabKey::project("p2"));
    let note = tabs.open(TabKey::note("n1"));
    let repo = tabs.open(TabKey::repo("r1"));
    let p1_first = tabs.open(TabKey::task_list("p1", "t1"));
    let p1_second = tabs.open(TabKey::task_list("p1", "t2"));
    let p2_task = tabs.open(TabKey::task_list("p2", "t3"));
    Fixture {
        tabs,
        p1,
        p2,
        note,
        repo,
        p1_first,
        p1_second,
        p2_task,
    }
}

#[test]
fn the_same_surface_opens_one_tab() {
    let mut tabs = Tabs::default();
    let first = tabs.open(TabKey::project("p1"));
    let again = tabs.open(TabKey::project("p1"));
    assert_eq!(first, again, "an open surface is focused, not duplicated");
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs.active(), Some(first));
}

#[test]
fn a_note_and_a_repo_with_one_id_are_two_tabs() {
    let mut tabs = Tabs::default();
    let note = tabs.open(TabKey::note("x"));
    let repo = tabs.open(TabKey::repo("x"));
    assert_ne!(note, repo, "the surface kind is part of the identity");
    assert_eq!(tabs.len(), 2);
}

#[test]
fn a_task_list_and_its_project_are_two_tabs() {
    let mut tabs = Tabs::default();
    let project = tabs.open(TabKey::project("x"));
    let task = tabs.open(TabKey::task_list("x", "x"));
    assert_ne!(project, task);
    assert_eq!(tabs.len(), 2);
}

#[test]
fn parent_project_names_the_project_a_tab_belongs_to() {
    assert_eq!(parent_project(&TabKey::project("p1")), Some("p1"));
    assert_eq!(parent_project(&TabKey::task_list("p1", "t1")), Some("p1"));
    assert_eq!(parent_project(&TabKey::note("n1")), None);
    assert_eq!(parent_project(&TabKey::repo("r1")), None);
}

#[test]
fn activating_an_open_tab_focuses_it() {
    let mut state = fixture();
    assert!(state.tabs.activate(state.p1));
    assert_eq!(state.tabs.active(), Some(state.p1));
}

#[test]
fn activating_a_closed_id_keeps_the_active_tab() {
    let mut tabs = Tabs::default();
    let project = tabs.open(TabKey::project("p1"));
    let task = tabs.open(TabKey::task_list("p1", "t1"));
    assert!(matches!(tabs.close(task), CloseOutcome::Closed { .. }));
    assert_eq!(tabs.active(), Some(project));

    assert!(!tabs.activate(task), "a closed id names no open tab");
    assert_eq!(
        tabs.active(),
        Some(project),
        "a stale active id must never leave every tab hidden"
    );
}

#[test]
fn activating_a_closed_id_on_an_empty_strip_keeps_chrome_home() {
    let mut tabs = Tabs::default();
    let only = tabs.open(TabKey::project("p1"));
    tabs.close(only);

    assert!(!tabs.activate(only));
    assert_eq!(tabs.active(), None);
    assert!(tabs.is_empty());
}

#[test]
fn closing_a_project_removes_its_children() {
    let mut state = fixture();
    let outcome = state.tabs.close(state.p1);

    let CloseOutcome::Closed { ids, .. } = outcome else {
        panic!("p1 is open");
    };
    assert_eq!(ids, vec![state.p1, state.p1_first, state.p1_second]);
    let remaining: Vec<TabId> = state.tabs.tabs().iter().map(|tab| tab.id()).collect();
    assert_eq!(
        remaining,
        vec![state.p2, state.note, state.repo, state.p2_task]
    );
}

#[test]
fn closing_a_child_leaves_its_siblings_and_its_project() {
    let mut state = fixture();
    state.tabs.close(state.p1_first);

    assert!(state.tabs.find(&TabKey::project("p1")).is_some());
    assert!(state.tabs.find(&TabKey::task_list("p1", "t1")).is_none());
    assert!(state.tabs.find(&TabKey::task_list("p1", "t2")).is_some());
}

#[test]
fn closing_the_active_tab_focuses_the_next_survivor_in_the_ring() {
    let mut state = fixture();
    state.tabs.activate(state.p1);

    state.tabs.close(state.p1);
    assert_eq!(
        state.tabs.active(),
        Some(state.p2),
        "the next ring survivor takes focus"
    );
}

#[test]
fn closing_the_active_tab_wraps_to_the_first_survivor() {
    let mut state = fixture();
    state.tabs.activate(state.p2_task);

    state.tabs.close(state.p2_task);
    assert_eq!(state.tabs.active(), Some(state.p1));
}

#[test]
fn closing_an_inactive_tab_keeps_the_active_tab() {
    let mut state = fixture();
    state.tabs.activate(state.p2_task);

    state.tabs.close(state.p1);
    assert_eq!(state.tabs.active(), Some(state.p2_task));
}

#[test]
fn the_last_close_leaves_no_active_tab() {
    let mut tabs = Tabs::default();
    let only = tabs.open(TabKey::note("n1"));

    let outcome = tabs.close(only);
    assert_eq!(
        outcome,
        CloseOutcome::Closed {
            ids: vec![only],
            active: None
        }
    );
    assert_eq!(tabs.active(), None, "the last close returns to chrome home");
    assert!(tabs.is_empty());
}

#[test]
fn closing_every_tab_one_by_one_ends_on_chrome_home() {
    let mut state = fixture();
    while let Some(id) = state.tabs.cycle_sequence().first().copied() {
        state.tabs.close(id);
    }

    assert!(state.tabs.is_empty());
    assert_eq!(state.tabs.active(), None);
}

#[test]
fn closing_a_closed_id_changes_nothing() {
    let mut state = fixture();
    state.tabs.close(state.p1_first);
    let before = state.tabs.clone();

    assert_eq!(state.tabs.close(state.p1_first), CloseOutcome::NotOpen);
    assert_eq!(state.tabs, before);
}

#[test]
fn the_ring_is_projects_then_parentless_then_children() {
    let state = fixture();
    assert_eq!(
        state.tabs.cycle_sequence(),
        vec![
            state.p1,
            state.p2,
            state.note,
            state.repo,
            state.p1_first,
            state.p1_second,
            state.p2_task,
        ]
    );
}

#[test]
fn the_ring_does_not_depend_on_the_active_tab() {
    let mut state = fixture();
    let ring = state.tabs.cycle_sequence();

    for id in ring.clone() {
        state.tabs.activate(id);
        assert_eq!(
            state.tabs.cycle_sequence(),
            ring,
            "the ring must not move when the active tab changes"
        );
    }
}

#[test]
fn a_walk_from_every_start_visits_every_tab_once() {
    let mut state = fixture();
    let ring = state.tabs.cycle_sequence();
    let mut expected = ring.clone();
    expected.sort();

    for start in ring.clone() {
        state.tabs.activate(start);
        let mut visited = Vec::new();
        for _ in 0..ring.len() {
            visited.push(state.tabs.cycle_next().expect("the strip is not empty"));
        }
        visited.sort();

        assert_eq!(
            visited, expected,
            "a walk from {start:?} must visit every tab"
        );
        assert_eq!(
            state.tabs.active(),
            Some(start),
            "a full walk returns to its start"
        );
    }
}

#[test]
fn stepping_backward_is_the_inverse_of_stepping_forward() {
    let mut state = fixture();
    state.tabs.activate(state.p1);

    for _ in 0..state.tabs.len() {
        state.tabs.cycle_next();
    }
    assert_eq!(state.tabs.active(), Some(state.p1));

    state.tabs.cycle_previous();
    assert_eq!(state.tabs.active(), Some(state.p2_task));
}

#[test]
fn row_one_holds_the_projects_and_the_parentless_row() {
    let state = fixture();
    let rows: StripRows = state.tabs.strip_rows();

    assert_eq!(rows.projects, vec![state.p1, state.p2]);
    assert_eq!(rows.parentless, vec![state.note, state.repo]);
}

#[test]
fn row_two_holds_the_children_of_the_active_project() {
    let mut state = fixture();
    state.tabs.activate(state.p1);
    let rows = state.tabs.strip_rows();

    assert_eq!(rows.active_project.as_deref(), Some("p1"));
    assert_eq!(rows.children, vec![state.p1_first, state.p1_second]);
}

#[test]
fn a_child_of_an_inactive_project_brings_its_own_row_when_it_activates() {
    let mut state = fixture();
    state.tabs.activate(state.p2_task);
    let rows = state.tabs.strip_rows();

    assert_eq!(rows.active_project.as_deref(), Some("p2"));
    assert_eq!(rows.children, vec![state.p2_task]);
    assert!(rows.contains(state.p2_task));
}

#[test]
fn a_parentless_tab_leaves_row_two_empty() {
    let mut state = fixture();
    state.tabs.activate(state.repo);
    let rows = state.tabs.strip_rows();

    assert_eq!(rows.active_project, None);
    assert!(rows.children.is_empty());
    assert!(rows.contains(state.repo));
}

#[test]
fn no_row_mixes_a_project_with_a_child() {
    let state = fixture();
    let rows = state.tabs.strip_rows();

    let mut level_one = rows.projects.clone();
    level_one.extend(rows.parentless.iter().copied());
    for id in &level_one {
        let key = state.tabs.tab(*id).expect("row 1 holds open tabs").key();
        assert_ne!(
            key.kind(),
            TabRouteKind::TaskList,
            "a child tab must never sit in row 1"
        );
    }
    for id in &rows.children {
        let key = state.tabs.tab(*id).expect("row 2 holds open tabs").key();
        assert_eq!(key.kind(), TabRouteKind::TaskList);
        assert!(!level_one.contains(id), "a tab cannot sit in both rows");
    }
}

#[test]
fn every_ring_position_renders_its_active_tab() {
    let mut state = fixture();

    for position in state.tabs.cycle_sequence() {
        state.tabs.activate(position);
        let active = state.tabs.active().expect("the tab was just activated");
        assert!(
            state.tabs.strip_rows().contains(active),
            "the active tab {position:?} rendered in no row"
        );
    }
}

#[test]
fn a_child_without_its_project_tab_still_renders_when_active() {
    let mut tabs = Tabs::default();
    let orphan = tabs.open(TabKey::task_list("p9", "t9"));
    let rows = tabs.strip_rows();

    assert_eq!(
        rows.active_project.as_deref(),
        Some("p9"),
        "the active project is derived from the child"
    );
    assert_eq!(rows.children, vec![orphan]);
    assert!(rows.projects.is_empty());
    assert_eq!(tabs.cycle_sequence(), vec![orphan]);
}

#[test]
fn descriptors_list_every_open_identity() {
    let state = fixture();
    let descriptors = state.tabs.descriptors();

    assert_eq!(descriptors.len(), state.tabs.len());
    let found = descriptors
        .iter()
        .find(|(_, key)| key == &TabKey::project("p1"))
        .map(|(id, _)| *id);
    assert_eq!(found, Some(state.p1), "the open lookup must find the tab");

    let mut keys: Vec<&TabKey> = descriptors.iter().map(|(_, key)| key).collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), descriptors.len(), "identities are unique");
}

#[test]
fn the_registry_opens_one_tab_per_surface() {
    let registry = TabRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.active(), None);

    let first = registry.open(TabKey::project("p1"));
    let again = registry.open(TabKey::project("p1"));

    assert_eq!(first, again);
    assert_eq!(registry.len(), 1);
    assert_eq!(registry.active_key(), Some(TabKey::project("p1")));

    let open: Vec<Tab> = registry.tabs();
    assert_eq!(
        open.len(),
        1,
        "the registry reports the tab the signal holds"
    );
    assert_eq!(open[0].key(), &TabKey::project("p1"));
}

#[test]
fn the_registry_close_cascades_and_moves_active_in_one_update() {
    let registry = TabRegistry::new();
    let project = registry.open(TabKey::project("p1"));
    let child = registry.open(TabKey::task_list("p1", "t1"));
    let other = registry.open(TabKey::project("p2"));
    assert_eq!(registry.active(), Some(other));

    let outcome = registry.close(project);

    assert_eq!(
        outcome,
        CloseOutcome::Closed {
            ids: vec![project, child],
            active: Some(other)
        }
    );
    assert_eq!(registry.len(), 1);
    assert_eq!(registry.active(), Some(other));
}

#[test]
fn the_registry_last_close_clears_active() {
    let registry = TabRegistry::new();
    let only = registry.open(TabKey::note("n1"));

    let outcome = registry.close(only);

    assert_eq!(
        outcome,
        CloseOutcome::Closed {
            ids: vec![only],
            active: None
        }
    );
    assert_eq!(registry.active(), None);
    assert!(registry.is_empty());
    assert!(registry.strip_rows().projects.is_empty());
}

#[test]
fn the_registry_rows_follow_the_active_child() {
    let registry = TabRegistry::new();
    registry.open(TabKey::project("p1"));
    registry.open(TabKey::project("p2"));
    let child = registry.open(TabKey::task_list("p1", "t1"));
    assert!(registry.activate(child));

    let rows = registry.strip_rows();

    assert_eq!(rows.active_project.as_deref(), Some("p1"));
    assert_eq!(rows.children, vec![child]);
    assert_eq!(registry.active_project().as_deref(), Some("p1"));
}

#[test]
fn the_registry_cycle_walks_the_ring() {
    let registry = TabRegistry::new();
    let project = registry.open(TabKey::project("p1"));
    let note = registry.open(TabKey::note("n1"));
    assert_eq!(registry.active(), Some(note));

    assert_eq!(registry.cycle_next(), Some(project));
    assert_eq!(registry.cycle_previous(), Some(note));
}
