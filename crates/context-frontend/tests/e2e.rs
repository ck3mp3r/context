//! Browser end-to-end harness for the SPA.
//!
//! The harness is a standalone cargo test binary: it never links
//! `context_frontend` (that crate is a wasm lib whose cdylib target does not
//! host-link). It builds the app with `trunk build`, serves `dist/` plus a
//! stubbed API from a loopback port, and drives a real Chromium through
//! Playwright.
//!
//! Prerequisite, once per machine:
//!
//! ```text
//! cargo run --manifest-path crates/context-frontend/Cargo.toml \
//!   --features e2e --example install-browsers
//! ```
//!
//! The harness itself:
//!
//! ```text
//! cargo test --manifest-path crates/context-frontend/Cargo.toml \
//!   --features e2e --test e2e
//! ```

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use playwright_rs::{Page, Playwright, expect};
use serde::Deserialize;

/// Project the stubbed API serves, so a real card exists to click.
const PROJECT_ID: &str = "e2e00001";
const PROJECT_TITLE: &str = "E2E Fixture Project";

/// A second project, so a second card can open a second tab.
const SECOND_PROJECT_ID: &str = "e2e00002";
const SECOND_PROJECT_TITLE: &str = "E2E Second Project";

/// The task list the stubbed API serves inside the fixture project, so a child card exists to
/// click.
const TASK_LIST_ID: &str = "e2e00003";

/// The note the stubbed API serves. It names a project whose tab is not open, so a global-list row
/// must open that parent project tab first (decision 8).
const NOTE_ID: &str = "e2e00004";
const NOTE_PARENT_PROJECT_ID: &str = "e2e00009";

/// The note that names no project, so its tab is parentless and its close is the last close.
const PARENTLESS_NOTE_ID: &str = "e2e00007";

/// The port the debug bundle dials for updates (`WS_URL` in `src/websocket.rs`). The stub owns it.
const WS_ADDR: &str = "127.0.0.1:8080";

/// The task the stubbed API serves inside the fixture task list, so a task card exists to open the
/// task detail surface.
const TASK_ID: &str = "e2e00006";

/// The skill the stubbed API serves, so a skill card exists to open the skill detail surface.
const SKILL_ID: &str = "e2e00005";

/// How long the harness waits for the served origin before failing.
const READY_TIMEOUT: Duration = Duration::from_secs(20);

/// A debug wasm bundle is tens of megabytes; first paint is not fast.
const BOOT_TIMEOUT: Duration = Duration::from_secs(90);

/// Budget for an interaction that follows a rendered page.
const UI_TIMEOUT: Duration = Duration::from_secs(30);

/// The SPA runs a debug build, so its API base is the trunk `/dev` prefix.
const API_PREFIX: &str = "/dev/api/v1/";

// region:    --- Test

/// Reads the container's observable state in one round trip.
///
/// `hidden_probe` is the keep-alive witness: the property is set on a live DOM node before the
/// switch, so it can only read back `true` if that same node survived. A remount yields a fresh
/// node without the expando.
const CONTAINER_PROBE: &str = r#"() => {
    const wrappers = Array.from(document.querySelectorAll('[data-tab-id]'));
    const active = wrappers.filter((wrapper) => wrapper.getAttribute('data-tab-active') === 'true');
    const hidden = wrappers.filter((wrapper) => wrapper.getAttribute('data-tab-active') === 'false');
    const shown = active[0];
    const gone = hidden[0];
    return {
        wrappers: wrappers.length,
        active: active.length,
        active_id: shown ? shown.getAttribute('data-tab-id') : '',
        active_display: shown ? getComputedStyle(shown).display : 'missing',
        hidden_id: gone ? gone.getAttribute('data-tab-id') : '',
        hidden_display: gone ? getComputedStyle(gone).display : 'missing',
        hidden_children: gone ? gone.querySelectorAll('[data-tab-view]').length : 0,
        hidden_probe: gone ? gone.__c5t_probe === true : false,
        body_children: document.body.childElementCount,
        history: history.length,
        active_view: shown ? (shown.querySelector('[data-tab-view]')?.getAttribute('data-tab-view') || '') : '',
        active_entity: shown ? (shown.querySelector('[data-tab-entity-id]')?.getAttribute('data-tab-entity-id') || '') : '',
        in_container: wrappers.every((wrapper) =>
            wrapper.parentElement !== null && wrapper.parentElement.hasAttribute('data-tab-container')),
    };
}"#;

/// Reads the routed area: a tab URL has no detail route, so nothing may render there.
const ROUTES_PROBE: &str = r#"() => {
    const area = document.querySelector('[data-routes-area]');
    return {
        present: area !== null,
        children: area ? area.childElementCount : -1,
        text: area ? (area.textContent || '').trim() : '',
    };
}"#;

/// The routed-area state [`ROUTES_PROBE`] returns.
#[derive(Debug, Deserialize)]
struct RoutesProbe {
    present: bool,
    children: usize,
    text: String,
}

/// The container state [`CONTAINER_PROBE`] returns.
#[derive(Debug, Deserialize)]
struct ContainerProbe {
    wrappers: usize,
    active: usize,
    active_id: String,
    active_display: String,
    hidden_id: String,
    hidden_display: String,
    hidden_children: usize,
    hidden_probe: bool,
    body_children: usize,
    history: usize,
    active_view: String,
    active_entity: String,
    in_container: bool,
}

/// Reads every overlay surface: where it renders, whether it paints, and whether it holds the page.
///
/// `body_overlays` counts the nodes a surface would add under `document.body` if it teleported
/// there; decision 12 keeps every surface inside its tab wrapper, so the count stays zero.
const SURFACE_PROBE: &str = r#"() => {
    const surfaces = Array.from(document.querySelectorAll('[data-surface]'));
    const activeTab = document.querySelector('[data-tab-active="true"]');
    const open = surfaces.filter((surface) => getComputedStyle(surface).display !== 'none');
    const shown = open[0];
    const panel = shown ? shown.querySelector('[data-surface-panel]') : null;
    const rect = panel ? panel.getBoundingClientRect() : null;
    const wrapper = shown ? shown.closest('[data-tab-id]') : null;
    const hit = document.elementFromPoint(
        Math.floor(window.innerWidth / 2),
        Math.floor(window.innerHeight / 2),
    );
    return {
        surfaces: surfaces.length,
        open: open.length,
        open_names: open.map((surface) => surface.getAttribute('data-surface')).join(','),
        open_in_active_tab: open.filter((surface) => activeTab && activeTab.contains(surface)).length,
        inside_tab: wrapper ? wrapper.getAttribute('data-tab-id') : '',
        wrapper_display: wrapper ? getComputedStyle(wrapper).display : 'missing',
        panel_width: rect ? Math.round(rect.width) : 0,
        panel_height: rect ? Math.round(rect.height) : 0,
        hit_in_surface: surfaces.some((surface) => surface.contains(hit)),
        focus_in_surface: surfaces.some((surface) => surface.contains(document.activeElement)),
        active_element: document.activeElement
            ? document.activeElement.tagName + '.' + (document.activeElement.className || '')
            : 'none',
        body_overlays: document.querySelectorAll(
            'body > [data-surface], body > [data-surface-backdrop], body > .thaw-dialog, body > .thaw-overlay-drawer-container, body > .thaw-overlay-drawer',
        ).length,
        body_children: document.body.childElementCount,
        scroll_locks: document.querySelectorAll('style[data-c5t-scroll-lock]').length,
    };
}"#;

/// The surface state [`SURFACE_PROBE`] returns.
#[derive(Debug, Deserialize)]
struct SurfaceProbe {
    surfaces: usize,
    open: usize,
    open_names: String,
    open_in_active_tab: usize,
    inside_tab: String,
    wrapper_display: String,
    panel_width: f64,
    panel_height: f64,
    hit_in_surface: bool,
    focus_in_surface: bool,
    active_element: String,
    body_overlays: usize,
    body_children: usize,
    scroll_locks: usize,
}

/// Runs a control's own click handler, without the pointer reaching it.
///
/// An open surface owns the pointer: its backdrop covers the viewport. A tab trigger behind that
/// backdrop is therefore reached through the handler the app itself installed.
const CLICK_THROUGH_PROBE: &str = r#"(selector) => {
    const element = document.querySelector(selector);
    if (!element) return false;
    element.click();
    return true;
}"#;

#[tokio::test(flavor = "multi_thread")]
async fn spa_boots_and_project_card_navigation_reaches_project_detail() {
    // The debug bundle dials the hardcoded `ws://localhost:8080/dev/ws`, so the stub owns that
    // exact port and this page connects to it from its first load. A taken port fails here.
    let socket = WebSocketStub::start();

    let root = repo_root();
    build_spa(&root);

    let dist = root.join("crates/context-server/dist");
    let index = dist.join("index.html");
    assert!(
        index.is_file(),
        "trunk build did not emit {}: the SPA cannot be served",
        index.display()
    );

    let server = StaticServer::start(dist);
    let origin = server.origin();
    wait_for_origin(&server.addr, &origin);
    println!("e2e origin ready: {origin}");

    let playwright = Playwright::launch()
        .await
        .expect("Playwright driver should start");
    let browser = playwright
        .chromium()
        .launch()
        .await
        .expect("Chromium should launch (run the install-browsers example first)");
    let page = browser
        .new_page()
        .await
        .expect("browser should open a page");

    page.goto(&origin, None)
        .await
        .expect("the SPA origin should load in the browser");

    // Smoke 1: the Projects chrome view renders.
    expect(page.locator("[data-page=\"projects\"]"))
        .with_timeout(BOOT_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the Projects page should render");

    // Baselines for the no-relocation and no-history pins, read before any tab exists.
    let body_children_before: usize = page
        .evaluate("() => document.body.childElementCount", None::<&()>)
        .await
        .expect("the page should report its body child count");
    let history_before: usize = page
        .evaluate("() => history.length", None::<&()>)
        .await
        .expect("the page should report its history length");

    let card = page.locator(format!("[data-project-card=\"{PROJECT_ID}\"]"));
    expect(card.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the stubbed project should render as a card");

    // Smoke 2: clicking the card navigates to the project detail surface.
    card.click(None)
        .await
        .expect("the project card should be clickable");
    expect(page.locator("[data-page=\"project-detail\"]"))
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the project detail page should render");

    let expected_path = format!("/projects/{PROJECT_ID}");
    assert!(
        page.url().ends_with(&expected_path),
        "clicking the card should land on {expected_path}, got {}",
        page.url()
    );

    // The project dashboard renders inside the tab wrapper once its payload arrives, and its
    // section pills are internal tab state, so the Notes pill is a button, not a route link.
    expect(page.locator("[data-tab-active=\"true\"] [data-panel=\"notes\"]"))
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the project dashboard should render inside the active tab wrapper");

    // Pin 1 (container): the card opened exactly one tab wrapper, and it is the active one.
    let wrappers = page.locator("[data-tab-id]");
    expect(wrappers.clone())
        .with_timeout(UI_TIMEOUT)
        .to_have_count(1)
        .await
        .expect("opening a project card should render exactly one tab wrapper");
    expect(page.locator("[data-tab-active=\"true\"]"))
        .with_timeout(UI_TIMEOUT)
        .to_have_count(1)
        .await
        .expect("exactly one tab wrapper may be active at a time");

    // Pin 2: `data-tab-id` is the tab's id, so the first tab the registry issues is `1`.
    let first_tab_id = wrappers
        .nth(0)
        .get_attribute("data-tab-id")
        .await
        .expect("the wrapper should expose data-tab-id")
        .unwrap_or_default();
    assert_eq!(
        first_tab_id, "1",
        "the first tab renders as data-tab-id=\"1\", got {first_tab_id:?}"
    );
    expect(page.locator("[data-tab-id=\"1\"] [data-tab-view]"))
        .with_timeout(UI_TIMEOUT)
        .to_have_count(1)
        .await
        .expect("the wrapper should hold the tab's mount slot and render its view");

    let history_after_first: usize = page
        .evaluate("() => history.length", None::<&()>)
        .await
        .expect("the page should report its history length");
    assert_eq!(
        history_after_first, history_before,
        "the card click writes its URL with replace, so no history entry may appear"
    );

    // Pin 3 (keep-alive witness): the probe lives on the DOM node itself. A remount loses it.
    let probe_set: bool = page
        .evaluate(
            "() => {\n                const wrapper = document.querySelector('[data-tab-active=\"true\"]');\n                if (!wrapper) return false;\n                wrapper.__c5t_probe = true;\n                return wrapper.__c5t_probe === true;\n            }",
            None::<&()>,
        )
        .await
        .expect("the page should accept the probe property");
    assert!(
        probe_set,
        "the active wrapper should carry the probe property"
    );

    // The second card lives on the chrome Projects view, so return to chrome first. Chrome
    // navigation never closes a tab (decision 8), which is what makes the witness meaningful.
    page.locator("nav a[href=\"/\"]")
        .first()
        .click(None)
        .await
        .expect("the Projects nav link should return to chrome home");
    let history_after_chrome: usize = page
        .evaluate("() => history.length", None::<&()>)
        .await
        .expect("the page should report its history length");

    let second_card = page.locator(format!("[data-project-card=\"{SECOND_PROJECT_ID}\"]"));
    expect(second_card.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome Projects view should render its cards again");
    second_card
        .click(None)
        .await
        .expect("the second project card should be clickable");

    let expected_second = format!("/projects/{SECOND_PROJECT_ID}");
    assert!(
        page.url().ends_with(&expected_second),
        "the second card should land on {expected_second}, got {}",
        page.url()
    );

    expect(page.locator("[data-tab-id]"))
        .with_timeout(UI_TIMEOUT)
        .to_have_count(2)
        .await
        .expect("the second card should open a second wrapper, not replace the first");

    let probe: ContainerProbe = page
        .evaluate(CONTAINER_PROBE, None::<&()>)
        .await
        .expect("the page should report the container state");

    assert_eq!(
        probe.wrappers, 2,
        "two opened cards should mount two wrappers, got {}",
        probe.wrappers
    );
    assert_eq!(
        probe.active, 1,
        "exactly one wrapper may be active, got {}",
        probe.active
    );
    assert_eq!(
        probe.active_id, "2",
        "the second tab is the active wrapper, got {:?}",
        probe.active_id
    );
    assert_ne!(
        probe.active_display, "none",
        "the active wrapper must paint, got display {:?}",
        probe.active_display
    );
    assert_eq!(
        probe.hidden_id, "1",
        "the first tab is the hidden wrapper, got {:?}",
        probe.hidden_id
    );
    assert_eq!(
        probe.hidden_display, "none",
        "a tab that is not active renders display:none, got {:?}",
        probe.hidden_display
    );
    assert_eq!(
        probe.hidden_children, 1,
        "a hidden tab stays mounted: its view must still be in the DOM"
    );
    assert!(
        probe.hidden_probe,
        "the first wrapper's DOM node must survive the switch; the probe is gone, so it remounted"
    );
    assert!(
        probe.in_container,
        "every wrapper must sit inside the container, never under document.body"
    );
    assert_eq!(
        probe.body_children, body_children_before,
        "opening tabs must not add a node under document.body"
    );
    assert_eq!(
        probe.history, history_after_chrome,
        "the second card click writes its URL with replace, so no history entry may appear"
    );

    // Decision 6: a card for an already-open surface focuses its tab, never a duplicate.
    page.locator("nav a[href=\"/\"]")
        .first()
        .click(None)
        .await
        .expect("the Projects nav link should return to chrome home");
    let first_card = page.locator(format!("[data-project-card=\"{PROJECT_ID}\"]"));
    expect(first_card.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome Projects view should render its cards again");

    let history_before_refocus: usize = page
        .evaluate("() => history.length", None::<&()>)
        .await
        .expect("the page should report its history length");

    first_card
        .click(None)
        .await
        .expect("the first project card should be clickable again");

    let refocus: ContainerProbe = page
        .evaluate(CONTAINER_PROBE, None::<&()>)
        .await
        .expect("the page should report the container state");
    assert_eq!(
        refocus.wrappers, 2,
        "clicking an open card must focus its tab, not duplicate it, got {} wrappers",
        refocus.wrappers
    );
    assert_eq!(
        refocus.active_id, "1",
        "the open surface's tab takes focus, got {:?}",
        refocus.active_id
    );
    assert_eq!(
        refocus.active_view, "project",
        "the focused tab still shows its project, got {:?}",
        refocus.active_view
    );
    assert_eq!(
        refocus.history, history_before_refocus,
        "focusing an open tab writes its URL with replace, so no history entry may appear"
    );

    // Reviewer amendment: the tab URL matches no detail route, so the routed area renders
    // nothing beneath the tab wrapper: no detail view and no fallback paragraph.
    let routes: RoutesProbe = page
        .evaluate(ROUTES_PROBE, None::<&()>)
        .await
        .expect("the page should report the routed area");
    assert!(routes.present, "the routed area must be in the DOM");
    assert_eq!(
        routes.children, 0,
        "with a tab active the routed area must render nothing, got {} element(s): {:?}",
        routes.children, routes.text
    );
    assert!(
        !routes.text.contains("Page not found"),
        "the fallback paragraph must not render under a tab, got {:?}",
        routes.text
    );
    expect(page.locator("[data-page=\"projects\"]"))
        .to_have_count(0)
        .await
        .expect("no leftover chrome view may sit under an active tab");

    // Decision 3 and 20: a panel click changes the tab's own state, never the URL.
    let url_before_panel = page.url();
    let history_before_panel: usize = page
        .evaluate("() => history.length", None::<&()>)
        .await
        .expect("the page should report its history length");

    page.locator("[data-tab-active=\"true\"] [data-panel=\"notes\"]")
        .click(None)
        .await
        .expect("the Notes pill should be clickable");
    expect(page.locator("[data-tab-active=\"true\"] [data-panel-content=\"notes\"]"))
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the panel content should switch to notes");
    let note_in_panel = page.locator(format!(
        "[data-tab-active=\"true\"] [data-note-card=\"{NOTE_ID}\"]",
    ));
    expect(note_in_panel)
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the notes panel should list the fixture note");

    assert_eq!(
        page.url(),
        url_before_panel,
        "a panel click must not rewrite the URL"
    );
    let history_after_panel: usize = page
        .evaluate("() => history.length", None::<&()>)
        .await
        .expect("the page should report its history length");
    assert_eq!(
        history_after_panel, history_before_panel,
        "a panel click must create no history entry"
    );

    page.locator("[data-tab-active=\"true\"] [data-panel=\"task-lists\"]")
        .click(None)
        .await
        .expect("the Task Lists pill should be clickable");
    let task_list_card = page.locator(format!(
        "[data-tab-active=\"true\"] [data-task-list-card=\"{TASK_LIST_ID}\"]",
    ));
    expect(task_list_card.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the task lists panel should list the fixture task list");

    // Decisions 7 and 8: a child card opens a child tab with its parent project tab present.
    let history_before_child: usize = page
        .evaluate("() => history.length", None::<&()>)
        .await
        .expect("the page should report its history length");

    task_list_card
        .click(None)
        .await
        .expect("the task list card should be clickable");

    let child: ContainerProbe = page
        .evaluate(CONTAINER_PROBE, None::<&()>)
        .await
        .expect("the page should report the container state");
    assert_eq!(
        child.wrappers, 3,
        "the child card must add one wrapper beside the two project tabs, got {}",
        child.wrappers
    );
    assert_eq!(
        child.active_id, "3",
        "the child tab is the active one, got {:?}",
        child.active_id
    );
    assert_eq!(
        child.active_view, "task-list",
        "the active tab shows a task list, got {:?}",
        child.active_view
    );
    assert_eq!(
        child.active_entity, TASK_LIST_ID,
        "the active tab shows the clicked task list, got {:?}",
        child.active_entity
    );
    expect(page.locator("[data-tab-id=\"1\"]"))
        .to_have_count(1)
        .await
        .expect("the parent project tab must still be open beside the child");
    assert_eq!(
        child.history, history_before_child,
        "the child open writes its URL with replace, so no history entry may appear"
    );

    let child_url = page.url();
    let expected_child_path = format!("/projects/{PROJECT_ID}/task-lists/{TASK_LIST_ID}");
    assert!(
        child_url.ends_with(&expected_child_path),
        "opening the child should write {expected_child_path}, got {child_url}"
    );
    for banned in ["page=", "q=", "sort=", "order="] {
        assert!(
            !child_url.contains(banned),
            "the URL carries surface identity only: {banned} must not appear, got {child_url}"
        );
    }

    // Decision 6 again: neither an open project tab nor an open child tab is duplicated.
    page.locator("nav a[href=\"/\"]")
        .first()
        .click(None)
        .await
        .expect("the Projects nav link should return to chrome home");
    expect(first_card.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome Projects view should render its cards again");
    first_card
        .click(None)
        .await
        .expect("the first project card should focus its open tab");

    let project_refocus: ContainerProbe = page
        .evaluate(CONTAINER_PROBE, None::<&()>)
        .await
        .expect("the page should report the container state");
    assert_eq!(
        project_refocus.wrappers, 3,
        "focusing the project tab must not duplicate it, got {} wrappers",
        project_refocus.wrappers
    );
    assert_eq!(
        project_refocus.active_id, "1",
        "the project tab takes focus back, got {:?}",
        project_refocus.active_id
    );

    task_list_card
        .click(None)
        .await
        .expect("the task list card should be clickable again");
    let child_again: ContainerProbe = page
        .evaluate(CONTAINER_PROBE, None::<&()>)
        .await
        .expect("the page should report the container state");
    assert_eq!(
        child_again.wrappers, 3,
        "clicking an open child card must not duplicate it, got {} wrappers",
        child_again.wrappers
    );
    assert_eq!(
        child_again.active_id, "3",
        "the open child tab takes focus back, got {:?}",
        child_again.active_id
    );

    // Decision 11: the app never reads the address bar after boot. A URL mutation it did not
    // make moves no active tab.
    let pushed: bool = page
        .evaluate(
            "() => { history.pushState({}, '', '/some/other/route'); return true; }",
            None::<&()>,
        )
        .await
        .expect("the page should accept a pushed history entry");
    assert!(pushed, "the pushed history entry should be applied");

    let after_push: ContainerProbe = page
        .evaluate(CONTAINER_PROBE, None::<&()>)
        .await
        .expect("the page should report the container state");
    assert_eq!(
        after_push.active_id, "3",
        "a pushed URL must not change the active tab, got {:?}",
        after_push.active_id
    );
    assert_eq!(
        after_push.wrappers, 3,
        "a pushed URL must not open or close a tab, got {} wrappers",
        after_push.wrappers
    );
    assert!(
        page.url().ends_with("/some/other/route"),
        "the app must not rewrite a URL it did not write, got {}",
        page.url()
    );

    // Decision 21: Back moves no active tab, because the app created no in-app history.
    let _ = page
        .go_back(None)
        .await
        .expect("the browser should step back");

    let after_back: ContainerProbe = page
        .evaluate(CONTAINER_PROBE, None::<&()>)
        .await
        .expect("the page should report the container state");
    assert_eq!(
        after_back.active_id, "3",
        "Back must not change the active tab, got {:?}",
        after_back.active_id
    );
    assert_eq!(
        after_back.wrappers, 3,
        "Back must not open or close a tab, got {} wrappers",
        after_back.wrappers
    );
    let routes_after_back: RoutesProbe = page
        .evaluate(ROUTES_PROBE, None::<&()>)
        .await
        .expect("the page should report the routed area");
    assert_eq!(
        routes_after_back.children, 0,
        "a tab URL reached through Back still renders nothing in the routed area"
    );

    // Decision 8: a global-list row opens the parent project tab first, then the child.
    page.locator("nav a[href=\"/notes\"]")
        .first()
        .click(None)
        .await
        .expect("the Notes nav link should reach the chrome notes list");
    let note_card = page.locator(format!("[data-routes-area] [data-note-card=\"{NOTE_ID}\"]"));
    expect(note_card.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome notes list should render the fixture note card");

    note_card
        .click(None)
        .await
        .expect("the note card should be clickable");

    let after_note: ContainerProbe = page
        .evaluate(CONTAINER_PROBE, None::<&()>)
        .await
        .expect("the page should report the container state");
    assert_eq!(
        after_note.wrappers, 5,
        "the row must open the parent project tab first, then the note tab: expected 5 wrappers, got {}",
        after_note.wrappers
    );
    assert_eq!(
        after_note.active_id, "5",
        "the note tab is the active one, got {:?}",
        after_note.active_id
    );
    assert_eq!(
        after_note.active_view, "note",
        "the active tab shows a note, got {:?}",
        after_note.active_view
    );
    expect(page.locator(format!(
        "[data-tab-id=\"4\"] [data-tab-view=\"project\"][data-tab-entity-id=\"{NOTE_PARENT_PROJECT_ID}\"]",
    )))
        .to_have_count(1)
        .await
        .expect("the named project tab opens before the note tab");

    let note_url = page.url();
    let expected_note_path = format!("/projects/{NOTE_PARENT_PROJECT_ID}/notes/{NOTE_ID}");
    assert!(
        note_url.ends_with(&expected_note_path),
        "the note tab should write {expected_note_path}, got {note_url}"
    );

    // === Inline surfaces (decision 12): a surface renders inside its tab wrapper ===

    // The project tab hosts both surfaces: return to chrome and focus it.
    page.locator("nav a[href=\"/\"]")
        .first()
        .click(None)
        .await
        .expect("the Projects nav link should return to chrome home");
    let host_card = page.locator(format!("[data-project-card=\"{PROJECT_ID}\"]"));
    expect(host_card.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome Projects view should render its cards again");
    host_card
        .click(None)
        .await
        .expect("the project card should focus its open tab");

    // Pin (inline dialog): a task card opens the detail surface inside the task list tab.
    page.locator("[data-tab-active=\"true\"] [data-panel=\"task-lists\"]")
        .click(None)
        .await
        .expect("the Task Lists pill should be clickable");
    let host_task_list = page.locator(format!(
        "[data-tab-active=\"true\"] [data-task-list-card=\"{TASK_LIST_ID}\"]",
    ));
    expect(host_task_list.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the task lists panel should list the fixture task list");
    host_task_list
        .click(None)
        .await
        .expect("the task list card should focus its open tab");

    // The stub serves one task, so every column renders it; any card opens a task detail surface.
    let task_card = page
        .locator(format!(
            "[data-tab-active=\"true\"] [data-task-card=\"{TASK_ID}\"]",
        ))
        .first();
    expect(task_card.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the task list tab should render the fixture task card");
    task_card
        .click(None)
        .await
        .expect("the task card should open the task detail surface");

    let dialog = wait_for_open_surface(&page, "task-detail").await;
    assert_eq!(
        dialog.open, 1,
        "exactly one task detail surface may open, got {}",
        dialog.open
    );
    assert!(
        !dialog.inside_tab.is_empty(),
        "the task detail surface must render inside a tab wrapper, not under document.body"
    );
    assert_eq!(
        dialog.body_overlays, 0,
        "no dialog, overlay, or backdrop node may render under document.body"
    );
    assert_eq!(
        dialog.body_children, body_children_before,
        "opening a surface must not add a node under document.body"
    );
    assert!(
        dialog.panel_width > 0.0 && dialog.panel_height > 0.0,
        "the open panel must paint, got {}x{}",
        dialog.panel_width,
        dialog.panel_height
    );
    assert_eq!(
        dialog.scroll_locks, 1,
        "an open surface in the active tab holds the page scroll"
    );
    assert!(
        dialog.focus_in_surface,
        "the open surface takes focus, so the page behind it gets no keystroke; focus sits on {}",
        dialog.active_element
    );

    // A Tab press stays inside the surface, which is the focus containment the active tab needs.
    page.keyboard()
        .press("Tab", None)
        .await
        .expect("the page should accept the Tab key");
    let tabbed = surface_probe(&page).await;
    assert!(
        tabbed.focus_in_surface,
        "a Tab press must not move focus behind the open surface"
    );

    // Escape closes the surface and releases the page.
    page.keyboard()
        .press("Escape", None)
        .await
        .expect("the page should accept the Escape key");
    let escaped = wait_for_closed_surface(&page).await;
    assert_eq!(
        escaped.scroll_locks, 0,
        "closing the surface must release the page scroll"
    );

    // Pin (backdrop): a click on the mask closes the surface too.
    task_card
        .click(None)
        .await
        .expect("the task card should reopen the task detail surface");
    wait_for_open_surface(&page, "task-detail").await;
    page.mouse()
        .click(5.0, 5.0, None)
        .await
        .expect("the mouse should click the backdrop");
    wait_for_closed_surface(&page).await;

    // Pin (inline drawer): a skill card opens the detail surface inside the project tab.
    page.locator("nav a[href=\"/\"]")
        .first()
        .click(None)
        .await
        .expect("the Projects nav link should return to chrome home");
    expect(host_card.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome Projects view should render its cards again");
    host_card
        .click(None)
        .await
        .expect("the project card should focus its open tab");

    page.locator("[data-tab-active=\"true\"] [data-panel=\"skills\"]")
        .click(None)
        .await
        .expect("the Skills pill should be clickable");
    let skill_card = page.locator(format!(
        "[data-tab-active=\"true\"] [data-skill-card=\"{SKILL_ID}\"]",
    ));
    expect(skill_card.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the skills panel should list the fixture skill");
    skill_card
        .click(None)
        .await
        .expect("the skill card should open the skill detail surface");

    let drawer = wait_for_open_surface(&page, "skill-detail").await;
    assert!(
        !drawer.inside_tab.is_empty(),
        "the skill detail surface must render inside a tab wrapper, not under document.body"
    );
    assert_eq!(
        drawer.body_overlays, 0,
        "no dialog, overlay, or backdrop node may render under document.body"
    );
    assert_eq!(
        drawer.body_children, body_children_before,
        "opening a drawer must not add a node under document.body"
    );
    assert!(
        drawer.panel_width > 0.0 && drawer.panel_height > 0.0,
        "the open drawer must paint, got {}x{}",
        drawer.panel_width,
        drawer.panel_height
    );
    assert_eq!(
        drawer.scroll_locks, 1,
        "an open drawer in the active tab holds the page scroll"
    );

    page.keyboard()
        .press("Escape", None)
        .await
        .expect("the page should accept the Escape key");
    wait_for_closed_surface(&page).await;

    skill_card
        .click(None)
        .await
        .expect("the skill card should reopen the skill detail surface");
    wait_for_open_surface(&page, "skill-detail").await;
    page.mouse()
        .click(5.0, 5.0, None)
        .await
        .expect("the mouse should click the drawer backdrop");
    wait_for_closed_surface(&page).await;

    // Pin (decision 13): switching tabs keeps the open surface, paints it nowhere, and releases
    // the page; switching back restores it. The surface's backdrop covers the viewport, so the tab
    // trigger behind it runs its own click handler.
    skill_card
        .click(None)
        .await
        .expect("the skill card should reopen the skill detail surface");
    wait_for_open_surface(&page, "skill-detail").await;

    click_through(&page, "[data-tab-active=\"true\"] [data-panel=\"task-lists\"]").await;
    click_through(
        &page,
        &format!("[data-tab-id=\"1\"] [data-task-list-card=\"{TASK_LIST_ID}\"]"),
    )
    .await;

    let hidden = surface_probe(&page).await;
    assert_eq!(
        hidden.surfaces, drawer.surfaces,
        "the tab switch must not add or remove a surface node"
    );
    assert_eq!(
        hidden.open, 1,
        "the open drawer must survive the tab switch"
    );
    assert_eq!(
        hidden.open_names, "skill-detail",
        "the surviving surface is the skill drawer, got {:?}",
        hidden.open_names
    );
    assert_eq!(
        hidden.open_in_active_tab, 0,
        "a surface of a hidden tab belongs to no active tab"
    );
    assert_eq!(
        hidden.wrapper_display, "none",
        "the hidden tab's wrapper is display:none, got {:?}",
        hidden.wrapper_display
    );
    assert_eq!(
        (hidden.panel_width, hidden.panel_height),
        (0.0, 0.0),
        "a hidden surface must paint nowhere, got {}x{}",
        hidden.panel_width,
        hidden.panel_height
    );
    assert!(
        !hidden.hit_in_surface,
        "a hidden surface must not be hit-testable"
    );
    assert_eq!(
        hidden.scroll_locks, 0,
        "a hidden tab's open surface must not hold the page scroll"
    );
    assert_eq!(
        hidden.body_overlays, 0,
        "a tab switch must not move a surface under document.body"
    );

    // The pointer reaches the page again, because the hidden surface is inert.
    page.locator("nav a[href=\"/\"]")
        .first()
        .click(None)
        .await
        .expect("the Projects nav link should be clickable while a hidden tab holds a surface");
    expect(host_card.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome Projects view should render its cards again");
    host_card
        .click(None)
        .await
        .expect("the project card should take focus back");

    let restored = wait_for_open_surface(&page, "skill-detail").await;
    assert_eq!(
        restored.inside_tab, drawer.inside_tab,
        "the surface returns to the tab it was opened in, got {:?}",
        restored.inside_tab
    );
    assert!(
        restored.panel_width > 0.0 && restored.panel_height > 0.0,
        "the surface must paint again after the tab takes focus back, got {}x{}",
        restored.panel_width,
        restored.panel_height
    );
    assert_eq!(
        restored.scroll_locks, 1,
        "the restored surface holds the page scroll again"
    );

    page.keyboard()
        .press("Escape", None)
        .await
        .expect("the page should accept the Escape key");
    let closed_after_return = wait_for_closed_surface(&page).await;
    assert_eq!(
        closed_after_return.scroll_locks, 0,
        "closing the restored surface must release the page scroll"
    );

    // === The websocket deletion reaction ===

    // The reload starts an empty strip, which is what the last-close path needs, and re-opens the
    // stub's socket because the debug bundle dials it on mount.
    let connections = socket.connections();
    page.goto(&origin, None)
        .await
        .expect("the SPA origin should load again for the deletion pins");
    expect(page.locator("[data-page=\"projects\"]"))
        .with_timeout(BOOT_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the Projects page should render on the reloaded SPA");
    socket.wait_for_connection_after(connections);

    // The note names a project, so opening it opens the project tab first: two tabs, note active.
    page.locator("nav a[href=\"/notes\"]")
        .first()
        .click(None)
        .await
        .expect("the Notes nav link should reach the chrome notes list");
    let deletion_note = page.locator(format!("[data-note-card=\"{NOTE_ID}\"]"));
    expect(deletion_note.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome notes list should render the fixture note card");
    deletion_note
        .click(None)
        .await
        .expect("the note card should open its tab");

    let open_note =
        wait_for_container(&page, "the note tab open beside its project tab", |probe| {
            probe.wrappers == 2 && probe.active_view == "note"
        })
        .await;
    let note_tab = open_note.active_id.clone();

    // A non-delete update keeps the tabs: each tab's own refetch arm stays the owner of those.
    socket.send(&format!(
        "{{\"type\":\"NoteUpdated\",\"data\":{{\"note_id\":\"{NOTE_ID}\"}}}}"
    ));
    settle_frame();
    let after_update = container_probe(&page).await;
    assert_eq!(
        after_update.wrappers, 2,
        "a NoteUpdated must neither open nor close a tab, got {} wrappers",
        after_update.wrappers
    );
    assert_eq!(
        after_update.active_id, note_tab,
        "a NoteUpdated must not move focus, got {:?}",
        after_update.active_id
    );

    // The deletion closes the note's tab; the project tab survives it and takes focus.
    socket.send(&format!(
        "{{\"type\":\"NoteDeleted\",\"data\":{{\"note_id\":\"{NOTE_ID}\"}}}}"
    ));
    let after_delete = wait_for_container(&page, "the deleted note's tab closed", |probe| {
        probe.wrappers == 1
    })
    .await;
    assert_eq!(
        after_delete.active, 1,
        "the surviving tab must be the active one, got {} active wrappers",
        after_delete.active
    );
    assert_ne!(
        after_delete.active_id, note_tab,
        "the closed tab must not keep focus, got {:?}",
        after_delete.active_id
    );
    assert_eq!(
        after_delete.active_view, "project",
        "the surviving tab is the note's project, got {:?}",
        after_delete.active_view
    );
    expect(page.locator(format!(
        "[data-tab-view=\"note\"][data-tab-entity-id=\"{NOTE_ID}\"]",
    )))
    .with_timeout(UI_TIMEOUT)
    .to_have_count(0)
    .await
    .expect("the deleted note's wrapper should be gone");

    // A surviving tab paints nothing beneath it: no detail view and no not-found view.
    let routes_after_delete: RoutesProbe = page
        .evaluate(ROUTES_PROBE, None::<&()>)
        .await
        .expect("the page should report the routed area");
    assert!(
        routes_after_delete.present,
        "the routed area must stay in the tree"
    );
    assert_eq!(
        routes_after_delete.children, 0,
        "a surviving tab renders nothing in the routed area, got {:?}",
        routes_after_delete.text
    );
    assert!(
        !routes_after_delete.text.contains("Page not found"),
        "the deletion must not paint a not-found view, got {:?}",
        routes_after_delete.text
    );

    // The reaction navigates nowhere: the one writer names the surviving tab, never a chrome list.
    let url_after_delete = page.url();
    assert!(
        !url_after_delete.ends_with("/notes")
            && !url_after_delete.ends_with("/repos")
            && !url_after_delete.ends_with("/task-lists"),
        "the deletion must not write a chrome list route, got {url_after_delete}"
    );
    let surviving_path = format!("/projects/{NOTE_PARENT_PROJECT_ID}");
    assert!(
        url_after_delete.ends_with(&surviving_path),
        "the one writer names the surviving tab {surviving_path}, got {url_after_delete}"
    );

    // A deletion that names no open tab changes nothing at all.
    socket.send("{\"type\":\"NoteDeleted\",\"data\":{\"note_id\":\"e2e000ff\"}}");
    settle_frame();
    let after_unknown = container_probe(&page).await;
    assert_eq!(
        after_unknown.wrappers, 1,
        "a deletion for a surface with no open tab must not close a tab, got {} wrappers",
        after_unknown.wrappers
    );
    assert_eq!(
        after_unknown.active_id, after_delete.active_id,
        "a deletion for a surface with no open tab must not move focus"
    );
    assert!(
        page.url().ends_with(&surviving_path),
        "a deletion for a surface with no open tab must not navigate, got {}",
        page.url()
    );

    // The last-close path needs an empty strip, so the page reloads again and opens the parentless
    // note, whose only tab it is.
    let connections = socket.connections();
    page.goto(&origin, None)
        .await
        .expect("the SPA origin should load again for the last-close pin");
    expect(page.locator("[data-page=\"projects\"]"))
        .with_timeout(BOOT_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the Projects page should render before the last-close pin");
    socket.wait_for_connection_after(connections);

    page.locator("nav a[href=\"/notes\"]")
        .first()
        .click(None)
        .await
        .expect("the Notes nav link should reach the chrome notes list");
    let parentless = page.locator(format!("[data-note-card=\"{PARENTLESS_NOTE_ID}\"]",));
    expect(parentless.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome notes list should render the parentless note card");
    parentless
        .click(None)
        .await
        .expect("the parentless note card should open its tab");

    wait_for_container(&page, "the parentless note's tab alone", |probe| {
        probe.wrappers == 1 && probe.active_view == "note"
    })
    .await;
    let parentless_path = format!("/notes/{PARENTLESS_NOTE_ID}");
    assert!(
        page.url().ends_with(&parentless_path),
        "a parentless note opens without a project segment, got {}",
        page.url()
    );

    socket.send(&format!(
        "{{\"type\":\"NoteDeleted\",\"data\":{{\"note_id\":\"{PARENTLESS_NOTE_ID}\"}}}}"
    ));
    let emptied =
        wait_for_container(&page, "the last tab closed", |probe| probe.wrappers == 0).await;
    assert_eq!(
        emptied.active, 0,
        "the last close leaves no active tab, got {} active wrappers",
        emptied.active
    );
    expect(page.locator("[data-page=\"projects\"]"))
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the last close returns to chrome home");
    assert!(
        page.url().ends_with('/'),
        "the last close writes chrome home, got {}",
        page.url()
    );

    browser.close().await.expect("the browser should close");
    drop(socket);
    drop(server);
}

// endregion: --- Test

// region:    --- Support

/// Reads the surface state in one round trip.
async fn surface_probe(page: &Page) -> SurfaceProbe {
    page.evaluate(SURFACE_PROBE, None::<&()>)
        .await
        .expect("the page should report the surface state")
}

/// Waits for a surface to be open and painted in the active tab.
async fn wait_for_open_surface(page: &Page, name: &str) -> SurfaceProbe {
    let deadline = Instant::now() + UI_TIMEOUT;
    loop {
        let probe = surface_probe(page).await;
        if probe.open == 1 && probe.open_names == name && probe.open_in_active_tab == 1 {
            return probe;
        }
        assert!(
            Instant::now() < deadline,
            "the {name} surface never opened in the active tab: {probe:?}"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

/// Waits until no surface paints.
async fn wait_for_closed_surface(page: &Page) -> SurfaceProbe {
    let deadline = Instant::now() + UI_TIMEOUT;
    loop {
        let probe = surface_probe(page).await;
        if probe.open == 0 {
            return probe;
        }
        assert!(
            Instant::now() < deadline,
            "a surface stayed open: {probe:?}"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

/// Reads the container state in one round trip.
async fn container_probe(page: &Page) -> ContainerProbe {
    page.evaluate(CONTAINER_PROBE, None::<&()>)
        .await
        .expect("the page should report the container state")
}

/// Polls the container until `predicate` holds, so a websocket frame has time to reach the app.
async fn wait_for_container(
    page: &Page,
    what: &str,
    predicate: impl Fn(&ContainerProbe) -> bool,
) -> ContainerProbe {
    let deadline = Instant::now() + UI_TIMEOUT;
    loop {
        let probe = container_probe(page).await;
        if predicate(&probe) {
            return probe;
        }
        assert!(
            Instant::now() < deadline,
            "the container never showed {what}: {probe:?}"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

/// Gives one frame on the loopback socket time to reach the app, so a "nothing changed" pin means
/// what it says. A frame on loopback lands in microseconds; this margin is two orders wider.
fn settle_frame() {
    thread::sleep(Duration::from_millis(500));
}

/// Runs a control's own click handler, for a control an open surface's backdrop covers.
async fn click_through(page: &Page, selector: &str) {
    expect(page.locator(selector))
        .with_timeout(UI_TIMEOUT)
        .to_have_count(1)
        .await
        .unwrap_or_else(|error| panic!("{selector} should exist before its click: {error}"));

    let clicked: bool = page
        .evaluate(CLICK_THROUGH_PROBE, Some(&selector))
        .await
        .expect("the page should run the control's click handler");
    assert!(clicked, "{selector} should be reachable through its handler");
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root should resolve from CARGO_MANIFEST_DIR")
}

/// Compile the app with trunk. A failure carries the tail of trunk's stderr.
fn build_spa(root: &Path) {
    let output = Command::new("trunk")
        .arg("build")
        .current_dir(root)
        .output()
        .unwrap_or_else(|error| panic!("`trunk build` could not start: {error}"));

    if output.status.success() {
        return;
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let tail: Vec<&str> = stderr.lines().rev().take(20).collect();
    let tail: Vec<&str> = tail.into_iter().rev().collect();
    panic!(
        "`trunk build` failed ({}):\n{}",
        output.status,
        tail.join("\n")
    );
}

fn wait_for_origin(addr: &SocketAddr, origin: &str) {
    let deadline = Instant::now() + READY_TIMEOUT;
    while Instant::now() < deadline {
        if origin_answers(addr) {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("the origin {origin} did not answer within {READY_TIMEOUT:?}");
}

fn origin_answers(addr: &SocketAddr) -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(addr, Duration::from_millis(250)) else {
        return false;
    };
    if stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .is_err()
    {
        return false;
    }

    let request = format!("GET / HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut head = [0u8; 32];
    match stream.read(&mut head) {
        Ok(read) => String::from_utf8_lossy(&head[..read]).starts_with("HTTP/1.1 200"),
        Err(_) => false,
    }
}

/// Serves `dist/` plus the stubbed API from a loopback port the OS assigns.
///
/// The server runs on its own thread inside this test process, so it cannot
/// outlive the run: `Drop` stops the accept loop and joins the thread.
struct StaticServer {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl StaticServer {
    fn start(dist: PathBuf) -> Self {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("the e2e harness should bind a loopback port");
        listener
            .set_nonblocking(true)
            .expect("the e2e listener should switch to non-blocking mode");
        let addr = listener
            .local_addr()
            .expect("the bound listener should report its address");

        let shutdown = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&shutdown);
        let thread = thread::spawn(move || accept_loop(listener, &dist, &stop));

        Self {
            addr,
            shutdown,
            thread: Some(thread),
        }
    }

    fn origin(&self) -> String {
        format!("http://{}", self.addr)
    }
}

impl Drop for StaticServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take()
            && handle.join().is_err()
        {
            eprintln!("e2e static server thread panicked during teardown");
        }
    }
}

fn accept_loop(listener: TcpListener, dist: &Path, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => serve_connection(stream, dist),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(2));
            }
            Err(error) => {
                eprintln!("e2e static server stopped accepting: {error}");
                return;
            }
        }
    }
}

fn serve_connection(mut stream: TcpStream, dist: &Path) {
    // A socket accepted from a non-blocking listener inherits the flag, and a
    // non-blocking `write_all` fails with EWOULDBLOCK once the send buffer is
    // full, which truncates every large asset.
    if stream.set_nonblocking(false).is_err() {
        return;
    }

    let Some(target) = read_request(&stream) else {
        return;
    };
    let path = target.split('?').next().unwrap_or("/");

    let response = route(path, dist);
    write_response(&mut stream, response);
    close_gracefully(&mut stream);
}

/// Reads the request head and drains the body, so closing the socket cannot
/// reset the connection and truncate the response the browser is reading.
fn read_request(stream: &TcpStream) -> Option<String> {
    let mut reader = BufReader::new(stream);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return None;
    }

    let mut parts = request_line.split_whitespace();
    parts.next()?;
    let target = parts.next()?.to_string();

    let mut body_len = 0;
    let mut header = String::new();
    loop {
        header.clear();
        let read = reader.read_line(&mut header).ok()?;
        if read == 0 || header == "\r\n" || header == "\n" {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            body_len = value.trim().parse().unwrap_or(0);
        }
    }

    if body_len > 0 {
        let mut body = vec![0u8; body_len];
        reader.read_exact(&mut body).ok()?;
    }

    Some(target)
}

/// Half-closes the connection and drains the peer's side, so the kernel does
/// not answer our close with a reset that discards the queued response.
fn close_gracefully(stream: &mut TcpStream) {
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));

    let mut sink = [0u8; 1024];
    loop {
        match stream.read(&mut sink) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
    }
}

// region:    --- WebSocket stub

/// The stub websocket the debug bundle dials, on the hardcoded port `src/websocket.rs` names.
///
/// One accepted client is kept, so the test can send a text frame into the running SPA; a reload's
/// socket replaces the one before it. The port is fixed, so a busy port fails the run with that
/// reason instead of moving elsewhere.
struct WebSocketStub {
    client: Arc<Mutex<Option<TcpStream>>>,
    accepted: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WebSocketStub {
    /// Binds `WS_ADDR` and answers opening handshakes. A taken port is a hard failure.
    fn start() -> Self {
        let listener = TcpListener::bind(WS_ADDR).unwrap_or_else(|error| {
            panic!(
                "the e2e websocket stub needs {WS_ADDR}, because the debug bundle hardcodes \
                 ws://localhost:8080/dev/ws, and the port is not free: {error}"
            )
        });
        listener
            .set_nonblocking(true)
            .expect("the e2e websocket listener should switch to non-blocking mode");

        let client = Arc::new(Mutex::new(None));
        let accepted = Arc::new(AtomicUsize::new(0));
        let shutdown = Arc::new(AtomicBool::new(false));

        let slot = Arc::clone(&client);
        let count = Arc::clone(&accepted);
        let stop = Arc::clone(&shutdown);
        let thread = thread::spawn(move || ws_accept_loop(listener, &slot, &count, &stop));

        Self {
            client,
            accepted,
            shutdown,
            thread: Some(thread),
        }
    }

    /// How many sockets the stub accepted, so a reload's own socket is the one that follows it.
    fn connections(&self) -> usize {
        self.accepted.load(Ordering::Relaxed)
    }

    /// Waits until a socket after `previous` arrives, which is the page's own connection.
    fn wait_for_connection_after(&self, previous: usize) {
        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            let accepted = self.connections();
            if accepted > previous {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the app never opened ws://localhost:8080/dev/ws (accepted {accepted} sockets)"
            );
            thread::sleep(Duration::from_millis(25));
        }
    }

    /// Writes one text frame to the latest accepted socket.
    fn send(&self, text: &str) {
        let mut slot = self
            .client
            .lock()
            .expect("the stub's client slot should not be poisoned");
        let stream = slot
            .as_mut()
            .unwrap_or_else(|| panic!("the stub has no open websocket to carry {text:?}"));
        if let Err(error) = stream.write_all(&text_frame(text)) {
            panic!("the stub could not write a frame to the app's websocket: {error}");
        }
        let _ = stream.flush();
    }
}

impl Drop for WebSocketStub {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take()
            && handle.join().is_err()
        {
            eprintln!("e2e websocket stub thread panicked during teardown");
        }
    }
}

fn ws_accept_loop(
    listener: TcpListener,
    client: &Mutex<Option<TcpStream>>,
    accepted: &AtomicUsize,
    stop: &AtomicBool,
) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let Some(stream) = ws_handshake(stream) else {
                    continue;
                };
                if let Ok(mut slot) = client.lock() {
                    *slot = Some(stream);
                }
                accepted.fetch_add(1, Ordering::Relaxed);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(2));
            }
            Err(error) => {
                eprintln!("e2e websocket stub stopped accepting: {error}");
                return;
            }
        }
    }
}

/// Answers the RFC 6455 opening handshake and keeps the socket for a later frame.
fn ws_handshake(mut stream: TcpStream) -> Option<TcpStream> {
    if stream.set_nonblocking(false).is_err() {
        return None;
    }
    if stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .is_err()
    {
        return None;
    }

    let key = read_websocket_key(&stream)?;
    let head = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        websocket_accept(&key)
    );
    if stream.write_all(head.as_bytes()).is_err() {
        return None;
    }
    let _ = stream.flush();

    Some(stream)
}

/// The client's `Sec-WebSocket-Key` from the opening handshake.
fn read_websocket_key(stream: &TcpStream) -> Option<String> {
    let mut reader = BufReader::new(stream);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return None;
    }
    if !request_line.starts_with("GET ") {
        return None;
    }

    let mut key = None;
    loop {
        let mut header = String::new();
        let read = reader.read_line(&mut header).ok()?;
        if read == 0 || header == "\r\n" || header == "\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("sec-websocket-key")
        {
            key = Some(value.trim().to_string());
        }
    }

    key
}

/// One final text frame in the server-to-client direction, which RFC 6455 leaves unmasked.
fn text_frame(text: &str) -> Vec<u8> {
    let payload = text.as_bytes();
    let mut frame = Vec::with_capacity(payload.len() + 4);
    frame.push(0x81);
    if payload.len() < 126 {
        frame.push(payload.len() as u8);
    } else {
        assert!(
            payload.len() <= usize::from(u16::MAX),
            "one stub frame carries at most 64 KiB"
        );
        frame.push(126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    }
    frame.extend_from_slice(payload);
    frame
}

/// The RFC 6455 accept token: `base64(SHA1(key + GUID))`.
fn websocket_accept(key: &str) -> String {
    let mut material = key.to_string();
    material.push_str("258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    base64(&sha1(material.as_bytes()))
}

/// SHA-1 of `bytes`, which is the hash the accept token covers.
fn sha1(bytes: &[u8]) -> [u8; 20] {
    let mut digest: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];

    let bit_len = (bytes.len() as u64) * 8;
    let mut message = bytes.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in message.as_chunks::<64>().0 {
        let mut words = [0u32; 80];
        for (index, word) in chunk.as_chunks::<4>().0.iter().enumerate() {
            words[index] = u32::from_be_bytes(*word);
        }
        for index in 16..80 {
            words[index] =
                (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                    .rotate_left(1);
        }

        let [mut a, mut b, mut c, mut d, mut e] = digest;
        for (index, word) in words.iter().enumerate() {
            let (f, k) = match index {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        digest[0] = digest[0].wrapping_add(a);
        digest[1] = digest[1].wrapping_add(b);
        digest[2] = digest[2].wrapping_add(c);
        digest[3] = digest[3].wrapping_add(d);
        digest[4] = digest[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (index, word) in digest.iter().enumerate() {
        out[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// Standard base64 with padding, the alphabet the accept token uses.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let triple = (u32::from(chunk[0]) << 16)
            | (u32::from(chunk.get(1).copied().unwrap_or(0)) << 8)
            | u32::from(chunk.get(2).copied().unwrap_or(0));
        encoded.push(char::from(ALPHABET[((triple >> 18) & 0x3f) as usize]));
        encoded.push(char::from(ALPHABET[((triple >> 12) & 0x3f) as usize]));
        encoded.push(match chunk.len() {
            1 => '=',
            _ => char::from(ALPHABET[((triple >> 6) & 0x3f) as usize]),
        });
        encoded.push(match chunk.len() {
            1 | 2 => '=',
            _ => char::from(ALPHABET[(triple & 0x3f) as usize]),
        });
    }
    encoded
}

/// RFC 6455 section 1.3 is the one published accept-token vector, so it pins the handshake helpers.
#[test]
fn websocket_accept_matches_the_rfc_example() {
    assert_eq!(
        websocket_accept("dGhlIHNhbXBsZSBub25jZQ=="),
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
}

// endregion: --- WebSocket stub

fn route(path: &str, dist: &Path) -> Response {
    if let Some(api_path) = path.strip_prefix(API_PREFIX) {
        return api_response(api_path);
    }

    // The SPA dials `ws://localhost:8080/dev/ws`, which the harness serves from its own listener
    // (`WebSocketStub`). This origin answers the path with a 404, so a bundle that ever dialed a
    // relative path fails loudly here instead of reading HTML as a frame.
    if path.starts_with("/dev/ws") {
        return Response::text(404, "e2e stub: the websocket lives on 127.0.0.1:8080");
    }

    static_response(dist, path)
}

fn api_response(path: &str) -> Response {
    match path {
        "projects" => Response::json(200, &project_page()),
        "task-lists" => Response::json(200, &task_list_page()),
        "notes" => Response::json(200, &note_page()),
        "repos" => Response::json(200, &empty_page()),
        "skills" => Response::json(200, &skill_page()),
        "update/status" => Response::json(
            200,
            &format!(
                "{{\"current_version\":\"{}\",\"status\":\"up_to_date\"}}",
                env!("CARGO_PKG_VERSION")
            ),
        ),
        "update/check" => Response::json(200, "{}"),
        _ => match record_by_path(path) {
            Some(body) => Response::json(200, &body),
            None => Response::json(
                404,
                "{\"error\":\"e2e stub: endpoint not served\",\"details\":null}",
            ),
        },
    }
}

/// One record the stub serves by id, when the path names one it knows.
fn record_by_path(path: &str) -> Option<String> {
    if let Some(body) = project_by_path(path) {
        return Some(body);
    }
    if path == format!("task-lists/{TASK_LIST_ID}/stats") {
        return Some(task_stats_record());
    }
    if path == format!("task-lists/{TASK_LIST_ID}") {
        return Some(task_list_record());
    }
    if path == format!("task-lists/{TASK_LIST_ID}/tasks") {
        return Some(task_page());
    }
    if path == format!("tasks/{TASK_ID}") {
        return Some(task_record());
    }
    if path == format!("skills/{SKILL_ID}") {
        return Some(skill_record());
    }
    if path == format!("notes/{NOTE_ID}") {
        return Some(note_record());
    }
    if path == format!("notes/{PARENTLESS_NOTE_ID}") {
        return Some(parentless_note_record());
    }
    None
}

/// The project detail response for `projects/<id>`, when the stub serves that project.
fn project_by_path(path: &str) -> Option<String> {
    let id = path.strip_prefix("projects/")?;
    match id {
        PROJECT_ID => Some(project_record(PROJECT_ID, PROJECT_TITLE)),
        SECOND_PROJECT_ID => Some(project_record(SECOND_PROJECT_ID, SECOND_PROJECT_TITLE)),
        _ => None,
    }
}

fn project_record(id: &str, title: &str) -> String {
    format!(
        "{{\"id\":\"{id}\",\"title\":\"{title}\",\"description\":\"Fixture served by the e2e harness.\",\"tags\":[\"e2e\"],\"external_refs\":[],\"created_at\":\"2026-01-01 00:00:00\",\"updated_at\":\"2026-01-01 00:00:00\"}}"
    )
}

fn project_page() -> String {
    format!(
        "{{\"items\":[{},{}],\"total\":2,\"limit\":12,\"offset\":0}}",
        project_record(PROJECT_ID, PROJECT_TITLE),
        project_record(SECOND_PROJECT_ID, SECOND_PROJECT_TITLE)
    )
}

fn empty_page() -> String {
    "{\"items\":[],\"total\":0,\"limit\":12,\"offset\":0}".to_string()
}

fn task_list_record() -> String {
    format!(
        "{{\"id\":\"{TASK_LIST_ID}\",\"project_id\":\"{PROJECT_ID}\",\"title\":\"E2E Fixture Task List\",\"description\":\"Fixture served by the e2e harness.\",\"status\":\"active\",\"external_refs\":[],\"notes\":null,\"tags\":[\"e2e\"],\"repo_ids\":[],\"created_at\":\"2026-01-01 00:00:00\",\"updated_at\":\"2026-01-01 00:00:00\"}}"
    )
}

fn task_list_page() -> String {
    format!(
        "{{\"items\":[{}],\"total\":1,\"limit\":12,\"offset\":0}}",
        task_list_record()
    )
}

fn task_stats_record() -> String {
    format!(
        "{{\"list_id\":\"{TASK_LIST_ID}\",\"total\":1,\"backlog\":0,\"todo\":1,\"in_progress\":0,\"review\":0,\"done\":0,\"cancelled\":0}}"
    )
}

/// The task inside the fixture task list, so a column holds a card to open the detail surface.
fn task_record() -> String {
    format!(
        "{{\"id\":\"{TASK_ID}\",\"list_id\":\"{TASK_LIST_ID}\",\"parent_id\":null,\"title\":\"E2E Fixture Task\",\"description\":\"Fixture task served by the e2e harness.\",\"status\":\"todo\",\"priority\":3,\"tags\":[\"e2e\"],\"external_refs\":[],\"created_at\":\"2026-01-01 00:00:00\",\"updated_at\":\"2026-01-01 00:00:00\"}}"
    )
}

fn task_page() -> String {
    format!(
        "{{\"items\":[{}],\"total\":1,\"limit\":25,\"offset\":0}}",
        task_record()
    )
}

/// The fixture skill, so the project dashboard lists a card to open the skill detail surface.
fn skill_record() -> String {
    format!(
        "{{\"id\":\"{SKILL_ID}\",\"name\":\"E2E Fixture Skill\",\"description\":\"Fixture skill served by the e2e harness.\",\"content\":\"Fixture skill body.\",\"tags\":[\"e2e\"],\"project_ids\":[\"{PROJECT_ID}\"],\"created_at\":\"2026-01-01 00:00:00\",\"updated_at\":\"2026-01-01 00:00:00\"}}"
    )
}

fn skill_page() -> String {
    format!(
        "{{\"items\":[{}],\"total\":1,\"limit\":12,\"offset\":0}}",
        skill_record()
    )
}

/// The fixture note names a project whose tab is not open, so the global list row must open that
/// project's tab first (decision 8).
fn note_record() -> String {
    format!(
        "{{\"id\":\"{NOTE_ID}\",\"title\":\"E2E Fixture Note\",\"content\":\"Fixture note served by the e2e harness.\",\"tags\":[\"e2e\"],\"project_ids\":[\"{NOTE_PARENT_PROJECT_ID}\"],\"repo_ids\":[],\"created_at\":\"2026-01-01 00:00:00\",\"updated_at\":\"2026-01-01 00:00:00\"}}"
    )
}

fn note_page() -> String {
    format!(
        "{{\"items\":[{},{}],\"total\":2,\"limit\":12,\"offset\":0}}",
        note_record(),
        parentless_note_record()
    )
}

/// The fixture note that names no project, so its tab is parentless and its close is the last close.
fn parentless_note_record() -> String {
    format!(
        "{{\"id\":\"{PARENTLESS_NOTE_ID}\",\"title\":\"E2E Parentless Note\",\"content\":\"Parentless note served by the e2e harness.\",\"tags\":[],\"project_ids\":[],\"repo_ids\":[],\"created_at\":\"2026-01-01 00:00:00\",\"updated_at\":\"2026-01-01 00:00:00\"}}"
    )
}

fn static_response(dist: &Path, path: &str) -> Response {
    let index = dist.join("index.html");
    let relative = path.trim_start_matches('/');

    if relative.is_empty() {
        return Response::file(&index);
    }
    if relative.split('/').any(|segment| segment == "..") {
        return Response::text(404, "e2e stub: path escapes the dist directory");
    }

    let candidate = dist.join(relative);
    if candidate.is_file() {
        return Response::file(&candidate);
    }

    // Client-side routes have no file of their own: hand back the SPA shell.
    Response::file(&index)
}

struct Response {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

impl Response {
    fn json(status: u16, body: &str) -> Self {
        Self {
            status,
            content_type: "application/json",
            body: body.as_bytes().to_vec(),
        }
    }

    fn text(status: u16, body: &str) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            body: body.as_bytes().to_vec(),
        }
    }

    fn file(path: &Path) -> Self {
        let Ok(body) = fs::read(path) else {
            return Self::text(500, "e2e stub: dist file unreadable");
        };
        Self {
            status: 200,
            content_type: content_type(path),
            body,
        }
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

fn write_response(stream: &mut TcpStream, response: Response) {
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        response.status,
        reason_phrase(response.status),
        response.content_type,
        response.body.len()
    );

    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    let _ = stream.write_all(&response.body);
    let _ = stream.flush();
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Error",
    }
}

// endregion: --- Support
