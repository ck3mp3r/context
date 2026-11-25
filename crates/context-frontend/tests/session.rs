//! Pins the tab session: the stored schema, the boot plan, and the lazy restore.
//!
//! The wasm crate cannot host-link its own test harness, so the model modules are included by path
//! and compiled into this binary: no wasm, no browser, and no storage. Storage access and the boot
//! sequence live in `src/tabs/boot.rs` and are pinned in the browser harness (`tests/e2e.rs`).

// The model modules are included whole, so the parts this binary does not pin are dead here.
#![allow(dead_code)]

#[path = "../src/tabs/open.rs"]
mod open;
#[path = "../src/tabs/pure.rs"]
mod pure;
#[path = "../src/tabs/registry.rs"]
mod registry;
#[path = "../src/tabs/session.rs"]
mod session;

use std::cell::RefCell;

use leptos::prelude::{GetUntracked, RwSignal};
use leptos_router::NavigateOptions;

use pure::TabKey;
use registry::TabRegistry;
use session::{
    BootPlan, BootSurface, SESSION_VERSION, StoredKey, StoredKind, StoredSession, StoredTab,
    TabTitles, adopt_owned_url, boot_surface, decode, encode, install_restored, plan_boot,
    snapshot, storage_key, write_owned_url,
};

/// The environment this test binary runs in: a debug build, so the dev key.
const DEV: &str = "dev";

fn stored_key(kind: StoredKind, id: &str) -> StoredKey {
    StoredKey {
        kind,
        id: id.to_string(),
    }
}

fn stored_tab(kind: StoredKind, id: &str, parent: Option<&str>) -> StoredTab {
    StoredTab {
        key: stored_key(kind, id),
        title: format!("label {id}"),
        parent: parent.map(str::to_string),
    }
}

fn session(tabs: Vec<StoredTab>, active: Option<StoredKey>) -> StoredSession {
    StoredSession {
        version: SESSION_VERSION,
        tabs,
        active,
    }
}

/// A project tab, its task list child, and a note that names no project, in that strip order.
fn sample() -> StoredSession {
    session(
        vec![
            stored_tab(StoredKind::Project, "p1", None),
            stored_tab(StoredKind::TaskList, "t1", Some("p1")),
            stored_tab(StoredKind::Note, "n1", None),
        ],
        Some(stored_key(StoredKind::TaskList, "t1")),
    )
}

/// The identities a registry holds, in strip order.
fn strip_order(registry: TabRegistry) -> Vec<TabKey> {
    registry
        .tabs()
        .into_iter()
        .map(|tab| tab.key().clone())
        .collect()
}

// region:    --- Schema

#[test]
fn decode_keeps_the_stored_strip_order_active_surface_and_attachment() {
    let encoded = encode(&sample()).expect("a session encodes");
    let decoded = decode(&encoded).expect("a session this build wrote decodes");

    assert_eq!(decoded, sample(), "the session round-trips unchanged");
    assert_eq!(
        decoded
            .tabs
            .iter()
            .map(|tab| tab.key.id.as_str())
            .collect::<Vec<_>>(),
        vec!["p1", "t1", "n1"],
        "the stored strip order is the order a reload rebuilds"
    );
    assert_eq!(
        decoded.active,
        Some(stored_key(StoredKind::TaskList, "t1")),
        "the active surface is stored, not the active tab id"
    );
    assert_eq!(
        decoded.tabs[1].parent.as_deref(),
        Some("p1"),
        "a child tab stores the project that holds its row"
    );
}

#[test]
fn decode_rejects_garbage() {
    assert_eq!(decode("not a session"), None);
    assert_eq!(decode(""), None);
    assert_eq!(decode("{\"version\":"), None);
}

#[test]
fn decode_rejects_a_version_mismatch() {
    let stale = format!(
        "{{\"version\":{},\"tabs\":[{{\"key\":{{\"kind\":\"project\",\"id\":\"p1\"}},\"title\":\"P\",\"parent\":null}}],\"active\":null}}",
        SESSION_VERSION + 1
    );
    assert_eq!(decode(&stale), None, "a stale schema drops to chrome");
}

#[test]
fn decode_rejects_a_session_with_no_tab() {
    let empty = format!("{{\"version\":{SESSION_VERSION},\"tabs\":[],\"active\":null}}");
    assert_eq!(decode(&empty), None, "an empty session drops to chrome");
}

#[test]
fn decode_drops_a_repeated_identity_and_an_empty_id() {
    let raw = encode(&session(
        vec![
            stored_tab(StoredKind::Note, "n1", Some("p1")),
            stored_tab(StoredKind::Note, "n1", Some("p2")),
            stored_tab(StoredKind::Project, "  ", None),
        ],
        Some(stored_key(StoredKind::Note, "n1")),
    ))
    .expect("a session encodes");

    let decoded = decode(&raw).expect("one tab survives the sanitize");
    assert_eq!(decoded.tabs.len(), 1, "identity is unique (decision 6)");
    assert_eq!(
        decoded.tabs[0].parent.as_deref(),
        Some("p1"),
        "the first attachment is the one a note keeps"
    );
}

#[test]
fn decode_drops_an_active_surface_the_session_does_not_hold() {
    let raw = encode(&session(
        vec![stored_tab(StoredKind::Project, "p1", None)],
        Some(stored_key(StoredKind::Note, "n9")),
    ))
    .expect("a session encodes");

    assert_eq!(
        decode(&raw).expect("the tab survives").active,
        None,
        "an active surface that is not in the strip cannot be restored"
    );
}

#[test]
fn the_storage_key_is_namespaced_per_environment() {
    assert_eq!(storage_key(DEV), "c5t.tabs.session.dev.v1");
    assert_eq!(storage_key("release"), "c5t.tabs.session.release.v1");
    assert_ne!(
        storage_key(DEV),
        storage_key("release"),
        "a dev build and a release build never collide"
    );
}

// endregion: --- Schema

// region:    --- Boot plan

#[test]
fn plan_boot_is_fresh_without_a_session() {
    assert_eq!(plan_boot(None, None), BootPlan::Fresh);
    assert_eq!(
        plan_boot(None, boot_surface("/notes/n1").as_ref()),
        BootPlan::Fresh,
        "a deep link with no session is not a restore"
    );
}

#[test]
fn plan_boot_restores_and_focuses_the_boot_surface_when_the_session_holds_it() {
    let stored = sample();
    let boot = boot_surface("/projects/p1/task-lists/t1").expect("the url names a tab surface");

    assert_eq!(
        plan_boot(Some(&stored), Some(&boot)),
        BootPlan::Restore {
            focus: Some(stored_key(StoredKind::TaskList, "t1")),
        },
        "a reload of a stored surface focuses that surface"
    );
}

#[test]
fn plan_boot_restores_and_appends_a_boot_surface_the_session_does_not_hold() {
    let stored = sample();
    let boot = boot_surface("/projects/p1/repos/r1").expect("the url names a tab surface");

    assert_eq!(
        plan_boot(Some(&stored), Some(&boot)),
        BootPlan::RestorePlusBoot { boot },
        "a deep link to a new surface restores the session and opens its own tab"
    );
}

#[test]
fn plan_boot_restores_the_stored_active_surface_on_a_chrome_boot() {
    let stored = sample();
    assert_eq!(
        plan_boot(Some(&stored), None),
        BootPlan::Restore {
            focus: Some(stored_key(StoredKind::TaskList, "t1")),
        },
        "a chrome boot restores the workspace with its active surface"
    );

    let no_active = session(sample().tabs, None);
    assert_eq!(
        plan_boot(Some(&no_active), None),
        BootPlan::Restore { focus: None },
        "a session without an active surface restores the strip"
    );
}

#[test]
fn boot_surface_names_the_tab_routes_and_no_chrome_route() {
    assert_eq!(
        boot_surface("/projects/p1"),
        Some(BootSurface {
            key: stored_key(StoredKind::Project, "p1"),
            parent: None,
        })
    );
    assert_eq!(
        boot_surface("/projects/p1/task-lists/t1"),
        Some(BootSurface {
            key: stored_key(StoredKind::TaskList, "t1"),
            parent: Some("p1".to_string()),
        })
    );
    assert_eq!(
        boot_surface("/notes/n1"),
        Some(BootSurface {
            key: stored_key(StoredKind::Note, "n1"),
            parent: None,
        })
    );
    assert_eq!(
        boot_surface("/repos/r1"),
        Some(BootSurface {
            key: stored_key(StoredKind::Repo, "r1"),
            parent: None,
        })
    );

    for chrome in [
        "/",
        "/notes",
        "/repos",
        "/skills",
        "/skills/abc",
        "/settings",
        "/projects/p1/task-lists",
        "/unknown/n1",
    ] {
        assert_eq!(
            boot_surface(chrome),
            None,
            "{chrome} is a chrome route, so it names no tab surface"
        );
    }
}

// endregion: --- Boot plan

// region:    --- Restore

#[test]
fn install_restored_rebuilds_the_strip_in_stored_order() {
    let registry = TabRegistry::new();
    let installed = install_restored(registry, &sample(), None);

    assert_eq!(
        strip_order(registry),
        installed,
        "the strip is the stored order"
    );
    assert_eq!(
        installed,
        vec![
            TabKey::project("p1"),
            TabKey::task_list("t1"),
            TabKey::note("n1"),
        ]
    );
}

#[test]
fn install_restored_reproduces_the_attachment_without_a_fetch() {
    let registry = TabRegistry::new();
    let stored = session(
        vec![
            stored_tab(StoredKind::Project, "p1", None),
            stored_tab(StoredKind::Project, "p2", None),
            stored_tab(StoredKind::Note, "n1", Some("p2")),
            stored_tab(StoredKind::Repo, "r1", None),
        ],
        Some(stored_key(StoredKind::Note, "n1")),
    );

    install_restored(registry, &stored, Some(&stored_key(StoredKind::Note, "n1")));

    let parent = |key: &TabKey| {
        registry
            .tabs()
            .into_iter()
            .find(|tab| tab.key() == key)
            .and_then(|tab| tab.parent_project().map(str::to_string))
    };
    assert_eq!(
        parent(&TabKey::note("n1")),
        Some("p2".to_string()),
        "the stored parent link comes back, so a dual-project note keeps its row and its cascade"
    );
    assert_eq!(
        parent(&TabKey::repo("r1")),
        None,
        "a stored tab with no parent comes back parentless"
    );
}

#[test]
fn install_restored_focuses_the_stored_active_surface() {
    let registry = TabRegistry::new();
    install_restored(
        registry,
        &sample(),
        Some(&stored_key(StoredKind::TaskList, "t1")),
    );

    assert_eq!(
        registry.active_key(),
        Some(TabKey::task_list("t1")),
        "the stored active surface takes focus"
    );
}

#[test]
fn install_restored_falls_back_to_the_first_tab_when_the_focus_is_gone() {
    let registry = TabRegistry::new();
    install_restored(
        registry,
        &sample(),
        Some(&stored_key(StoredKind::Note, "gone")),
    );

    assert_eq!(
        registry.active_key(),
        Some(TabKey::project("p1")),
        "a strip with no active tab would render every tab hidden"
    );
}

#[test]
fn a_restored_strip_mounts_only_the_focused_tab() {
    let registry = TabRegistry::new();
    install_restored(
        registry,
        &sample(),
        Some(&stored_key(StoredKind::TaskList, "t1")),
    );

    let open = registry.tabs();
    let mounting: Vec<TabKey> = open
        .iter()
        .filter(|tab| tab.is_mounted())
        .map(|tab| tab.key().clone())
        .collect();

    assert_eq!(open.len(), 3, "every stored tab is back in the strip");
    assert_eq!(
        mounting,
        vec![TabKey::task_list("t1")],
        "only the active tab mounts at boot, so the hidden tabs fetch nothing (decision 14)"
    );
}

#[test]
fn an_opened_tab_mounts_at_once() {
    let registry = TabRegistry::new();
    let project = registry.open(TabKey::project("p1"));
    let task_list = registry.open_in(TabKey::task_list("t1"), "p1");

    assert!(
        registry.mounted(project),
        "a parent tab opened for its child mounts at once, or its surface would never appear"
    );
    assert!(registry.mounted(task_list));
}

#[test]
fn activating_a_restored_tab_mounts_it_for_good() {
    let registry = TabRegistry::new();
    let installed = install_restored(
        registry,
        &sample(),
        Some(&stored_key(StoredKind::TaskList, "t1")),
    );
    let hidden = registry
        .tabs()
        .into_iter()
        .find(|tab| tab.key() == &installed[0])
        .map(|tab| tab.id());
    let hidden = hidden.expect("the project tab is in the strip");
    assert!(!registry.mounted(hidden), "a restored tab starts unmounted");

    assert!(registry.activate(hidden));
    assert!(
        registry.mounted(hidden),
        "the first activation mounts the tab"
    );

    registry.activate(
        registry
            .tabs()
            .into_iter()
            .find(|tab| tab.key() == &TabKey::task_list("t1"))
            .map(|tab| tab.id())
            .expect("the task list tab is in the strip"),
    );
    assert!(
        registry.mounted(hidden),
        "a mounted tab stays mounted, so a switch hides and never unmounts (decision 13)"
    );
}

#[test]
fn closing_the_active_restored_tab_mounts_the_survivor() {
    let registry = TabRegistry::new();
    install_restored(
        registry,
        &sample(),
        Some(&stored_key(StoredKind::TaskList, "t1")),
    );
    let closed = registry
        .active()
        .expect("the stored active tab took focus on restore");

    registry.close(closed);

    let active = registry.active().expect("a survivor takes focus");
    assert!(
        registry.mounted(active),
        "the tab a close focuses must mount, or the content area would render nothing"
    );
}

#[test]
fn cycling_to_a_restored_tab_mounts_it() {
    let registry = TabRegistry::new();
    install_restored(
        registry,
        &sample(),
        Some(&stored_key(StoredKind::TaskList, "t1")),
    );

    let next = registry
        .cycle_next()
        .expect("the restored strip is not empty");

    assert!(
        registry.mounted(next),
        "a restored tab mounts the moment the ring reaches it (decision 14)"
    );
}

// endregion: --- Restore

// region:    --- Snapshot

#[test]
fn snapshot_stores_the_strip_order_the_active_surface_and_the_labels() {
    let registry = TabRegistry::new();
    let titles = TabTitles::new();
    registry.open(TabKey::project("p1"));
    registry.open_in(TabKey::note("n1"), "p1");
    titles.set(TabKey::project("p1"), "Project One".to_string());
    titles.set(TabKey::note("n1"), "Note One".to_string());

    let stored = snapshot(registry, titles);

    assert_eq!(stored.version, SESSION_VERSION);
    assert_eq!(
        stored
            .tabs
            .iter()
            .map(|tab| (tab.key.clone(), tab.title.as_str(), tab.parent.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (stored_key(StoredKind::Project, "p1"), "Project One", None),
            (stored_key(StoredKind::Note, "n1"), "Note One", Some("p1")),
        ],
        "every open tab stores its identity, its label, and its attachment in strip order"
    );
    assert_eq!(
        stored.active,
        Some(stored_key(StoredKind::Note, "n1")),
        "the active surface is the last opened one"
    );
}

#[test]
fn a_stored_session_rebuilds_the_strip_it_came_from() {
    let registry = TabRegistry::new();
    let titles = TabTitles::new();
    registry.open(TabKey::project("p1"));
    registry.open_in(TabKey::task_list("t1"), "p1");
    registry.open(TabKey::note("n1"));

    let encoded = encode(&snapshot(registry, titles)).expect("the session encodes");
    let stored = decode(&encoded).expect("the session decodes");

    let rebuilt = TabRegistry::new();
    install_restored(rebuilt, &stored, stored.active.as_ref());

    assert_eq!(
        strip_order(rebuilt),
        strip_order(registry),
        "a reload rebuilds the identities in the same strip order"
    );
    assert_eq!(
        rebuilt.active_key(),
        registry.active_key(),
        "and focuses the same active surface"
    );
}

// endregion: --- Snapshot

// region:    --- URL ownership

#[test]
fn write_owned_url_writes_only_when_the_url_differs() {
    let seen: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let writer = |url: &str, options: NavigateOptions| {
        assert!(
            options.replace,
            "every write replaces, so no history entry appears"
        );
        seen.borrow_mut().push(url.to_string());
    };

    let owned = RwSignal::new("/projects/p1".to_string());
    write_owned_url(&writer, &owned, "/projects/p1");
    assert!(
        seen.borrow().is_empty(),
        "boot restore writes nothing: the URL already names the surface"
    );

    write_owned_url(&writer, &owned, "/notes/n1");
    assert_eq!(
        owned.get_untracked(),
        "/notes/n1",
        "the app owns the URL it wrote"
    );

    write_owned_url(&writer, &owned, "/notes/n1");
    assert_eq!(
        seen.into_inner(),
        vec!["/notes/n1".to_string()],
        "one write per change, so a deep link survives a restore and no write repeats"
    );
}

#[test]
fn adopt_owned_url_takes_the_url_a_chrome_navigation_landed_on() {
    let seen: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let writer = |url: &str, _: NavigateOptions| seen.borrow_mut().push(url.to_string());

    let owned = RwSignal::new("/projects/p1".to_string());
    adopt_owned_url(&owned, "/notes");

    assert_eq!(
        owned.get_untracked(),
        "/notes",
        "chrome navigation adopts the URL it landed on, and writes nothing"
    );
    assert!(
        seen.borrow().is_empty(),
        "an adoption is not a navigation: the deactivation writes no URL"
    );

    write_owned_url(&writer, &owned, "/projects/p1");
    assert_eq!(
        seen.into_inner(),
        vec!["/projects/p1".to_string()],
        "returning to a tab writes its URL again, because the app no longer owned it"
    );
}

// endregion: --- URL ownership

// region:    --- Titles

#[test]
fn has_label_is_true_only_for_a_known_non_blank_label() {
    let titles = TabTitles::new();
    let note = TabKey::note("n1");

    assert!(
        !titles.has_label(&note),
        "a session that resolved nothing knows no label"
    );

    titles.set(note.clone(), String::new());
    assert!(
        !titles.has_label(&note),
        "a blank stored label names no surface, so the open path fetches"
    );

    titles.set(note.clone(), "Note One".to_string());
    assert!(titles.has_label(&note));
    assert!(
        !titles.has_label(&TabKey::repo("n1")),
        "the guard is the identity, not the entity id"
    );
}

#[test]
fn a_label_that_arrives_after_the_open_is_what_the_session_stores() {
    let registry = TabRegistry::new();
    let titles = TabTitles::new();
    registry.open(TabKey::task_list("t1"));

    let before = snapshot(registry, titles);
    assert_eq!(
        before.tabs[0].title, "",
        "a tab opened before its label arrives stores the empty label"
    );

    titles.set(TabKey::task_list("t1"), "Fixture Task List".to_string());

    assert_eq!(
        snapshot(registry, titles).tabs[0].title,
        "Fixture Task List",
        "a label that lands mid-session is the label the session stores"
    );
}

// endregion: --- Titles
