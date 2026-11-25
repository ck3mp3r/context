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
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use playwright_rs::protocol::{ClickOptions, MouseButton};
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
const TASK_LIST_TITLE: &str = "E2E Fixture Task List";

/// The note the stubbed API serves. It names a project whose tab is not open, so a global-list row
/// must open that parent project tab first (decision 8).
const NOTE_ID: &str = "e2e00004";
const NOTE_PARENT_PROJECT_ID: &str = "e2e00009";
const NOTE_PARENT_PROJECT_TITLE: &str = "E2E Note Parent Project";

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

/// The session key a debug bundle stores under (decision 14): namespaced per environment.
const SESSION_KEY: &str = "c5t.tabs.session.dev.v1";

/// A project id no stub route serves, so a restored tab for it answers 404 (decision 14).
const BOGUS_ID: &str = "e2e00fff";

/// A task list no tab shows, so its update message names another entity (decision 15).
const OTHER_TASK_LIST_ID: &str = "e2e00f01";

/// A note no tab shows, so its update message names another entity (decision 15).
const OTHER_NOTE_ID: &str = "e2e00f02";

/// The candidate the stubbed updater announces, and the newer one that must bring a dismissed
/// pill back.
const UPDATE_VERSION: &str = "0.9.0";
const UPDATE_NEWER_VERSION: &str = "0.9.1";

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

/// Writes one storage key from an argument pair, so a boot restores the session it holds.
const WRITE_SESSION_PROBE: &str = r#"([key, value]) => {
    localStorage.setItem(key, value);
    return true;
}"#;

/// Removes one storage key, so the next boot has no session to restore.
const CLEAR_SESSION_PROBE: &str = r#"(key) => {
    localStorage.removeItem(key);
    return true;
}"#;

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

/// Reads the strip: whether it shows, which row each chip sits in, and how each chip is built.
///
/// `separated` is the structural pin: it is true only when the `×` control and the activate control
/// both exist and the `×` control is not a descendant of the activate control.
const STRIP_PROBE: &str = r#"() => {
    const root = document.querySelector('[data-strip]');
    const rows = Array.from(document.querySelectorAll('[data-strip-row]')).map((row) => ({
        name: row.getAttribute('data-strip-row') || '',
        chips: Array.from(row.querySelectorAll('[data-strip-tab]')).map((chip) => {
            const activate = chip.querySelector('[data-tab-activate]');
            const close = chip.querySelector('[data-tab-close]');
            return {
                id: chip.getAttribute('data-strip-tab') || '',
                kind: chip.getAttribute('data-strip-kind') || '',
                active: chip.getAttribute('data-strip-active') === 'true',
                label: activate ? (activate.textContent || '').trim() : '',
                separated: activate !== null && close !== null
                    && close.closest('[data-tab-activate]') === null,
            };
        }),
    }));
    return {
        present: root !== null,
        visible: root ? getComputedStyle(root).display !== 'none' : false,
        rows: rows,
    };
}"#;

/// The strip state [`STRIP_PROBE`] returns.
#[derive(Debug, Deserialize)]
struct StripProbe {
    present: bool,
    visible: bool,
    rows: Vec<StripRowProbe>,
}

/// One strip row and the chips it holds.
#[derive(Debug, Deserialize)]
struct StripRowProbe {
    name: String,
    chips: Vec<StripChipProbe>,
}

/// One tab chip: its id, its surface kind, whether it is the active chip, its label, and whether its
/// close control is structurally separate from its activate control.
#[derive(Debug, Deserialize)]
struct StripChipProbe {
    id: String,
    kind: String,
    active: bool,
    label: String,
    separated: bool,
}

/// Reads every tab wrapper: its id, visibility, and the surface mounted inside it.
const TABS_PROBE: &str = r#"() => Array.from(document.querySelectorAll('[data-tab-id]')).map((wrapper) => ({
    id: wrapper.getAttribute('data-tab-id') || '',
    active: wrapper.getAttribute('data-tab-active') === 'true',
    display: getComputedStyle(wrapper).display,
    view: wrapper.querySelector('[data-tab-view]')
        ? wrapper.querySelector('[data-tab-view]').getAttribute('data-tab-view')
        : '',
    entity: wrapper.querySelector('[data-tab-entity-id]')
        ? wrapper.querySelector('[data-tab-entity-id]').getAttribute('data-tab-entity-id')
        : '',
}))"#;

/// The wrapper state [`TABS_PROBE`] returns.
#[derive(Debug, Deserialize)]
struct TabEntry {
    id: String,
    active: bool,
    display: String,
    view: String,
    entity: String,
}

/// Reads the path the address bar holds, without touching history.
const PATH_PROBE: &str = r#"() => location.pathname"#;

/// Dispatches one `keydown` on the window, exactly as the webview delivers it.
///
/// The return value is `defaultPrevented`: it is true only when the app's own handler ran and claimed
/// the chord. A plain Chromium claims Cmd+W and Ctrl+Tab before the page sees them, so this probe is
/// the Tier-1 witness that the handler is wired (decision 17).
const KEYDOWN_PROBE: &str = r#"([key, meta, ctrl, shift, repeat]) => {
    const event = new KeyboardEvent('keydown', {
        key: key,
        bubbles: true,
        cancelable: true,
        metaKey: meta,
        ctrlKey: ctrl,
        shiftKey: shift,
        repeat: repeat,
    });
    window.dispatchEvent(event);
    return event.defaultPrevented;
}"#;

/// The update pill, its controls, and its place in the stacking order.
///
/// `pill_z`, `nav_z`, and `surface_z` are the computed z-index values of the pill, the app nav, and
/// the open inline surface. The hit test reads the pointer at the pill's own centre, which is how a
/// surface covering the pill proves the stacking order the CSS declares.
const PILL_PROBE: &str = r#"() => {
    const pill = document.querySelector('[data-update-pill]');
    const nav = document.querySelector('nav');
    const surface = Array.from(document.querySelectorAll('[data-surface]'))
        .find((candidate) => getComputedStyle(candidate).display !== 'none');
    const depth = (element) => {
        if (!element) return null;
        const value = getComputedStyle(element).zIndex;
        return value === 'auto' ? 0 : parseInt(value, 10);
    };
    const rect = pill ? pill.getBoundingClientRect() : null;
    const hit = rect
        ? document.elementFromPoint(
            Math.floor(rect.left + rect.width / 2),
            Math.floor(rect.top + rect.height / 2),
        )
        : null;
    const sentence = pill ? pill.querySelector('[data-update-text]') : null;
    const text = sentence ? (sentence.textContent || '').trim() : '';
    return {
        present: pill !== null,
        phase: pill ? (pill.getAttribute('data-update-phase') || '') : '',
        text: text,
        percent: text.includes('%'),
        spinners: pill ? pill.querySelectorAll('[data-update-spinner]').length : 0,
        thaw_spinners: pill ? pill.querySelectorAll('.thaw-spinner').length : 0,
        progress_bars: pill
            ? Array.from(pill.querySelectorAll('progress, [role="progressbar"]'))
                .filter((node) => !node.classList.contains('thaw-spinner'))
                .length
            : 0,
        update_cta: pill ? pill.querySelectorAll('[data-update-install]').length : 0,
        restart_cta: pill ? pill.querySelectorAll('[data-update-restart]').length : 0,
        dismiss_cta: pill ? pill.querySelectorAll('[data-update-dismiss]').length : 0,
        position: pill ? getComputedStyle(pill).position : '',
        pill_z: depth(pill),
        nav_z: depth(nav),
        surface_z: depth(surface),
        right_gap: rect ? Math.round(window.innerWidth - rect.right) : -1,
        bottom_gap: rect ? Math.round(window.innerHeight - rect.bottom) : -1,
        in_right_half: rect ? rect.left > window.innerWidth / 2 : false,
        in_bottom_half: rect ? rect.top > window.innerHeight / 2 : false,
        hit_in_pill: pill && hit ? pill.contains(hit) : false,
        hit_in_surface: surface && hit ? surface.contains(hit) : false,
    };
}"#;

/// The pill state [`PILL_PROBE`] returns.
#[derive(Debug, Deserialize)]
struct PillProbe {
    present: bool,
    phase: String,
    text: String,
    percent: bool,
    spinners: usize,
    thaw_spinners: usize,
    progress_bars: usize,
    update_cta: usize,
    restart_cta: usize,
    dismiss_cta: usize,
    position: String,
    pill_z: Option<i32>,
    nav_z: Option<i32>,
    surface_z: Option<i32>,
    right_gap: i32,
    bottom_gap: i32,
    in_right_half: bool,
    in_bottom_half: bool,
    hit_in_pill: bool,
    hit_in_surface: bool,
}

/// Stack the harness body runs on.
///
/// The whole async state machine of the e2e body lives in `block_on`'s frame, and a debug build
/// makes it big: the body of this test needs about 1 MiB before the deepest interaction runs. A
/// libtest test thread gets 2 MiB, so the harness sat close to its own limit and a rebuild could
/// tip it over. An explicit stack keeps the margin independent of the toolchain's frame sizes.
const HARNESS_STACK: usize = 32 * 1024 * 1024;

#[test]
fn spa_boots_and_project_card_navigation_reaches_project_detail() {
    let body = thread::Builder::new()
        .name("spa-e2e".to_string())
        .stack_size(HARNESS_STACK)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("the e2e runtime should build")
                .block_on(spa_body());
        })
        .expect("the e2e harness thread should start");

    if body.join().is_err() {
        panic!("the spa e2e body panicked");
    }
}

async fn spa_body() {
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

    // Pin (parent chip labelled mid-session): the note card opened a project tab this session had
    // never opened, so the open path resolves its label beside the note's. The parent chip renders
    // the served title, and no chip keeps the "<Kind> <id>" fallback.
    let labelled = wait_for_strip(&page, "the parent project chip labelled", |probe| {
        probe.visible
            && strip_row(probe, "projects")
                .iter()
                .any(|chip| chip.label == NOTE_PARENT_PROJECT_TITLE)
            && strip_chips(probe)
                .iter()
                .all(|chip| !chip.label.contains("e2e000"))
    })
    .await;
    let project_titles: Vec<&str> = strip_row(&labelled, "projects")
        .iter()
        .map(|chip| chip.label.as_str())
        .collect();
    assert_eq!(
        project_titles,
        vec![
            PROJECT_TITLE,
            SECOND_PROJECT_TITLE,
            NOTE_PARENT_PROJECT_TITLE
        ],
        "the parent the note card opened renders its title, not the id fallback: {labelled:?}"
    );
    assert!(
        strip_chips(&labelled)
            .iter()
            .all(|chip| !chip.label.is_empty()),
        "no chip renders a blank label: {labelled:?}"
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

    click_through(
        &page,
        "[data-tab-active=\"true\"] [data-panel=\"task-lists\"]",
    )
    .await;
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
    // A reload now restores the session (decision 14), so the pins that need a chrome boot clear it
    // first.
    clear_session(&page).await;
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
    clear_session(&page).await;
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

    // Decision 14, corrupt: one garbage value under the namespaced key drops to chrome, with no
    // error, no toast, and no tab. The last close left the URL on chrome home.
    write_session(&page, "not a session").await;
    let connections = socket.connections();
    page.goto(&origin, None)
        .await
        .expect("the SPA origin should load again with a corrupt session");
    expect(page.locator("[data-page=\"projects\"]"))
        .with_timeout(BOOT_TIMEOUT)
        .to_be_visible()
        .await
        .expect("a corrupt session boots to chrome home");
    expect(page.locator("[data-tab-id]"))
        .to_have_count(0)
        .await
        .expect("a corrupt session restores no tab");
    socket.wait_for_connection_after(connections);

    // Decision 14, restore: the project tab and its task list child, the child active. A reload of
    // that URL rebuilds both in stored order with the child active, and mounts the child alone.
    let restored_project = page.locator(format!("[data-project-card=\"{PROJECT_ID}\"]"));
    expect(restored_project.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome Projects view should render the fixture card");
    restored_project
        .click(None)
        .await
        .expect("the project card should open its tab");
    wait_for_container(&page, "the project tab open", |probe| {
        probe.wrappers == 1 && probe.active_view == "project"
    })
    .await;

    let restored_task_list = page.locator(format!(
        "[data-tab-active=\"true\"] [data-task-list-card=\"{TASK_LIST_ID}\"]",
    ));
    expect(restored_task_list.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the project dashboard should list the fixture task list");
    restored_task_list
        .click(None)
        .await
        .expect("the task list card should open its child tab");
    wait_for_container(&page, "two stored tabs", |probe| {
        probe.wrappers == 2 && probe.active_view == "task-list"
    })
    .await;

    let deep_link = page.url();
    let project_path = format!("projects/{PROJECT_ID}");
    let project_calls_before = server.api_calls(&project_path);

    let connections = socket.connections();
    page.goto(&deep_link, None)
        .await
        .expect("the deep link should load again");
    let restored = wait_for_container(&page, "the stored strip rebuilt", |probe| {
        probe.wrappers == 2 && probe.active_view == "task-list"
    })
    .await;
    socket.wait_for_connection_after(connections);

    assert_eq!(
        restored.active_id, "2",
        "tab ids are issued in stored order, so the child keeps its place, got {:?}",
        restored.active_id
    );
    assert_eq!(
        restored.active_entity, TASK_LIST_ID,
        "the reload rebuilds the same active surface, got {:?}",
        restored.active_entity
    );
    assert_eq!(
        restored.hidden_id, "1",
        "the project tab is the first stored tab, got {:?}",
        restored.hidden_id
    );
    assert_eq!(
        restored.hidden_display, "none",
        "a restored tab that is not active paints nowhere"
    );
    assert_eq!(
        restored.hidden_children, 0,
        "a restored hidden tab mounts on first activation only, so it holds no view at boot"
    );
    assert_eq!(
        page.url(),
        deep_link,
        "boot restore writes no URL: the deep link it opened is still the URL"
    );

    wait_for_api_calls(&server, &project_path, project_calls_before + 1);
    settle_frame();
    assert_eq!(
        server.api_calls(&project_path),
        project_calls_before + 1,
        "the hidden tab costs one label request and no surface fetch"
    );

    // The hidden tab mounts on its first activation, through the card that focuses it (decision 6).
    page.locator("nav a[href=\"/\"]")
        .first()
        .click(None)
        .await
        .expect("the Projects nav link should reach chrome home");
    let focus_project = page.locator(format!("[data-project-card=\"{PROJECT_ID}\"]"));
    expect(focus_project.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome Projects view should render the fixture card again");
    focus_project
        .click(None)
        .await
        .expect("the project card should focus the restored tab");
    let activated = wait_for_container(&page, "the restored tab mounted", |probe| {
        probe.wrappers == 2 && probe.active_view == "project"
    })
    .await;
    assert_eq!(
        activated.active_id, "1",
        "the stored project tab takes focus, got {:?}",
        activated.active_id
    );
    wait_for_api_calls(&server, &project_path, project_calls_before + 2);
    assert!(
        server.api_calls(&project_path) > project_calls_before + 1,
        "the restored tab fetches its surface on activation, and only then"
    );

    // Decision 14, 404: a stored surface the server no longer has is dropped, without a reload. The
    // session names one valid project and one bogus id, and the bogus one was the active tab.
    page.locator("nav a[href=\"/\"]")
        .first()
        .click(None)
        .await
        .expect("the Projects nav link should reach chrome home before the 404 session");
    write_session(&page, &stale_session()).await;

    let document_loads = server.calls("/");
    page.goto(&origin, None)
        .await
        .expect("the SPA origin should load again with a stale session");
    let dropped = wait_for_container(&page, "the missing surface dropped", |probe| {
        probe.wrappers == 1 && probe.active_entity == PROJECT_ID
    })
    .await;

    assert_eq!(
        dropped.active_view, "project",
        "the surviving tab is the project the server still has, got {:?}",
        dropped.active_view
    );
    expect(page.locator(format!(
        "[data-tab-view=\"project\"][data-tab-entity-id=\"{BOGUS_ID}\"]",
    )))
    .with_timeout(UI_TIMEOUT)
    .to_have_count(0)
    .await
    .expect("the missing surface must leave no wrapper behind");
    wait_for_api_calls(&server, &format!("projects/{BOGUS_ID}"), 1);
    assert_eq!(
        server.calls("/"),
        document_loads + 1,
        "the dropped tab must not reload the page"
    );

    // Decision 14, fresh deep link: no stored session, and a boot URL naming a note under its
    // project. The boot open enqueues the parent project beside the surface it opens, so the strip
    // labels that parent instead of rendering the "<Kind> <id>" fallback.
    clear_session(&page).await;
    let fresh_boot_path = format!("/projects/{NOTE_PARENT_PROJECT_ID}/notes/{NOTE_ID}");
    let connections = socket.connections();
    page.goto(&format!("{origin}{fresh_boot_path}"), None)
        .await
        .expect("the SPA should load the fresh deep link");
    let fresh_boot =
        wait_for_container(&page, "the deep-linked note beside its project", |probe| {
            probe.wrappers == 2 && probe.active_view == "note"
        })
        .await;
    socket.wait_for_connection_after(connections);

    assert_eq!(
        fresh_boot.active_entity, NOTE_ID,
        "the boot URL's own surface is the active tab, got {:?}",
        fresh_boot.active_entity
    );
    assert!(
        page.url().ends_with(&fresh_boot_path),
        "boot writes no URL: the deep link it opened is still the URL, got {}",
        page.url()
    );

    let boot_strip = wait_for_strip(&page, "the fresh boot's parent project labelled", |probe| {
        probe.visible
            && strip_row(probe, "projects")
                .iter()
                .any(|chip| chip.label == NOTE_PARENT_PROJECT_TITLE)
            && strip_chips(probe)
                .iter()
                .all(|chip| !chip.label.contains("e2e000"))
    })
    .await;
    let boot_project_titles: Vec<&str> = strip_row(&boot_strip, "projects")
        .iter()
        .map(|chip| chip.label.as_str())
        .collect();
    assert_eq!(
        boot_project_titles,
        vec![NOTE_PARENT_PROJECT_TITLE],
        "a fresh boot labels the parent project it opened, not its id: {boot_strip:?}"
    );

    // === Decision 15: scoped liveness ===
    //
    // A panel the user never opened fetches nothing; a hidden tab keeps the data it loaded live on
    // its own entity's message; another entity's message refetches nothing; one socket serves every
    // tab. The counts come from the stub's request log, so each pin is a fetch count and not a
    // side effect of the UI.
    let connections = socket.connections();
    clear_session(&page).await;
    page.goto(&origin, None)
        .await
        .expect("the SPA origin should load for the liveness pins");
    expect(page.locator("[data-page=\"projects\"]"))
        .with_timeout(BOOT_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the Projects page should render before the liveness pins");
    socket.wait_for_connection_after(connections);
    let sockets_with_no_tab = socket.connections();

    let task_lists_before = server.api_calls("task-lists");
    let notes_before = server.api_calls("notes");
    let repos_before = server.api_calls("repos");
    let skills_before = server.api_calls("skills");
    let project_path = format!("projects/{PROJECT_ID}");
    let project_calls_before = server.api_calls(&project_path);

    let liveness_card = page.locator(format!("[data-project-card=\"{PROJECT_ID}\"]"));
    expect(liveness_card.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the fixture project card should render for the liveness pins");
    liveness_card
        .click(None)
        .await
        .expect("the project card should open its tab");
    wait_for_container(
        &page,
        "the project tab open for the liveness pins",
        |probe| probe.wrappers == 1 && probe.active_view == "project",
    )
    .await;

    // The visible panel loads once; the three panels the user has not opened fetch nothing.
    wait_for_api_calls(&server, "task-lists", task_lists_before + 1);
    settle_frame();
    assert_eq!(
        server.api_calls("task-lists"),
        task_lists_before + 1,
        "the panel on screen fetches exactly once at mount"
    );
    assert_eq!(
        server.api_calls("notes"),
        notes_before,
        "the notes panel was never opened, so it fetches nothing"
    );
    assert_eq!(
        server.api_calls("repos"),
        repos_before,
        "the repos panel was never opened, so it fetches nothing"
    );
    assert_eq!(
        server.api_calls("skills"),
        skills_before,
        "the skills panel was never opened, so it fetches nothing"
    );
    assert_eq!(
        server.api_calls(&project_path),
        project_calls_before + 2,
        "the card click costs one label GET and the tab's own entity fetch, with no request for its panels"
    );

    // Opening the notes panel is what loads it, and only then.
    page.locator("[data-tab-active=\"true\"] [data-panel=\"notes\"]")
        .click(None)
        .await
        .expect("the Notes pill should be clickable");
    wait_for_api_calls(&server, "notes", notes_before + 1);
    expect(page.locator(format!(
        "[data-tab-active=\"true\"] [data-note-card=\"{NOTE_ID}\"]",
    )))
    .with_timeout(UI_TIMEOUT)
    .to_be_visible()
    .await
    .expect("the notes panel should list the fixture note once it is opened");

    // A task-list message refetches the task-list panel, which has loaded, and the notes panel,
    // which is now hidden, stays as it is: each panel answers only its own kind of message.
    let task_lists_loaded = server.api_calls("task-lists");
    let notes_loaded = server.api_calls("notes");
    socket.send(&format!(
        "{{\"type\":\"TaskListUpdated\",\"data\":{{\"task_list_id\":\"{OTHER_TASK_LIST_ID}\"}}}}"
    ));
    wait_for_api_calls(&server, "task-lists", task_lists_loaded + 1);
    settle_frame();
    assert_eq!(
        server.api_calls("task-lists"),
        task_lists_loaded + 1,
        "a hidden panel that already loaded refetches on its own kind of message"
    );
    assert_eq!(
        server.api_calls("notes"),
        notes_loaded,
        "a task-list message refetches no notes view"
    );

    // One socket serves the whole app: opening a second tab opens no second connection.
    page.locator("nav a[href=\"/notes\"]")
        .first()
        .click(None)
        .await
        .expect("the Notes nav link should reach the chrome notes list");
    let parentless_card = page.locator(format!(
        "[data-routes-area] [data-note-card=\"{PARENTLESS_NOTE_ID}\"]",
    ));
    expect(parentless_card.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome notes list should render the parentless note card");

    let note_path = format!("notes/{PARENTLESS_NOTE_ID}");
    let note_calls_before = server.api_calls(&note_path);
    parentless_card
        .click(None)
        .await
        .expect("the parentless note card should open its tab");
    wait_for_container(&page, "the note tab active over the project tab", |probe| {
        probe.wrappers == 2 && probe.active_view == "note"
    })
    .await;
    wait_for_api_calls(&server, &note_path, note_calls_before + 1);
    assert_eq!(
        socket.connections(),
        sockets_with_no_tab,
        "two tabs share the one websocket: no per-tab connection"
    );

    // The note surface loads its body and then its selected-note view, so both requests land before
    // the counts below are read.
    settle_frame();

    // Another entity's message refetches nothing: neither the hidden project tab nor the visible
    // note tab answers a message that names an entity it does not show.
    let note_calls_loaded = server.api_calls(&note_path);
    let project_calls_loaded = server.api_calls(&project_path);
    socket.send(&format!(
        "{{\"type\":\"ProjectUpdated\",\"data\":{{\"project_id\":\"{SECOND_PROJECT_ID}\"}}}}"
    ));
    settle_frame();
    assert_eq!(
        server.api_calls(&project_path),
        project_calls_loaded,
        "a message naming another project refetches no hidden project tab"
    );
    assert_eq!(
        server.api_calls(&note_path),
        note_calls_loaded,
        "a project message refetches no note tab"
    );

    // The hidden tab's own message still refreshes the data it already loaded (decision 15).
    socket.send(&format!(
        "{{\"type\":\"ProjectUpdated\",\"data\":{{\"project_id\":\"{PROJECT_ID}\"}}}}"
    ));
    wait_for_api_calls(&server, &project_path, project_calls_loaded + 1);
    assert_eq!(
        server.api_calls(&note_path),
        note_calls_loaded,
        "the hidden tab's own refresh costs the visible tab nothing"
    );

    // The visible note tab answers its own note's message and no other note's.
    socket.send(&format!(
        "{{\"type\":\"NoteUpdated\",\"data\":{{\"note_id\":\"{OTHER_NOTE_ID}\"}}}}"
    ));
    settle_frame();
    assert_eq!(
        server.api_calls(&note_path),
        note_calls_loaded,
        "a note message naming another note refetches no note tab"
    );
    socket.send(&format!(
        "{{\"type\":\"NoteUpdated\",\"data\":{{\"note_id\":\"{PARENTLESS_NOTE_ID}\"}}}}"
    ));
    wait_for_api_calls(&server, &note_path, note_calls_loaded + 1);
    assert_eq!(
        socket.connections(),
        sockets_with_no_tab,
        "a refetch opens no second websocket"
    );

    // === The strip and its keyboard (decisions 2, 5, 9, 10, 16, 17) ===
    //
    // The strip renders two levels, every close leaves a visible survivor, the `×` control sits beside
    // the activate control rather than inside it, and the ring and the two Cmd chords reach the same
    // registry as the strip.

    let connections = socket.connections();
    clear_session(&page).await;
    page.goto(&origin, None)
        .await
        .expect("the SPA origin should load for the strip pins");
    expect(page.locator("[data-page=\"projects\"]"))
        .with_timeout(BOOT_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the Projects page should render before the strip pins");
    socket.wait_for_connection_after(connections);

    // Pin (chrome home): no tab is open, so the strip paints nothing at all.
    let empty_strip = strip_probe(&page).await;
    assert!(
        empty_strip.present,
        "the strip stays in the tree, so it can show itself when a tab opens"
    );
    assert!(
        !empty_strip.visible,
        "chrome home hides the strip: {empty_strip:?}"
    );
    assert!(
        strip_chips(&empty_strip).is_empty(),
        "an empty strip holds no chip"
    );

    // Pin (one project tab): row one holds its chip, and the chip's close control is a sibling.
    let strip_project = page.locator(format!("[data-project-card=\"{PROJECT_ID}\"]"));
    expect(strip_project.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome Projects view should render the fixture card");
    strip_project
        .click(None)
        .await
        .expect("the project card should open its tab");
    let one_tab = wait_for_container(&page, "the project tab open for the strip pins", |probe| {
        probe.wrappers == 1 && probe.active_view == "project"
    })
    .await;

    let strip = wait_for_strip(&page, "one project chip", |probe| {
        probe.visible && strip_chips(probe).len() == 1
    })
    .await;
    let projects = strip_row(&strip, "projects");
    assert_eq!(projects.len(), 1, "row one holds the project chip");
    assert_eq!(projects[0].kind, "project");
    assert_eq!(
        projects[0].id, one_tab.active_id,
        "the chip names the tab it activates"
    );
    assert!(projects[0].active, "the open tab's chip is the active chip");
    assert!(
        projects[0].separated,
        "the close control is a sibling of the activate control, never its child"
    );
    assert!(
        !projects[0].label.is_empty(),
        "a chip never renders a blank label"
    );
    assert!(
        !has_strip_row(&strip, "children"),
        "a project with no child row renders no level-two row"
    );
    assert!(
        !has_strip_row(&strip, "parentless"),
        "a project tab is not parentless"
    );

    // Pin (two levels): the child tab lands in row two while its project holds row one.
    let strip_task_list = page.locator(format!(
        "[data-tab-active=\"true\"] [data-task-list-card=\"{TASK_LIST_ID}\"]",
    ));
    expect(strip_task_list.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the project dashboard should list the fixture task list");
    strip_task_list
        .click(None)
        .await
        .expect("the task list card should open its child tab");
    let two_tabs = wait_for_container(&page, "the task list child tab open", |probe| {
        probe.wrappers == 2 && probe.active_view == "task-list"
    })
    .await;

    let strip = wait_for_strip(&page, "the child chip in row two", |probe| {
        strip_chips(probe).len() == 2
    })
    .await;
    let projects = strip_row(&strip, "projects");
    let children = strip_row(&strip, "children");
    assert_eq!(projects.len(), 1, "row one keeps the project chip");
    assert_eq!(projects[0].kind, "project");
    assert!(
        !projects[0].active,
        "the project chip is not the active chip any more"
    );
    assert_eq!(
        children.len(),
        1,
        "row two holds the active project's child"
    );
    assert_eq!(children[0].kind, "task-list");
    assert_eq!(children[0].id, two_tabs.active_id);
    assert!(children[0].active);
    assert!(
        projects.iter().all(|chip| chip.kind == "project")
            && children.iter().all(|chip| chip.kind != "project"),
        "no row mixes a project tab with a child"
    );

    // Pin (a chip labelled mid-session): both cards were clicked after chrome home, so neither tab
    // had a label. The open path resolves each identity once, so both chips render the server title
    // and no chip falls back to "<Kind> <id>" without a reload.
    let labelled = wait_for_strip(&page, "both chips labelled by their titles", |probe| {
        strip_row(probe, "projects")
            .first()
            .is_some_and(|chip| chip.label == PROJECT_TITLE)
            && strip_row(probe, "children")
                .first()
                .is_some_and(|chip| chip.label == TASK_LIST_TITLE)
    })
    .await;
    assert!(
        strip_chips(&labelled)
            .iter()
            .all(|chip| !chip.label.contains("e2e000")),
        "no chip falls back to the entity id once its label lands: {labelled:?}"
    );

    // Pin (no second GET): a tab whose label is known costs nothing when it is re-activated or
    // re-opened. The request log is the witness, so a duplicate label fetch would move a count.
    let task_list_path = format!("task-lists/{TASK_LIST_ID}");
    let project_calls_labelled = server.api_calls(&project_path);
    let task_list_calls_labelled = server.api_calls(&task_list_path);

    page.locator(format!("[data-tab-activate=\"{}\"]", projects[0].id))
        .click(None)
        .await
        .expect("the project chip should be clickable");
    wait_for_container(&page, "the project chip focused", |probe| {
        probe.wrappers == 2 && probe.active_view == "project"
    })
    .await;

    let task_list_card_again = page.locator(format!(
        "[data-tab-active=\"true\"] [data-task-list-card=\"{TASK_LIST_ID}\"]",
    ));
    expect(task_list_card_again.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the project dashboard should list the task list card again");
    task_list_card_again
        .click(None)
        .await
        .expect("the task list card should focus its open tab");
    wait_for_container(&page, "the task list card focused its own tab", |probe| {
        probe.wrappers == 2 && probe.active_view == "task-list"
    })
    .await;
    settle_frame();
    assert_eq!(
        server.api_calls(&project_path),
        project_calls_labelled,
        "re-activating a labelled tab issues no label GET"
    );
    assert_eq!(
        server.api_calls(&task_list_path),
        task_list_calls_labelled,
        "re-opening a surface whose label is known issues no label GET"
    );

    // Pin (activate): the strip's activate control focuses its own tab.
    let child_id = children[0].id.clone();
    page.locator(format!("[data-tab-activate=\"{}\"]", projects[0].id))
        .click(None)
        .await
        .expect("the project chip should be clickable");
    let back_on_project = wait_for_container(&page, "the project chip focused", |probe| {
        probe.wrappers == 2 && probe.active_view == "project"
    })
    .await;
    assert_eq!(back_on_project.active_id, projects[0].id);

    // Pin (close a background tab): the active tab's content is unchanged.
    page.locator(format!("[data-tab-close=\"{child_id}\"]"))
        .click(None)
        .await
        .expect("the background chip's × should be clickable");
    let after_background_close =
        wait_for_container(&page, "the background child tab closed", |probe| {
            probe.wrappers == 1
        })
        .await;
    assert_eq!(
        after_background_close.active_id, back_on_project.active_id,
        "closing a background tab leaves the active tab alone"
    );
    assert_eq!(after_background_close.active_view, "project");
    assert_eq!(after_background_close.active_display, "block");
    let strip = wait_for_strip(&page, "the child chip gone", |probe| {
        strip_chips(probe).len() == 1
    })
    .await;
    assert!(
        !has_strip_row(&strip, "children"),
        "an empty level-two row leaves the tree"
    );

    // Pin (close the active tab): the restored survivor must mount and paint. This is the first
    // build's failure: a close re-activated a removed id and the content area went blank.
    write_session(&page, &two_project_session()).await;
    let connections = socket.connections();
    page.goto(&origin, None)
        .await
        .expect("the SPA origin should restore the stored strip");
    let stored = wait_for_container(&page, "the two stored project tabs", |probe| {
        probe.wrappers == 2 && probe.active_entity == SECOND_PROJECT_ID
    })
    .await;
    socket.wait_for_connection_after(connections);
    assert_eq!(
        stored.active_id, "2",
        "tab ids follow stored order, so the active tab is the second"
    );
    assert_eq!(
        stored.hidden_children, 0,
        "the restored background tab mounts nothing at boot"
    );

    let strip = wait_for_strip(&page, "the stored labels", |probe| {
        probe.visible && strip_chips(probe).len() == 2
    })
    .await;
    let labels: Vec<&str> = strip_row(&strip, "projects")
        .iter()
        .map(|chip| chip.label.as_str())
        .collect();
    assert_eq!(
        labels,
        vec![PROJECT_TITLE, SECOND_PROJECT_TITLE],
        "the strip renders the labels the session restored"
    );
    assert!(
        strip_chips(&strip).iter().all(|chip| chip.separated),
        "every chip keeps its close control outside its activate control"
    );

    page.locator(format!("[data-tab-close=\"{}\"]", stored.active_id))
        .click(None)
        .await
        .expect("the active chip's × should be clickable");
    let survivor = wait_for_container(&page, "the survivor of an active close", |probe| {
        probe.wrappers == 1
    })
    .await;
    assert_eq!(survivor.active, 1, "exactly one tab takes focus");
    assert_eq!(survivor.active_entity, PROJECT_ID);
    assert_eq!(
        survivor.active_display, "block",
        "a close never leaves every tab hidden"
    );
    assert_eq!(
        survivor.active_view, "project",
        "the survivor the close activated is mounted, so the content area is not blank"
    );

    // Pin (middle click): it closes a tab too, and the last close lands on chrome home.
    let survivor_chip = page.locator(format!("[data-strip-tab=\"{}\"]", survivor.active_id));
    expect(survivor_chip.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the survivor's chip should be visible");
    survivor_chip
        .click(Some(
            ClickOptions::builder().button(MouseButton::Middle).build(),
        ))
        .await
        .expect("the chip should accept a middle click");
    let emptied = wait_for_container(&page, "the middle-clicked tab closed", |probe| {
        probe.wrappers == 0
    })
    .await;
    assert_eq!(emptied.active, 0, "the last close leaves no active tab");
    expect(page.locator("[data-page=\"projects\"]"))
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the last close returns to chrome home, never closes the window");
    assert_eq!(
        page_path(&page).await,
        "/",
        "the last close writes chrome home"
    );
    assert!(
        !strip_probe(&page).await.visible,
        "the empty strip hides again"
    );

    // Pin (ring): Ctrl+Tab walks the fixed ring and reaches a child of an inactive project, while
    // that child's chip is not rendered at all.
    let ring_project = page.locator(format!("[data-project-card=\"{PROJECT_ID}\"]"));
    expect(ring_project.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome Projects view should render the fixture card for the ring pins");
    ring_project
        .click(None)
        .await
        .expect("the project card should open its tab");
    wait_for_container(&page, "the project tab open for the ring pins", |probe| {
        probe.wrappers == 1 && probe.active_view == "project"
    })
    .await;

    let ring_task_list = page.locator(format!(
        "[data-tab-active=\"true\"] [data-task-list-card=\"{TASK_LIST_ID}\"]",
    ));
    expect(ring_task_list.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the project dashboard should list the fixture task list");
    ring_task_list
        .click(None)
        .await
        .expect("the task list card should open its child tab");
    wait_for_container(&page, "the child tab open for the ring pins", |probe| {
        probe.wrappers == 2 && probe.active_view == "task-list"
    })
    .await;

    // Chrome navigation opens and closes no tab, so the second project card is reachable while both
    // tabs stay open (decision 8).
    page.locator("nav a[href=\"/\"]")
        .first()
        .click(None)
        .await
        .expect("the Projects nav link should reach chrome home");
    let ring_second_selector = format!("[data-project-card=\"{SECOND_PROJECT_ID}\"]");
    let ring_second = page.locator(ring_second_selector);
    expect(ring_second.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome Projects view should render the second fixture card");
    ring_second
        .click(None)
        .await
        .expect("the second project card should open its own tab");
    wait_for_container(&page, "the second project tab open", |probe| {
        probe.wrappers == 3 && probe.active_entity == SECOND_PROJECT_ID
    })
    .await;

    let ring_child = tab_entry(&page, "task-list", TASK_LIST_ID).await;
    assert_eq!(
        ring_child.display, "none",
        "a child of an inactive project stays mounted and hidden"
    );
    assert!(!ring_child.active);

    let strip = wait_for_strip(&page, "the two project chips", |probe| {
        strip_chips(probe).len() == 2
    })
    .await;
    assert_eq!(
        strip_row(&strip, "projects").len(),
        2,
        "row one holds both project chips"
    );
    assert!(
        !has_strip_row(&strip, "children"),
        "the active project has no child, so level two renders nothing"
    );
    assert!(
        strip_chips(&strip)
            .iter()
            .all(|chip| chip.id != ring_child.id),
        "a child of an inactive project is not rendered in the strip"
    );

    assert!(
        press_chord(&page, "Tab", false, true, false).await,
        "the app claims Ctrl+Tab"
    );
    let on_ring_child = wait_for_container(
        &page,
        "the ring reached the other project's child",
        |probe| probe.wrappers == 3 && probe.active_view == "task-list",
    )
    .await;
    assert_eq!(
        on_ring_child.active_id, ring_child.id,
        "Ctrl+Tab advanced one step of the fixed ring"
    );
    assert_eq!(on_ring_child.active_entity, TASK_LIST_ID);
    assert_eq!(on_ring_child.active_display, "block");

    let strip = wait_for_strip(&page, "the active project's child row", |probe| {
        has_strip_row(probe, "children")
    })
    .await;
    let children = strip_row(&strip, "children");
    assert_eq!(
        children.len(),
        1,
        "row two holds the active project's child"
    );
    assert_eq!(children[0].id, ring_child.id);
    assert!(children[0].active);
    assert_eq!(
        strip_row(&strip, "projects").len(),
        2,
        "both project chips stay in row one"
    );
    assert!(
        strip_row(&strip, "projects")
            .iter()
            .all(|chip| chip.kind == "project")
            && children.iter().all(|chip| chip.kind == "task-list"),
        "row two never holds a project tab"
    );

    assert!(
        press_chord(&page, "Tab", false, true, true).await,
        "the app claims Ctrl+Shift+Tab"
    );
    let back_on_second = wait_for_container(&page, "the ring stepped back", |probe| {
        probe.wrappers == 3 && probe.active_entity == SECOND_PROJECT_ID
    })
    .await;
    assert_eq!(back_on_second.active_view, "project");
    let strip = wait_for_strip(&page, "the child row gone again", |probe| {
        !has_strip_row(probe, "children")
    })
    .await;
    assert_eq!(strip_chips(&strip).len(), 2);

    // Pin (Cmd+W): the active tab closes and a mounted survivor paints.
    assert!(
        press_chord(&page, "w", true, false, false).await,
        "the app claims Cmd+W"
    );
    let after_cmd_w = wait_for_container(&page, "Cmd+W closed the active tab", |probe| {
        probe.wrappers == 2
    })
    .await;
    assert_eq!(
        after_cmd_w.active_id, ring_child.id,
        "the ring's next survivor takes focus"
    );
    assert_eq!(after_cmd_w.active_view, "task-list");
    assert_eq!(after_cmd_w.active_display, "block");

    // Pin (Cmd+T): it reaches chrome Projects and opens no blank tab.
    let tabs_before_cmd_t = tabs_probe(&page).await.len();
    assert!(
        press_chord(&page, "t", true, false, false).await,
        "the app claims Cmd+T"
    );
    expect(page.locator("[data-page=\"projects\"]"))
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("Cmd+T reaches the Projects chrome view");
    assert_eq!(page_path(&page).await, "/", "Cmd+T writes chrome home");
    assert_eq!(
        tabs_probe(&page).await.len(),
        tabs_before_cmd_t,
        "Cmd+T opens no blank tab"
    );

    // Pin (the + control): it reaches the same chrome view, and a chip's activate control writes the
    // tab URL back through the one writer.
    let strip = strip_probe(&page).await;
    let project_chip = strip_row(&strip, "projects")
        .first()
        .expect("the project chip is still in row one")
        .id
        .clone();
    page.locator(format!("[data-tab-activate=\"{project_chip}\"]"))
        .click(None)
        .await
        .expect("the project chip should focus its tab");
    let focused_project = wait_for_container(&page, "the project chip focused", |probe| {
        probe.active_entity == PROJECT_ID && probe.active_view == "project"
    })
    .await;
    assert_eq!(
        focused_project.active_id, project_chip,
        "the strip's activate control focuses its own tab"
    );
    assert_eq!(
        page_path(&page).await,
        format!("/projects/{PROJECT_ID}"),
        "the one writer names the tab the strip focused"
    );

    // Pin (decision 17): Cmd+Shift+W stays the window's accelerator, so the SPA does not claim it.
    let tabs_before_shift = tabs_probe(&page).await.len();
    assert!(
        !press_chord(&page, "w", true, false, true).await,
        "Cmd+Shift+W belongs to the window, not to the tab shell"
    );
    settle_frame();
    let after_shift = tabs_probe(&page).await;
    assert_eq!(
        after_shift.len(),
        tabs_before_shift,
        "Cmd+Shift+W closes no tab in the page"
    );
    assert!(
        after_shift.iter().any(|tab| tab.active),
        "a tab is still active after Cmd+Shift+W"
    );

    // Pin (Cmd+,): the settings view is chrome, so the chord opens no tab and deactivates the ones
    // that are open. From a tab surface first: the one writer leaves the strip's count alone and the
    // deactivation hides every wrapper, so settings is the only surface.
    let tabs_before_cmd_comma = tabs_probe(&page).await;
    assert!(
        tabs_before_cmd_comma.iter().any(|tab| tab.active),
        "a tab is active before Cmd+,"
    );
    assert!(
        press_chord(&page, ",", true, false, false).await,
        "the app claims Cmd+,"
    );
    expect(page.locator("[data-page=\"settings\"]"))
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("Cmd+, reaches the settings view");
    assert_eq!(
        page_path(&page).await,
        "/settings",
        "Cmd+, writes the settings URL"
    );
    let after_cmd_comma = wait_for_container(&page, "no active tab on settings", |probe| {
        probe.active == 0
    })
    .await;
    assert_eq!(
        after_cmd_comma.wrappers,
        tabs_before_cmd_comma.len(),
        "Cmd+, opens no tab and closes no tab from a tab surface"
    );
    assert!(
        tabs_probe(&page)
            .await
            .iter()
            .all(|tab| tab.display == "none"),
        "settings is chrome, so no tab wrapper may paint under it"
    );

    // Pin (Cmd+, from chrome): it names settings from the same start point Cmd+T names home from.
    assert!(
        press_chord(&page, "t", true, false, false).await,
        "the app claims Cmd+T"
    );
    expect(page.locator("[data-page=\"projects\"]"))
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("Cmd+T returns to the Projects chrome view");
    assert_eq!(page_path(&page).await, "/", "Cmd+T writes chrome home");

    let tabs_before_chrome_comma = tabs_probe(&page).await.len();
    assert!(
        press_chord(&page, ",", true, false, false).await,
        "the app claims Cmd+, from chrome"
    );
    expect(page.locator("[data-page=\"settings\"]"))
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("Cmd+, reaches the settings view from chrome");
    assert_eq!(
        page_path(&page).await,
        "/settings",
        "Cmd+, writes the settings URL from chrome"
    );
    assert_eq!(
        tabs_probe(&page).await.len(),
        tabs_before_chrome_comma,
        "Cmd+, opens no tab from chrome"
    );

    // Pin (key hold): a repeat is dropped before any chord decides, so it stays unclaimed.
    assert!(
        !press_chord_repeat(&page, ",", true, false, false, true).await,
        "a repeated Cmd+, stays unclaimed"
    );
    settle_frame();
    assert_eq!(
        page_path(&page).await,
        "/settings",
        "a repeated Cmd+, navigates nowhere"
    );

    // Pin (Cmd+Shift+,): the shifted chord belongs to the window, so the SPA does not claim it.
    assert!(
        !press_chord(&page, ",", true, false, true).await,
        "Cmd+Shift+, stays unclaimed"
    );
    settle_frame();
    assert_eq!(
        page_path(&page).await,
        "/settings",
        "Cmd+Shift+, navigates nowhere"
    );

    // Pin (Ctrl+,): only plain Cmd chords reach the command arm, so Ctrl+, stays unclaimed.
    assert!(
        !press_chord(&page, ",", false, true, false).await,
        "Ctrl+, stays unclaimed"
    );
    settle_frame();
    assert_eq!(
        page_path(&page).await,
        "/settings",
        "Ctrl+, navigates nowhere"
    );

    // === Chrome navigation deactivates the strip (decisions 5 and 6) ===
    //
    // Exactly one surface renders: chrome XOR one tab surface. Selecting a chrome view deactivates
    // the tab set without closing it, so no wrapper paints under that view. The projects overview
    // shows the opened project tabs in row one only, with no active chip and no second row.

    let connections = socket.connections();
    clear_session(&page).await;
    page.goto(&origin, None)
        .await
        .expect("the SPA origin should load for the chrome navigation pins");
    expect(page.locator("[data-page=\"projects\"]"))
        .with_timeout(BOOT_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the Projects page should render before the chrome navigation pins");
    socket.wait_for_connection_after(connections);

    let chrome_project = page.locator(format!("[data-project-card=\"{PROJECT_ID}\"]"));
    expect(chrome_project.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the chrome Projects view should render the fixture card");
    chrome_project
        .click(None)
        .await
        .expect("the project card should open its tab");
    let chrome_tab =
        wait_for_container(&page, "the project tab open for the chrome pins", |probe| {
            probe.wrappers == 1 && probe.active_view == "project"
        })
        .await;

    let chrome_task_list = page.locator(format!(
        "[data-tab-active=\"true\"] [data-task-list-card=\"{TASK_LIST_ID}\"]",
    ));
    expect(chrome_task_list.clone())
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the project dashboard should list the fixture task list");
    chrome_task_list
        .click(None)
        .await
        .expect("the task list card should open its child tab");
    wait_for_container(&page, "the task list child tab open", |probe| {
        probe.wrappers == 2 && probe.active_view == "task-list"
    })
    .await;

    // Pin (a chrome-global view deactivates): every chrome list hides the strip, paints no tab, and
    // keeps the tab set open. Its own view is the only surface in the routed area.
    for href in ["/notes", "/repos", "/skills", "/settings"] {
        page.locator(format!("nav a[href=\"{href}\"]"))
            .first()
            .click(None)
            .await
            .unwrap_or_else(|error| panic!("the {href} nav link should be clickable: {error}"));
        assert_eq!(
            page_path(&page).await,
            href,
            "the {href} nav link should reach {href}"
        );

        let hidden = wait_for_strip(&page, "the strip hidden on a chrome list", |probe| {
            !probe.visible
        })
        .await;
        assert!(!hidden.visible, "{href} hides the strip: {hidden:?}");

        let chrome = wait_for_container(&page, "no active tab on a chrome list", |probe| {
            probe.active == 0
        })
        .await;
        assert_eq!(
            chrome.wrappers, 2,
            "{href} keeps every open tab: {chrome:?}"
        );
        assert_eq!(chrome.active, 0, "{href} activates no tab: {chrome:?}");

        let entries = tabs_probe(&page).await;
        assert!(
            entries.iter().all(|tab| tab.display == "none"),
            "{href} must paint no tab: {entries:?}"
        );

        let routes: RoutesProbe = page
            .evaluate(ROUTES_PROBE, None::<&()>)
            .await
            .expect("the page should report the routed area");
        assert!(
            routes.present && routes.children > 0,
            "{href} renders its own view in the routed area: {routes:?}"
        );

        if href == "/notes" {
            expect(page.locator(format!("[data-routes-area] [data-note-card=\"{NOTE_ID}\"]",)))
                .with_timeout(UI_TIMEOUT)
                .to_be_visible()
                .await
                .expect("the notes overview renders while every tab hides");
        }
    }

    // Pin (the projects overview): row one only. The opened project tabs render with no active chip,
    // no second row, and no parentless row, and the overview is the only surface.
    page.locator("nav a[href=\"/\"]")
        .first()
        .click(None)
        .await
        .expect("the Projects nav link should return to the overview");
    assert_eq!(
        page_path(&page).await,
        "/",
        "the Projects nav link writes the overview URL"
    );

    let home = wait_for_strip(&page, "the projects row on the overview", |probe| {
        probe.visible && has_strip_row(probe, "projects")
    })
    .await;
    assert_eq!(
        strip_row(&home, "projects").len(),
        1,
        "row one holds the opened project tab: {home:?}"
    );
    assert!(
        strip_row(&home, "projects")[0].id == chrome_tab.active_id,
        "row one names the tab the card opened: {home:?}"
    );
    assert!(
        !has_strip_row(&home, "children"),
        "the overview renders no second row: {home:?}"
    );
    assert!(
        !has_strip_row(&home, "parentless"),
        "the overview renders no parentless row: {home:?}"
    );
    assert!(
        strip_chips(&home).iter().all(|chip| !chip.active),
        "no chip is active on the overview: {home:?}"
    );

    let overview = container_probe(&page).await;
    assert_eq!(
        overview.wrappers, 2,
        "the overview keeps both open tabs: {overview:?}"
    );
    assert_eq!(
        overview.active, 0,
        "the overview activates no tab: {overview:?}"
    );
    expect(page.locator("[data-page=\"projects\"]"))
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the overview is the only surface on the projects home");

    // Pin (a project chip returns): selecting the project tab from the overview paints its overview
    // and brings its task-list row back.
    let project_chip = strip_row(&home, "projects")[0].id.clone();
    page.locator(format!("[data-tab-activate=\"{project_chip}\"]"))
        .click(None)
        .await
        .expect("the project chip should focus its tab");
    let back_on_project = wait_for_container(
        &page,
        "the project tab painted from the overview",
        |probe| probe.active_view == "project" && probe.active_entity == PROJECT_ID,
    )
    .await;
    assert_eq!(
        back_on_project.active, 1,
        "exactly one surface paints: {back_on_project:?}"
    );
    assert_eq!(
        page_path(&page).await,
        format!("/projects/{PROJECT_ID}"),
        "the one writer names the tab the chip focused"
    );

    let with_children = wait_for_strip(&page, "the project's task-list row", |probe| {
        has_strip_row(probe, "children")
    })
    .await;
    assert_eq!(
        strip_row(&with_children, "children").len(),
        1,
        "row two holds the project's task list: {with_children:?}"
    );
    assert!(
        strip_row(&with_children, "projects")[0].active,
        "the project chip is the active chip again: {with_children:?}"
    );

    // Pin (a task-list chip shows the kanban): selecting the child chip paints that tab alone.
    let child_chip = strip_row(&with_children, "children")[0].id.clone();
    page.locator(format!("[data-tab-activate=\"{child_chip}\"]"))
        .click(None)
        .await
        .expect("the task-list chip should focus its tab");
    let kanban = wait_for_container(&page, "the task-list tab painted", |probe| {
        probe.active_view == "task-list" && probe.active_entity == TASK_LIST_ID
    })
    .await;
    assert_eq!(kanban.active, 1, "exactly one surface paints: {kanban:?}");
    assert_eq!(
        page_path(&page).await,
        format!("/projects/{PROJECT_ID}/task-lists/{TASK_LIST_ID}"),
        "the one writer names the kanban surface"
    );
    let routes_under_tab: RoutesProbe = page
        .evaluate(ROUTES_PROBE, None::<&()>)
        .await
        .expect("the page should report the routed area");
    assert_eq!(
        routes_under_tab.children, 0,
        "a tab surface renders nothing in the routed area: {routes_under_tab:?}"
    );

    // === Update pill (one surface, app level, bottom right) ===

    // Pin (idle): nothing is announced, so nothing shows. The mount fetch has long landed.
    let idle = pill_probe(&page).await;
    assert!(
        !idle.present,
        "an idle update state shows no pill, got {idle:?}"
    );

    // Pin (available): the candidate version, the Update call to action, and a dismiss control.
    server.set_update_status("available", Some(UPDATE_VERSION), None);
    socket.send(&update_state_frame("available", UPDATE_VERSION));
    let available = wait_for_pill(&page, "available", |probe| {
        probe.present && probe.phase == "available"
    })
    .await;
    assert_eq!(
        available.text,
        format!("Update {UPDATE_VERSION} available"),
        "the available pill names the candidate version"
    );
    assert_eq!(
        available.update_cta, 1,
        "the available pill offers Update: {available:?}"
    );
    assert_eq!(
        available.dismiss_cta, 1,
        "the available pill can be dismissed: {available:?}"
    );
    assert_eq!(
        available.restart_cta, 0,
        "no restart is offered before an install: {available:?}"
    );

    // Pin (dismissal, per version): the dismissal names one candidate, so re-announcing the same
    // version keeps the pill hidden and a newer one brings it back.
    page.locator("[data-update-pill] [data-update-dismiss]")
        .click(None)
        .await
        .expect("the dismiss control should be clickable");
    let dismissed = wait_for_pill(&page, "dismissed", |probe| !probe.present).await;
    assert!(
        !dismissed.present,
        "a dismissal hides the pill: {dismissed:?}"
    );

    let status_calls = server.api_calls("update/status");
    socket.send(&update_state_frame("available", UPDATE_VERSION));
    wait_for_api_calls(&server, "update/status", status_calls + 1);
    settle_frame();
    let still_dismissed = pill_probe(&page).await;
    assert!(
        !still_dismissed.present,
        "a same-version re-announcement keeps the dismissed pill hidden: {still_dismissed:?}"
    );

    // Pin (dismissal survives an install): `downloading` arrives without the candidate fields, and
    // the dismissed candidate stays hidden through it.
    server.set_update_status("downloading", None, None);
    socket.send(&update_phase_frame("downloading"));
    let install_calls = server.api_calls("update/status");
    wait_for_api_calls(&server, "update/status", install_calls + 1);
    settle_frame();
    let during_install = pill_probe(&page).await;
    assert!(
        !during_install.present,
        "a dismissed candidate stays hidden through the install: {during_install:?}"
    );

    // Pin (stacking): the pill floats above the app chrome but below an open inline surface, which
    // covers it completely, so a dialog never sits under an update notice.
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
        .expect("the chrome Projects view should render its cards");
    host_card
        .click(None)
        .await
        .expect("the project card should focus its open tab");
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
    wait_for_open_surface(&page, "task-detail").await;

    // Pin (dismissal, newer candidate): a version newer than the dismissed one brings the pill back,
    // even with a surface open, and the surface still covers it.
    server.set_update_status("available", Some(UPDATE_NEWER_VERSION), None);
    socket.send(&update_state_frame("available", UPDATE_NEWER_VERSION));
    let newer = wait_for_pill(&page, "a newer candidate", |probe| {
        probe.present && probe.text.contains(UPDATE_NEWER_VERSION)
    })
    .await;
    assert_eq!(
        newer.text,
        format!("Update {UPDATE_NEWER_VERSION} available"),
        "a newer candidate brings the pill back"
    );

    let layered = pill_probe(&page).await;
    assert!(
        layered.present,
        "the pill stays mounted under an open surface: {layered:?}"
    );
    assert_eq!(
        layered.position, "fixed",
        "the pill is fixed to the viewport: {layered:?}"
    );
    assert_eq!(
        layered.pill_z,
        Some(40),
        "the pill paints at z-index 40: {layered:?}"
    );
    assert!(
        layered.nav_z.unwrap_or_default() < 40,
        "the pill sits above the app chrome: {layered:?}"
    );
    assert_eq!(
        layered.surface_z,
        Some(50),
        "the open inline surface paints at z-index 50: {layered:?}"
    );
    assert!(
        !layered.hit_in_pill,
        "an open surface covers the pill: {layered:?}"
    );
    assert!(
        layered.hit_in_surface,
        "the pointer at the pill's centre lands in the open surface: {layered:?}"
    );
    assert!(
        layered.in_right_half && layered.in_bottom_half,
        "the pill hugs the bottom right corner: {layered:?}"
    );
    assert!(
        (0..=32).contains(&layered.right_gap) && (0..=32).contains(&layered.bottom_gap),
        "the pill keeps a small gap to both viewport edges: {layered:?}"
    );

    page.keyboard()
        .press("Escape", None)
        .await
        .expect("the page should accept the Escape key");
    wait_for_closed_surface(&page).await;

    // Pin (phases): a spinner and the phase sentence, with no percentage and no progress bar. The
    // phase carries no candidate version, and the pill still shows it: the newer candidate was never
    // dismissed.
    server.set_update_status("downloading", None, None);
    socket.send(&update_phase_frame("downloading"));
    let downloading = wait_for_pill(&page, "downloading", |probe| {
        probe.present && probe.phase == "downloading"
    })
    .await;
    assert_eq!(downloading.text, "Downloading update…");
    assert_eq!(
        downloading.spinners, 1,
        "a running phase shows the spinner: {downloading:?}"
    );
    assert_eq!(
        downloading.thaw_spinners, 1,
        "the phase reuses the thaw Spinner: {downloading:?}"
    );
    assert!(
        !downloading.percent,
        "a phase shows no percentage: {downloading:?}"
    );
    assert_eq!(
        downloading.progress_bars, 0,
        "a phase shows no progress bar: {downloading:?}"
    );
    assert_eq!(
        downloading.dismiss_cta, 0,
        "a running phase carries no dismiss control: {downloading:?}"
    );

    // Pin (ready): the restart sentence and the Restart call to action.
    server.set_update_status("ready", Some(UPDATE_NEWER_VERSION), None);
    socket.send(&update_state_frame("ready", UPDATE_NEWER_VERSION));
    let ready = wait_for_pill(&page, "ready", |probe| {
        probe.present && probe.phase == "ready"
    })
    .await;
    assert_eq!(ready.text, "Update installed — restart to apply");
    assert_eq!(
        ready.restart_cta, 1,
        "the ready pill offers Restart: {ready:?}"
    );
    assert_eq!(
        ready.dismiss_cta, 1,
        "the ready pill can be dismissed: {ready:?}"
    );

    // Pin (one restart path): the CTA posts to /update/restart and nothing else. A 202 leaves the
    // staged state in place, which is what a restarting app would report.
    server.set_restart_status(202);
    let restart_calls = server.api_calls("update/restart");
    page.locator("[data-update-pill] [data-update-restart]")
        .click(None)
        .await
        .expect("the Restart control should be clickable");
    wait_for_api_calls(&server, "update/restart", restart_calls + 1);
    settle_frame();
    let restarted = pill_probe(&page).await;
    assert_eq!(
        restarted.phase, "ready",
        "a 202 keeps the restart state: {restarted:?}"
    );

    // Pin (refused restart): a 409 ends at the failed pill state, not a spinner that never ends.
    server.set_restart_status(409);
    page.locator("[data-update-pill] [data-update-restart]")
        .click(None)
        .await
        .expect("the Restart control should be clickable a second time");
    let refused = wait_for_pill(&page, "a refused restart", |probe| {
        probe.present && probe.phase == "failed"
    })
    .await;
    assert!(
        refused.text.contains("restart failed"),
        "the failed pill reports the refused restart: {refused:?}"
    );
    assert_eq!(
        refused.spinners, 0,
        "a refused restart ends the spinner: {refused:?}"
    );
    assert_eq!(
        refused.dismiss_cta, 1,
        "the failed pill is dismissible: {refused:?}"
    );

    // Pin (no flow): a 503 takes the same failed-pill path.
    server.set_update_status("ready", Some(UPDATE_NEWER_VERSION), None);
    socket.send(&update_state_frame("ready", UPDATE_NEWER_VERSION));
    wait_for_pill(&page, "ready again", |probe| {
        probe.present && probe.phase == "ready"
    })
    .await;
    server.set_restart_status(503);
    page.locator("[data-update-pill] [data-update-restart]")
        .click(None)
        .await
        .expect("the Restart control should be clickable after the state moved on");
    let unavailable = wait_for_pill(&page, "a restart with no flow", |probe| {
        probe.present && probe.phase == "failed"
    })
    .await;
    assert!(
        unavailable.text.contains("The restart failed"),
        "a 503 ends at the failed pill state: {unavailable:?}"
    );
    assert_eq!(
        unavailable.spinners, 0,
        "a 503 leaves no spinner behind: {unavailable:?}"
    );

    // Pin (failure surface): the service's own failure message renders in the pill, with a dismiss
    // control and no action, so no second error surface exists.
    server.set_update_status("failed", None, Some("GitHub refused the manifest"));
    socket.send(&update_phase_frame("failed"));
    let failed = wait_for_pill(&page, "a failed update", |probe| {
        probe.present
            && probe.phase == "failed"
            && probe.text.contains("GitHub refused the manifest")
    })
    .await;
    assert_eq!(
        failed.dismiss_cta, 1,
        "the failed pill is dismissible: {failed:?}"
    );
    assert_eq!(failed.spinners, 0, "a failure shows no spinner: {failed:?}");
    assert_eq!(
        failed.update_cta + failed.restart_cta,
        0,
        "a failure offers no action: {failed:?}"
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

/// Waits until the server has answered `count` requests for one API path.
///
/// A "no fetch" pin must not race the app: the label resolve is asynchronous, so the count is polled
/// before the exact value is asserted.
fn wait_for_api_calls(server: &StaticServer, path: &str, count: usize) {
    let deadline = Instant::now() + UI_TIMEOUT;
    while server.api_calls(path) < count {
        assert!(
            Instant::now() < deadline,
            "the API never answered {count} request(s) for {path}"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

/// Clears the stored session, so the next boot is chrome and not a restore (decision 14).
async fn clear_session(page: &Page) {
    let _: bool = page
        .evaluate(CLEAR_SESSION_PROBE, Some(&SESSION_KEY))
        .await
        .expect("the page should clear the stored session");
}

/// Writes a session string, so the next boot restores what it holds (decision 14).
async fn write_session(page: &Page, raw: &str) {
    let _: bool = page
        .evaluate(WRITE_SESSION_PROBE, Some(&(SESSION_KEY, raw)))
        .await
        .expect("the page should write the stored session");
}

/// A session with one project the stub serves and one it does not, the missing surface active.
fn stale_session() -> String {
    format!(
        "{{\"version\":1,\"tabs\":[{{\"key\":{{\"kind\":\"project\",\"id\":\"{PROJECT_ID}\"}},\"title\":\"stale label\",\"parent\":null}},{{\"key\":{{\"kind\":\"project\",\"id\":\"{BOGUS_ID}\"}},\"title\":\"gone label\",\"parent\":null}}],\"active\":{{\"kind\":\"project\",\"id\":\"{BOGUS_ID}\"}}}}"
    )
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
    assert!(
        clicked,
        "{selector} should be reachable through its handler"
    );
}

/// Reads the strip in one round trip.
async fn strip_probe(page: &Page) -> StripProbe {
    page.evaluate(STRIP_PROBE, None::<&()>)
        .await
        .expect("the page should report the strip state")
}

/// Polls the strip until `predicate` holds.
async fn wait_for_strip(
    page: &Page,
    what: &str,
    predicate: impl Fn(&StripProbe) -> bool,
) -> StripProbe {
    let deadline = Instant::now() + UI_TIMEOUT;
    loop {
        let probe = strip_probe(page).await;
        if predicate(&probe) {
            return probe;
        }
        assert!(
            Instant::now() < deadline,
            "the strip never showed {what}: {probe:?}"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

/// The chips of one row, or an empty slice when the row is not rendered.
fn strip_row<'a>(probe: &'a StripProbe, name: &str) -> &'a [StripChipProbe] {
    probe
        .rows
        .iter()
        .find(|row| row.name == name)
        .map(|row| row.chips.as_slice())
        .unwrap_or_default()
}

/// Whether the strip renders this row at all.
fn has_strip_row(probe: &StripProbe, name: &str) -> bool {
    probe.rows.iter().any(|row| row.name == name)
}

/// Every chip of the strip, whatever row renders it.
fn strip_chips(probe: &StripProbe) -> Vec<&StripChipProbe> {
    probe.rows.iter().flat_map(|row| &row.chips).collect()
}

/// Reads every tab wrapper in one round trip.
async fn tabs_probe(page: &Page) -> Vec<TabEntry> {
    page.evaluate(TABS_PROBE, None::<&()>)
        .await
        .expect("the page should report the tab wrappers")
}

/// The wrapper of the surface with this view and entity id.
async fn tab_entry(page: &Page, view: &str, entity: &str) -> TabEntry {
    let deadline = Instant::now() + UI_TIMEOUT;
    loop {
        if let Some(entry) = tabs_probe(page)
            .await
            .into_iter()
            .find(|entry| entry.view == view && entry.entity == entity)
        {
            return entry;
        }
        assert!(
            Instant::now() < deadline,
            "no wrapper renders the {view} surface {entity}"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

/// The path the address bar holds.
async fn page_path(page: &Page) -> String {
    page.evaluate(PATH_PROBE, None::<&()>)
        .await
        .expect("the page should report its path")
}

/// Dispatches one synthetic `keydown` on the window and reports whether the app claimed it.
async fn press_chord(page: &Page, key: &str, meta: bool, ctrl: bool, shift: bool) -> bool {
    press_chord_repeat(page, key, meta, ctrl, shift, false).await
}

/// Dispatches one synthetic `keydown` that reports the key-hold state the webview sets.
async fn press_chord_repeat(
    page: &Page,
    key: &str,
    meta: bool,
    ctrl: bool,
    shift: bool,
    repeat: bool,
) -> bool {
    page.evaluate(KEYDOWN_PROBE, Some(&(key, meta, ctrl, shift, repeat)))
        .await
        .expect("the page should accept a synthetic keydown")
}

/// Reads the update pill in one round trip.
async fn pill_probe(page: &Page) -> PillProbe {
    page.evaluate(PILL_PROBE, None::<&()>)
        .await
        .expect("the page should report the update pill")
}

/// Polls the pill until `predicate` holds, so a websocket frame and the status fetch it triggers
/// have time to reach the app.
async fn wait_for_pill(
    page: &Page,
    what: &str,
    predicate: impl Fn(&PillProbe) -> bool,
) -> PillProbe {
    let deadline = Instant::now() + UI_TIMEOUT;
    loop {
        let probe = pill_probe(page).await;
        if predicate(&probe) {
            return probe;
        }
        assert!(
            Instant::now() < deadline,
            "the update pill never reached {what}: {probe:?}"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

/// A session holding both stub projects, the second one active.
///
/// The first project therefore restores unmounted, which is the state a close has to activate without
/// blanking the content area.
fn two_project_session() -> String {
    format!(
        "{{\"version\":1,\"tabs\":[{{\"key\":{{\"kind\":\"project\",\"id\":\"{PROJECT_ID}\"}},\"title\":\"{PROJECT_TITLE}\",\"parent\":null}},{{\"key\":{{\"kind\":\"project\",\"id\":\"{SECOND_PROJECT_ID}\"}},\"title\":\"{SECOND_PROJECT_TITLE}\",\"parent\":null}}],\"active\":{{\"kind\":\"project\",\"id\":\"{SECOND_PROJECT_ID}\"}}}}"
    )
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
    /// Every request path the server answered, in arrival order: the boot-fetch witness.
    requests: Arc<Mutex<Vec<String>>>,
    /// The mutable update stub: the pill re-reads `/update/status` after every announcement.
    update: Arc<UpdateStub>,
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
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&requests);
        let update = Arc::new(UpdateStub::new());
        let stub = Arc::clone(&update);
        let thread = thread::spawn(move || accept_loop(listener, &dist, &log, &stub, &stop));

        Self {
            addr,
            requests,
            update,
            shutdown,
            thread: Some(thread),
        }
    }

    /// Re-stub the state `GET /update/status` reports.
    fn set_update_status(&self, status: &str, candidate: Option<&str>, message: Option<&str>) {
        self.update.set_status(status, candidate, message);
    }

    /// Re-stub the answer `POST /update/restart` gives.
    fn set_restart_status(&self, status: u16) {
        self.update.set_restart_status(status);
    }

    fn origin(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// How many requests this server answered for one exact path, since the run started.
    ///
    /// The stub answers API paths whose query string was already stripped, so an exact comparison
    /// counts one request and no lookalike beside it.
    fn calls(&self, path: &str) -> usize {
        self.requests
            .lock()
            .map(|log| log.iter().filter(|request| *request == path).count())
            .unwrap_or_default()
    }

    /// How many requests this server answered for one API path.
    fn api_calls(&self, path: &str) -> usize {
        self.calls(&format!("{API_PREFIX}{path}"))
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

fn accept_loop(
    listener: TcpListener,
    dist: &Path,
    requests: &Mutex<Vec<String>>,
    update: &UpdateStub,
    stop: &AtomicBool,
) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => serve_connection(stream, dist, requests, update),
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

fn serve_connection(
    mut stream: TcpStream,
    dist: &Path,
    requests: &Mutex<Vec<String>>,
    update: &UpdateStub,
) {
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
    if let Ok(mut log) = requests.lock() {
        log.push(path.to_string());
    }

    let response = route(path, dist, update);
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

/// The mutable part of the update stub.
///
/// The pill re-reads `/update/status` after every announcement, so a test re-stubs this state and
/// sends an `UpdateStateChanged` frame to drive one phase.
struct UpdateStub {
    status: Mutex<String>,
    restart: AtomicU16,
}

impl UpdateStub {
    fn new() -> Self {
        Self {
            status: Mutex::new(update_status_body("idle", None, None)),
            restart: AtomicU16::new(202),
        }
    }

    fn set_status(&self, status: &str, candidate: Option<&str>, message: Option<&str>) {
        if let Ok(mut body) = self.status.lock() {
            *body = update_status_body(status, candidate, message);
        }
    }

    fn status_body(&self) -> String {
        self.status
            .lock()
            .map(|body| body.clone())
            .unwrap_or_default()
    }

    fn set_restart_status(&self, status: u16) {
        self.restart.store(status, Ordering::Relaxed);
    }

    fn restart_status(&self) -> u16 {
        self.restart.load(Ordering::Relaxed)
    }
}

/// The `/update/status` body for one phase, in the shape `UpdateStatusPayload` deserializes.
fn update_status_body(status: &str, candidate: Option<&str>, message: Option<&str>) -> String {
    let candidate = candidate
        .map(|version| format!(",\"candidate_version\":\"{version}\""))
        .unwrap_or_default();
    let message = message
        .map(|message| format!(",\"message\":\"{message}\""))
        .unwrap_or_default();

    format!(
        "{{\"current_version\":\"{}\",\"status\":\"{status}\"{candidate}{message}}}",
        env!("CARGO_PKG_VERSION")
    )
}

/// One `/ws` announcement of an update state change, in the shape `UpdateMessage` deserializes.
fn update_state_frame(status: &str, version: &str) -> String {
    format!(
        "{{\"type\":\"UpdateStateChanged\",\"data\":{{\"status\":\"{status}\",\"version\":\"{version}\"}}}}"
    )
}

/// One announcement for a phase that names no candidate version, which is how an install's
/// `downloading` and `installing` frames arrive.
fn update_phase_frame(status: &str) -> String {
    format!(
        "{{\"type\":\"UpdateStateChanged\",\"data\":{{\"status\":\"{status}\",\"version\":null}}}}"
    )
}

fn route(path: &str, dist: &Path, update: &UpdateStub) -> Response {
    if let Some(api_path) = path.strip_prefix(API_PREFIX) {
        return api_response(api_path, update);
    }

    // The SPA dials `ws://localhost:8080/dev/ws`, which the harness serves from its own listener
    // (`WebSocketStub`). This origin answers the path with a 404, so a bundle that ever dialed a
    // relative path fails loudly here instead of reading HTML as a frame.
    if path.starts_with("/dev/ws") {
        return Response::text(404, "e2e stub: the websocket lives on 127.0.0.1:8080");
    }

    static_response(dist, path)
}

fn api_response(path: &str, update: &UpdateStub) -> Response {
    match path {
        "projects" => Response::json(200, &project_page()),
        "task-lists" => Response::json(200, &task_list_page()),
        "notes" => Response::json(200, &note_page()),
        "repos" => Response::json(200, &empty_page()),
        "skills" => Response::json(200, &skill_page()),
        "update/status" => Response::json(200, &update.status_body()),
        "update/check" => Response::json(200, "{}"),
        "update/restart" => {
            let status = update.restart_status();
            if (200..300).contains(&status) {
                Response::json(status, "{}")
            } else {
                Response::json(
                    status,
                    "{\"error\":\"e2e stub: the restart was refused\",\"details\":null}",
                )
            }
        }
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
        NOTE_PARENT_PROJECT_ID => Some(project_record(
            NOTE_PARENT_PROJECT_ID,
            NOTE_PARENT_PROJECT_TITLE,
        )),
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
        "{{\"id\":\"{TASK_LIST_ID}\",\"project_id\":\"{PROJECT_ID}\",\"title\":\"{TASK_LIST_TITLE}\",\"description\":\"Fixture served by the e2e harness.\",\"status\":\"active\",\"external_refs\":[],\"notes\":null,\"tags\":[\"e2e\"],\"repo_ids\":[],\"created_at\":\"2026-01-01 00:00:00\",\"updated_at\":\"2026-01-01 00:00:00\"}}"
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
        202 => "Accepted",
        404 => "Not Found",
        409 => "Conflict",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

// endregion: --- Support
