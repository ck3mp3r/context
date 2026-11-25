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
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use playwright_rs::{Playwright, expect};
use serde::Deserialize;

/// Project the stubbed API serves, so a real card exists to click.
const PROJECT_ID: &str = "e2e00001";
const PROJECT_TITLE: &str = "E2E Fixture Project";

/// A second project, so a second card can open a second tab.
const SECOND_PROJECT_ID: &str = "e2e00002";
const SECOND_PROJECT_TITLE: &str = "E2E Second Project";

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
        in_container: wrappers.every((wrapper) =>
            wrapper.parentElement !== null && wrapper.parentElement.hasAttribute('data-tab-container')),
    };
}"#;

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
    in_container: bool,
}

#[tokio::test(flavor = "multi_thread")]
async fn spa_boots_and_project_card_navigation_reaches_project_detail() {
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

    // The tab strip renders only after the project payload arrives, so this
    // separates "routed to the detail page" from "detail page has its data".
    expect(page.locator(format!("a[href=\"{expected_path}/notes\"]")))
        .with_timeout(UI_TIMEOUT)
        .to_be_visible()
        .await
        .expect("the project detail tabs should render once the project loads");

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

    browser.close().await.expect("the browser should close");
    drop(server);
}

// endregion: --- Test

// region:    --- Support

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

fn route(path: &str, dist: &Path) -> Response {
    if let Some(api_path) = path.strip_prefix(API_PREFIX) {
        return api_response(api_path);
    }

    // The SPA dials `/dev/ws`; the stubbed API has no websocket, so the
    // connection is refused outright instead of answered with HTML.
    if path.starts_with("/dev/ws") {
        return Response::text(404, "e2e stub: no websocket");
    }

    static_response(dist, path)
}

fn api_response(path: &str) -> Response {
    match path {
        "projects" => Response::json(200, &project_page()),
        "task-lists" | "notes" | "repos" | "skills" => Response::json(200, &empty_page()),
        "update/status" => Response::json(
            200,
            &format!(
                "{{\"current_version\":\"{}\",\"status\":\"up_to_date\"}}",
                env!("CARGO_PKG_VERSION")
            ),
        ),
        "update/check" => Response::json(200, "{}"),
        _ => match project_by_path(path) {
            Some(body) => Response::json(200, &body),
            None => Response::json(
                404,
                "{\"error\":\"e2e stub: endpoint not served\",\"details\":null}",
            ),
        },
    }
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
