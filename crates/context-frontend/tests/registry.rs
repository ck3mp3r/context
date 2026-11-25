//! Pins the tab model: identity, attachment, close, cycle, and strip rows.
//!
//! The wasm crate cannot host-link its own test harness, so the model modules are included by path
//! and compiled into this binary: no wasm, no browser, and no rendering. The registry is exercised
//! through the same public API the shell uses, so the signal path is pinned headless too.

// The model modules are included whole, so the parts this binary does not pin are dead here.
#![allow(dead_code)]

#[path = "../src/tabs/open.rs"]
mod open;
#[path = "../src/tabs/pure.rs"]
mod pure;
#[path = "../src/tabs/registry.rs"]
mod registry;

use open::{focus_or_open, open_card, open_surface, surface_url, write_active_url};
use pure::{CloseOutcome, StripRows, Tab, TabId, TabKey, TabRouteKind, Tabs};
use registry::TabRegistry;

use std::cell::{Cell, RefCell};

use leptos_router::NavigateOptions;

/// Two projects with attached and parentless tabs: the smallest state that fills both rows and the
/// whole ring.
///
/// Strip order is `p1, p2, n-free, n-p1, r-free, r-p2, t1, t2, t3`, so the ring is
/// `[p1, p2, n-free, r-free, n-p1, t1, t2, r-p2, t3]` and the active tab is the last opened one.
struct Fixture {
    tabs: Tabs,
    p1: TabId,
    p2: TabId,
    free_note: TabId,
    free_repo: TabId,
    p1_note: TabId,
    p1_task: TabId,
    p1_task_second: TabId,
    p2_repo: TabId,
    p2_task: TabId,
}

fn fixture() -> Fixture {
    let mut tabs = Tabs::default();
    let p1 = tabs.open(TabKey::project("p1"));
    let p2 = tabs.open(TabKey::project("p2"));
    let free_note = tabs.open(TabKey::note("n-free"));
    let p1_note = tabs.open_in(TabKey::note("n-p1"), "p1");
    let free_repo = tabs.open(TabKey::repo("r-free"));
    let p2_repo = tabs.open_in(TabKey::repo("r-p2"), "p2");
    let p1_task = tabs.open_in(TabKey::task_list("t1"), "p1");
    let p1_task_second = tabs.open_in(TabKey::task_list("t2"), "p1");
    let p2_task = tabs.open_in(TabKey::task_list("t3"), "p2");
    Fixture {
        tabs,
        p1,
        p2,
        free_note,
        free_repo,
        p1_note,
        p1_task,
        p1_task_second,
        p2_repo,
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
fn a_tab_id_renders_as_its_number_for_the_dom() {
    let mut tabs = Tabs::default();
    let first = tabs.open(TabKey::project("p1"));
    let second = tabs.open(TabKey::note("n1"));

    assert_eq!(
        first.to_string(),
        "1",
        "the container's data-tab-id is the tab's id"
    );
    assert_eq!(second.to_string(), "2", "tab ids only ever grow");
    assert_eq!(TabId::default().to_string(), "0", "the detached id is 0");
}

#[test]
fn a_key_names_its_surface_kind_and_entity_id() {
    assert_eq!(TabKey::project("p1").kind(), TabRouteKind::Project);
    assert_eq!(TabKey::task_list("t1").kind(), TabRouteKind::TaskList);
    assert_eq!(TabKey::note("n1").kind(), TabRouteKind::Note);
    assert_eq!(TabKey::repo("r1").kind(), TabRouteKind::Repo);

    assert_eq!(TabKey::project("p1").entity_id(), "p1");
    assert_eq!(TabKey::task_list("t1").entity_id(), "t1");
    assert_eq!(TabKey::note("n1").entity_id(), "n1");
    assert_eq!(TabKey::repo("r1").entity_id(), "r1");
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
    let task = tabs.open_in(TabKey::task_list("x"), "x");
    assert_ne!(project, task, "the attachment is not part of the identity");
    assert_eq!(tabs.len(), 2);
}

#[test]
fn tab_parent_project_names_the_attached_project() {
    let state = fixture();
    let parent = |id: TabId| state.tabs.tab(id).and_then(Tab::parent_project);

    assert_eq!(
        parent(state.p1),
        Some("p1"),
        "a project tab's parent project is itself"
    );
    assert_eq!(parent(state.p1_note), Some("p1"));
    assert_eq!(parent(state.p2_repo), Some("p2"));
    assert_eq!(parent(state.p1_task), Some("p1"));
    assert_eq!(
        parent(state.free_note),
        None,
        "a note that names no project is parentless"
    );
    assert_eq!(parent(state.free_repo), None);
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
    let task = tabs.open_in(TabKey::task_list("t1"), "p1");
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
fn closing_a_project_removes_its_attached_tabs() {
    let mut state = fixture();
    let outcome = state.tabs.close(state.p1);

    let CloseOutcome::Closed { ids, .. } = outcome else {
        panic!("p1 is open");
    };
    assert_eq!(
        ids,
        vec![state.p1, state.p1_note, state.p1_task, state.p1_task_second],
        "the cascade follows attachment, so the project's note and task lists go too"
    );
    let remaining: Vec<TabId> = state.tabs.tabs().iter().map(|tab| tab.id()).collect();
    assert_eq!(
        remaining,
        vec![
            state.p2,
            state.free_note,
            state.free_repo,
            state.p2_repo,
            state.p2_task
        ],
        "a parentless note and repo survive the close"
    );
}

#[test]
fn closing_a_child_leaves_its_siblings_and_its_project() {
    let mut state = fixture();
    state.tabs.close(state.p1_task);

    assert!(state.tabs.find(&TabKey::project("p1")).is_some());
    assert!(state.tabs.find(&TabKey::task_list("t1")).is_none());
    assert!(state.tabs.find(&TabKey::task_list("t2")).is_some());
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
    state.tabs.close(state.p1_task);
    let before = state.tabs.clone();

    assert_eq!(state.tabs.close(state.p1_task), CloseOutcome::NotOpen);
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
            state.free_note,
            state.free_repo,
            state.p1_note,
            state.p1_task,
            state.p1_task_second,
            state.p2_repo,
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
    assert_eq!(rows.parentless, vec![state.free_note, state.free_repo]);
}

#[test]
fn row_two_holds_the_attached_tabs_of_the_active_project() {
    let mut state = fixture();
    state.tabs.activate(state.p1);
    let rows = state.tabs.strip_rows();

    assert_eq!(rows.active_project.as_deref(), Some("p1"));
    assert_eq!(
        rows.children,
        vec![state.p1_note, state.p1_task, state.p1_task_second],
        "a project's note and task lists share its child row"
    );
}

#[test]
fn a_child_of_an_inactive_project_brings_its_own_row_when_it_activates() {
    let mut state = fixture();
    state.tabs.activate(state.p2_repo);
    let rows = state.tabs.strip_rows();

    assert_eq!(rows.active_project.as_deref(), Some("p2"));
    assert_eq!(rows.children, vec![state.p2_repo, state.p2_task]);
    assert!(rows.contains(state.p2_repo));
}

#[test]
fn a_parentless_tab_leaves_row_two_empty() {
    let mut state = fixture();
    state.tabs.activate(state.free_repo);
    let rows = state.tabs.strip_rows();

    assert_eq!(rows.active_project, None);
    assert!(rows.children.is_empty());
    assert!(rows.contains(state.free_repo));
}

#[test]
fn an_attached_note_renders_in_its_project_row() {
    let mut tabs = Tabs::default();
    let project = tabs.open(TabKey::project("p1"));
    let note = tabs.open_in(TabKey::note("n1"), "p1");

    tabs.activate(project);
    assert_eq!(tabs.strip_rows().children, vec![note]);

    tabs.activate(note);
    let rows = tabs.strip_rows();
    assert_eq!(rows.active_project.as_deref(), Some("p1"));
    assert_eq!(rows.children, vec![note]);
    assert!(rows.contains(note));
}

#[test]
fn a_project_attached_note_cascades_with_its_project() {
    let mut tabs = Tabs::default();
    let project = tabs.open(TabKey::project("p1"));
    let note = tabs.open_in(TabKey::note("n1"), "p1");

    let outcome = tabs.close(project);

    assert_eq!(
        outcome,
        CloseOutcome::Closed {
            ids: vec![project, note],
            active: None
        }
    );
    assert!(tabs.is_empty());
}

#[test]
fn a_projectless_note_survives_a_project_close() {
    let mut tabs = Tabs::default();
    let project = tabs.open(TabKey::project("p1"));
    let note = tabs.open(TabKey::note("n1"));

    tabs.close(project);

    assert_eq!(tabs.len(), 1);
    assert!(tabs.contains(note));
    assert_eq!(tabs.strip_rows().parentless, vec![note]);
}

#[test]
fn a_note_attached_to_two_projects_stays_one_tab() {
    let mut tabs = Tabs::default();
    let p1 = tabs.open(TabKey::project("p1"));
    let p2 = tabs.open(TabKey::project("p2"));
    let first = tabs.open_in(TabKey::note("n1"), "p1");
    let again = tabs.open_in(TabKey::note("n1"), "p2");

    assert_eq!(first, again, "identity is kind plus entity id");
    assert_eq!(tabs.len(), 3);
    assert_eq!(
        tabs.tab(first).and_then(Tab::parent_project),
        Some("p1"),
        "the first attachment stays, so the note closes with the project it came from"
    );

    tabs.close(p2);
    assert!(tabs.contains(first), "the other project's close spares it");

    tabs.close(p1);
    assert!(!tabs.contains(first), "its own project's close takes it");
}

#[test]
fn no_row_mixes_a_project_with_a_child() {
    let mut state = fixture();
    state.tabs.activate(state.p1);
    let rows = state.tabs.strip_rows();

    let mut level_one = rows.projects.clone();
    level_one.extend(rows.parentless.iter().copied());
    for id in &level_one {
        let tab = state.tabs.tab(*id).expect("row 1 holds open tabs");
        assert!(
            tab.parent_project().is_none() || tab.key().kind() == TabRouteKind::Project,
            "a tab attached to a project must never sit in row 1"
        );
    }
    for id in &rows.children {
        let tab = state.tabs.tab(*id).expect("row 2 holds open tabs");
        assert_ne!(
            tab.key().kind(),
            TabRouteKind::Project,
            "a project tab is never its own child"
        );
        assert_eq!(tab.parent_project(), Some("p1"));
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
fn the_ring_keeps_projects_and_orphans_apart() {
    let mut tabs = Tabs::default();
    let project = tabs.open(TabKey::project("p1"));
    let child = tabs.open_in(TabKey::note("n1"), "p1");
    let orphan_task = tabs.open_in(TabKey::task_list("t9"), "p9");
    let orphan_note = tabs.open_in(TabKey::note("n9"), "p9");

    let ring = tabs.cycle_sequence();
    assert_eq!(
        ring,
        vec![project, child, orphan_task, orphan_note],
        "projects, then the parentless row, then each project's children, then tabs whose project tab is not open"
    );

    let mut unique = ring.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        tabs.len(),
        "every open tab appears exactly once"
    );

    for id in ring {
        tabs.activate(id);
        let active = tabs.active().expect("the tab was just activated");
        assert!(
            tabs.strip_rows().contains(active),
            "an orphan at {id:?} rendered in no row"
        );
    }
}

#[test]
fn a_child_without_its_project_tab_still_renders_when_active() {
    let mut tabs = Tabs::default();
    let orphan = tabs.open_in(TabKey::task_list("t9"), "p9");
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
fn the_registry_descriptors_list_every_open_identity() {
    let registry = TabRegistry::new();
    registry.open(TabKey::project("p1"));
    registry.open_in(TabKey::note("n1"), "p1");

    let descriptors = registry.descriptors();

    assert_eq!(descriptors.len(), 2);
    assert!(
        descriptors
            .iter()
            .any(|(_, key)| key == &TabKey::note("n1")),
        "the \"already open\" lookup must find an attached tab"
    );
}

#[test]
fn the_registry_close_cascades_and_moves_active_in_one_update() {
    let registry = TabRegistry::new();
    let project = registry.open(TabKey::project("p1"));
    let child = registry.open_in(TabKey::task_list("t1"), "p1");
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
fn the_registry_attaches_a_note_to_its_project() {
    let registry = TabRegistry::new();
    let project = registry.open(TabKey::project("p1"));
    let note = registry.open_in(TabKey::note("n1"), "p1");

    let rows = registry.strip_rows();
    assert_eq!(rows.active_project.as_deref(), Some("p1"));
    assert_eq!(rows.children, vec![note]);

    assert_eq!(
        registry.close(project),
        CloseOutcome::Closed {
            ids: vec![project, note],
            active: None
        }
    );
    assert_eq!(registry.active(), None);
    assert!(registry.is_empty());
}

#[test]
fn deactivating_the_strip_keeps_every_tab_open() {
    let mut state = fixture();
    let before = state.tabs.clone();

    assert!(
        state.tabs.deactivate(),
        "a strip with an active tab changes"
    );

    assert_eq!(
        state.tabs.active(),
        None,
        "chrome navigation leaves no active tab"
    );
    assert_eq!(
        state.tabs.len(),
        before.len(),
        "chrome navigation closes no tab"
    );
    assert_eq!(
        state.tabs.tabs(),
        before.tabs(),
        "the strip order and every attachment survive"
    );
    assert!(
        state.tabs.strip_rows().projects == before.strip_rows().projects,
        "row one keeps the opened project tabs"
    );
    assert!(
        state.tabs.strip_rows().children.is_empty(),
        "row two follows the active project, so no active tab leaves it empty"
    );
}

#[test]
fn deactivating_a_deactivated_strip_changes_nothing() {
    let mut state = fixture();
    state.tabs.deactivate();
    let before = state.tabs.clone();

    assert!(
        !state.tabs.deactivate(),
        "a strip that is already deactivated reports no change"
    );
    assert_eq!(state.tabs, before, "a repeated deactivation is a no-op");
}

#[test]
fn activating_a_tab_after_a_deactivation_takes_focus_back() {
    let mut state = fixture();
    state.tabs.deactivate();

    assert!(state.tabs.activate(state.p1));
    assert_eq!(state.tabs.active(), Some(state.p1));
    assert_eq!(
        state.tabs.strip_rows().active_project.as_deref(),
        Some("p1"),
        "the returned tab brings its own child row back"
    );
}

#[test]
fn the_registry_deactivate_keeps_every_tab_open() {
    let registry = TabRegistry::new();
    let project = registry.open(TabKey::project("p1"));
    registry.open_in(TabKey::task_list("t1"), "p1");

    assert!(registry.deactivate());

    assert_eq!(registry.active(), None, "chrome navigation clears focus");
    assert_eq!(registry.len(), 2, "chrome navigation closes no tab");
    assert!(
        registry.mounted(project),
        "a deactivated tab stays mounted, so returning to it costs no fetch"
    );
    assert!(
        !registry.deactivate(),
        "a second chrome navigation changes nothing"
    );
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
    let note = registry.open_in(TabKey::note("n1"), "p1");
    assert!(registry.activate(note));

    let rows = registry.strip_rows();

    assert_eq!(rows.active_project.as_deref(), Some("p1"));
    assert_eq!(rows.children, vec![note]);
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

#[test]
fn focus_or_open_focuses_an_open_surface_without_duplicating_it() {
    let registry = TabRegistry::new();
    let project = registry.open(TabKey::project("p1"));
    let other = registry.open(TabKey::project("p2"));
    assert_eq!(registry.active(), Some(other));

    let focused = focus_or_open(registry, TabKey::project("p1"));

    assert_eq!(focused, project, "the open surface keeps its tab id");
    assert_eq!(registry.len(), 2, "no duplicate tab appears");
    assert_eq!(registry.active(), Some(project), "the open tab takes focus");
}

#[test]
fn focus_or_open_opens_a_surface_that_is_not_open() {
    let registry = TabRegistry::new();
    let opened = focus_or_open(registry, TabKey::note("n1"));

    assert_eq!(registry.len(), 1);
    assert_eq!(registry.active(), Some(opened));
    assert_eq!(registry.active_key(), Some(TabKey::note("n1")));
}

#[test]
fn open_surface_opens_the_parent_project_tab_before_the_child() {
    let registry = TabRegistry::new();
    let child = open_surface(registry, TabKey::task_list("t1"), Some("p1"));

    let descriptors = registry.descriptors();
    assert_eq!(descriptors.len(), 2, "the parent tab opens with the child");
    let parent = descriptors
        .iter()
        .find(|(_, key)| key == &TabKey::project("p1"))
        .map(|(id, _)| *id)
        .expect("the parent project tab must be open");
    assert!(
        parent < child,
        "the parent tab opens first, so its id is lower"
    );
    assert_eq!(registry.active(), Some(child), "the child tab is active");
    assert_eq!(
        registry
            .tabs()
            .into_iter()
            .find(|tab| tab.id() == child)
            .and_then(|tab| tab.parent_project().map(str::to_string)),
        Some("p1".to_string()),
        "the child attaches to the named project"
    );
}

#[test]
fn open_surface_opens_a_parentless_surface_without_a_project_tab() {
    let registry = TabRegistry::new();
    let note = open_surface(registry, TabKey::note("n1"), None);

    assert_eq!(
        registry.len(),
        1,
        "no project tab is opened for a parentless note"
    );
    assert_eq!(registry.active(), Some(note));
    assert!(registry.strip_rows().parentless.contains(&note));
}

#[test]
fn open_surface_ignores_a_project_for_a_project_surface() {
    let registry = TabRegistry::new();
    let project = open_surface(registry, TabKey::project("p1"), Some("p2"));

    assert_eq!(
        registry.len(),
        1,
        "a project surface never opens a second project tab"
    );
    assert_eq!(registry.active(), Some(project));
}

#[test]
fn open_surface_focuses_an_open_child_instead_of_duplicating_it() {
    let registry = TabRegistry::new();
    let first = open_surface(registry, TabKey::task_list("t1"), Some("p1"));
    registry.open(TabKey::project("p2"));

    let again = open_surface(registry, TabKey::task_list("t1"), Some("p1"));

    assert_eq!(again, first, "the open child keeps its tab id");
    assert_eq!(registry.len(), 3, "the second open adds no tab");
    assert_eq!(
        registry.active(),
        Some(first),
        "the open child takes focus back"
    );
}

#[test]
fn open_card_reports_the_parent_label_before_the_clicked_one() {
    let registry = TabRegistry::new();

    let pending = open_card(registry, TabKey::task_list("t1"), Some("p1"), &|_| false);

    assert_eq!(
        pending,
        vec![TabKey::project("p1"), TabKey::task_list("t1")],
        "the parent project tab the click opens is resolved before the surface it holds"
    );
    assert_eq!(
        registry.len(),
        2,
        "the label gate changes nothing about the parent-first open"
    );
    assert_eq!(
        registry.active_key(),
        Some(TabKey::task_list("t1")),
        "the clicked surface still takes focus"
    );
}

#[test]
fn open_card_stays_quiet_about_a_parent_that_is_already_open() {
    let registry = TabRegistry::new();
    registry.open(TabKey::project("p1"));

    let pending = open_card(registry, TabKey::note("n1"), Some("p1"), &|_| false);

    assert_eq!(
        pending,
        vec![TabKey::note("n1")],
        "a parent that only focused was resolved when it opened"
    );
    assert_eq!(registry.len(), 2, "the click opens no second parent tab");
}

#[test]
fn open_card_stays_quiet_about_a_parent_whose_label_is_known() {
    let registry = TabRegistry::new();
    let labelled = |key: &TabKey| key == &TabKey::project("p1");

    let pending = open_card(registry, TabKey::task_list("t1"), Some("p1"), &labelled);

    assert_eq!(
        pending,
        vec![TabKey::task_list("t1")],
        "a parent the session already labels costs no request"
    );
}

#[test]
fn open_card_asks_for_no_label_a_second_time() {
    let registry = TabRegistry::new();
    let resolved = Cell::new(false);
    let labelled = |_: &TabKey| resolved.get();

    let first = open_card(registry, TabKey::note("n1"), None, &labelled);
    assert_eq!(
        first,
        vec![TabKey::note("n1")],
        "the first open of an unlabelled surface resolves its label"
    );

    resolved.set(true);
    let again = open_card(registry, TabKey::note("n1"), None, &labelled);

    assert!(
        again.is_empty(),
        "re-opening a surface whose label is known issues no GET"
    );
    assert_eq!(
        registry.len(),
        1,
        "re-opening the same surface opens no second tab"
    );
}

#[test]
fn surface_url_names_the_surface_and_its_project_only() {
    assert_eq!(
        surface_url(&TabKey::project("p1"), Some("p1")),
        "/projects/p1"
    );
    assert_eq!(
        surface_url(&TabKey::task_list("t1"), Some("p1")),
        "/projects/p1/task-lists/t1"
    );
    assert_eq!(
        surface_url(&TabKey::note("n1"), Some("p1")),
        "/projects/p1/notes/n1"
    );
    assert_eq!(
        surface_url(&TabKey::repo("r1"), Some("p1")),
        "/projects/p1/repos/r1"
    );
    assert_eq!(surface_url(&TabKey::note("n1"), None), "/notes/n1");
    assert_eq!(surface_url(&TabKey::repo("r1"), None), "/repos/r1");
    assert_eq!(
        surface_url(&TabKey::task_list("t1"), None),
        "/task-lists/t1"
    );
}

#[test]
fn write_active_url_writes_once_with_replace() {
    let seen: RefCell<Vec<(String, bool)>> = RefCell::new(Vec::new());

    {
        let writer = |url: &str, options: NavigateOptions| {
            seen.borrow_mut().push((url.to_string(), options.replace));
        };
        write_active_url(&writer, "/projects/p1");
        write_active_url(&writer, "/notes/n1");
    }

    assert_eq!(
        seen.into_inner(),
        vec![
            ("/projects/p1".to_string(), true),
            ("/notes/n1".to_string(), true),
        ],
        "every write replaces its entry, so a tab switch grows no history"
    );
}

#[test]
fn closing_the_active_restored_tab_mounts_the_survivor_it_activates() {
    let mut tabs = Tabs::default();
    let first = tabs.restore(TabKey::project("p1"), None);
    let second = tabs.restore(TabKey::project("p2"), None);
    assert!(
        !tabs.is_mounted(first),
        "a restored tab mounts on its first activation"
    );

    tabs.activate(second);
    assert_eq!(
        tabs.close(second),
        CloseOutcome::Closed {
            ids: vec![second],
            active: Some(first)
        }
    );

    assert!(
        tabs.is_mounted(first),
        "the survivor a close activates must mount, or the content area blanks"
    );
    assert!(
        tabs.strip_rows().contains(first),
        "the survivor renders in a row, so the strip never names a hidden tab"
    );
}

#[test]
fn every_close_leaves_the_active_tab_mounted_and_rendered() {
    let mut tabs = Tabs::default();
    tabs.restore(TabKey::project("p1"), None);
    tabs.restore(TabKey::project("p2"), None);
    tabs.restore(TabKey::note("n-free"), None);
    tabs.restore(TabKey::task_list("t1"), Some("p1".to_string()));
    let second = tabs.restore(TabKey::project("p2-again"), None);
    tabs.activate(second);

    let mut closed = Vec::new();
    while let Some(id) = tabs.active() {
        tabs.close(id);
        closed.push(id);

        let rows = tabs.strip_rows();
        match tabs.active() {
            Some(active) => {
                assert_ne!(active, id, "a close never keeps the tab it removed");
                assert!(
                    tabs.is_mounted(active),
                    "the close activated an unmounted tab: {active:?}"
                );
                assert!(
                    rows.contains(active),
                    "the close activated a tab no row renders: {active:?}"
                );
            }
            None => assert!(tabs.is_empty(), "no active tab means chrome home"),
        }
    }

    assert_eq!(closed.len(), 5, "every restored tab closed exactly once");
    assert!(tabs.is_empty());
}
