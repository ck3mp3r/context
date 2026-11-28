//! Server machinery shared by the app binaries.
//!
//! The port decision lives here: [`api_port`] reads the stored settings of the
//! build profile, so the attach probe, the in-process server, the SPA origin and
//! the tray MCP row all name the same port. [`start_server`] then either attaches
//! to a server that already answers `/health` or starts one and waits for it.

use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

use context_core::settings::{self, Settings, StoredSettings};
use context_core::update::UpdateFlow;
use context_core::{Database, get_db_path, try_get_config_path};
use context_db::SqliteDatabase;
use context_server::api::notifier::ChangeNotifier;
use context_server::api::{self, Config};

#[cfg(test)]
#[path = "../test/server.rs"]
mod server_test;

/// Dev server port. `trunk serve` proxies `/dev` and `/dev/ws` here.
const DEBUG_API_PORT: u16 = 3738;
/// Production port. MCP clients are configured for `localhost:3737/mcp`.
const RELEASE_API_PORT: u16 = 3737;
/// Debug SPA origin, served by `trunk serve` on 8080.
const DEBUG_SPA_ORIGIN: &str = "http://127.0.0.1:8080/";

/// How long to wait for the server to answer `/health` before giving up.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(10);
/// Delay between health probes while waiting for the server.
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(200);
/// Timeout of a single health probe.
const HEALTH_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);

/// Port the app talks to: the stored port, else the build profile's port.
///
/// This is the single port decision point: the attach probe, the in-process
/// server, the SPA origin and the tray MCP row all follow it, so a stored port
/// moves them together.
pub fn api_port() -> u16 {
    stored_settings().port.unwrap_or_else(profile_api_port)
}

/// Port of the build profile, used when the settings file carries none.
///
/// Uses `cfg!(debug_assertions)` — the same predicate the data directory uses —
/// so the app attaches to a server of its own profile and opens its own
/// database. A debug build never touches the production port.
fn profile_api_port() -> u16 {
    if cfg!(debug_assertions) {
        DEBUG_API_PORT
    } else {
        RELEASE_API_PORT
    }
}

/// Stored settings for this run: the file when it is readable and valid.
///
/// A file that cannot be read or parsed, or that carries values outside the
/// accepted range, is reported and ignored, so the app still starts. An
/// environment that names no config root yields the same defaults.
fn stored_settings() -> StoredSettings {
    let Some(path) = try_get_config_path() else {
        return StoredSettings::default();
    };

    let stored = match settings::load(&path) {
        Ok(stored) => stored,
        Err(error) => {
            eprintln!("context: ignoring settings file: {error}");
            return StoredSettings::default();
        }
    };

    if let Err(error) = settings::validate_stored(&stored) {
        eprintln!(
            "context: ignoring settings file {}: {error}",
            path.display()
        );
        return StoredSettings::default();
    }

    stored
}

/// Settings the app works with: stored values over the built-in defaults.
///
/// A runner without CLI flags of its own uses this as its whole configuration;
/// a runner with flags applies them on top.
pub fn effective_settings() -> Settings {
    settings::resolve(&stored_settings())
}

/// SPA origin for this run: the trunk dev server, or the in-process server.
pub fn spa_origin() -> String {
    if cfg!(debug_assertions) {
        DEBUG_SPA_ORIGIN.to_string()
    } else {
        format!("http://127.0.0.1:{}/", api_port())
    }
}

/// Config for the in-process server.
///
/// The host stays loopback: the webview loads the SPA from it, so the server
/// must not listen beyond the machine. `port` is the decision the caller made
/// through [`api_port`].
fn server_config(settings: &Settings, port: u16) -> Config {
    Config {
        host: IpAddr::from([127, 0, 0, 1]),
        port,
        verbosity: settings.verbosity,
        enable_docs: settings.enable_docs,
        skills_dir: skills_dir_override(settings),
        // The host stays loopback and ignores the stored value; the webview loads the SPA from it.
        host_fixed: true,
    }
}

/// Skills directory: `C5T_SKILLS_DIR` wins over the stored value.
fn skills_dir_override(settings: &Settings) -> PathBuf {
    std::env::var("C5T_SKILLS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| settings.skills_dir.clone())
}

/// Loopback health endpoint of the server on `port`.
pub fn health_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/health")
}

/// Ensure a service answers on `port`, spawning the sibling when none does.
///
/// The GUI runs this before it creates its window. A healthy service means the
/// GUI is a pure view over it; no service means the GUI starts `context-service`
/// next to itself and waits for `/health`. Returns the operator-facing message
/// for the error dialog.
pub fn ensure_service(port: u16) -> Result<(), String> {
    tauri::async_runtime::block_on(ensure_service_async(port))
}

/// Async half of [`ensure_service`], for callers already on a runtime.
pub async fn ensure_service_async(port: u16) -> Result<(), String> {
    let url = health_url(port);
    let client = health_client()?;

    if probe_health(&client, &url).await {
        return Ok(());
    }

    spawn_service()?;
    wait_until_healthy(&client, &url, HEALTH_TIMEOUT).await
}

/// Path of the sibling service binary: next to this executable.
///
/// The bundler puts both binaries in `Contents/MacOS`, and `cargo build` puts
/// both in `target/debug`, so the sibling is always a directory away.
pub(crate) fn sibling_service_path() -> Result<PathBuf, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("Failed to locate this executable: {error}"))?;
    let directory = executable
        .parent()
        .ok_or_else(|| format!("{} has no directory", executable.display()))?;

    Ok(directory.join("context-service"))
}

/// Start the sibling service detached from this process.
///
/// The service outlives the GUI: the GUI tracks no handle, and its own process
/// group keeps a Ctrl+C in a dev terminal from taking the service down.
fn spawn_service() -> Result<(), String> {
    let service = sibling_service_path()?;

    let mut command = std::process::Command::new(&service);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    command
        .spawn()
        .map_err(|error| format!("Failed to start {}: {error}", service.display()))?;

    Ok(())
}

/// Wait for `url` to answer HTTP 200, or report why it never did.
async fn wait_until_healthy(
    client: &reqwest::Client,
    url: &str,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if probe_health(client, url).await {
            return Ok(());
        }

        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        tokio::time::sleep(HEALTH_POLL_INTERVAL.min(remaining)).await;
    }

    Err(format!(
        "The context service did not answer {url} within {} seconds.",
        timeout.as_secs_f64()
    ))
}

/// Build the client used for health probes.
pub(crate) fn health_client() -> Result<reqwest::Client, String> {
    // Install ring as the default crypto provider for rustls. Workspace builds
    // enable rustls without a provider, and building a client then panics.
    let _ = rustls::crypto::ring::default_provider().install_default();

    reqwest::Client::builder()
        .timeout(HEALTH_REQUEST_TIMEOUT)
        .build()
        .map_err(|error| format!("Failed to create the health probe client: {error}"))
}

/// Attach to a running server, or start one and wait until it is healthy.
///
/// The caller owns the notifier and the update flow, so the same pair drives the
/// server and the updater. Returns an error message for the operator when the
/// server cannot be reached and cannot be started.
pub async fn start_server(
    port: u16,
    health_url: &str,
    notifier: ChangeNotifier,
    update_flow: impl UpdateFlow + 'static,
) -> Result<(), String> {
    let client = health_client()?;

    if probe_health(&client, health_url).await {
        return Ok(());
    }

    let db_path = get_db_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    let db = SqliteDatabase::open(&db_path)
        .await
        .map_err(|error| format!("Failed to open {}: {error}", db_path.display()))?;
    db.migrate()
        .map_err(|error| format!("Failed to migrate {}: {error}", db_path.display()))?;

    let config = server_config(&effective_settings(), port);

    let (failure_tx, mut failure_rx) = tokio::sync::mpsc::unbounded_channel();
    tauri::async_runtime::spawn(async move {
        let failure = match api::run(config, db, notifier, update_flow).await {
            Ok(()) => "The context server stopped before it became ready.".to_string(),
            Err(error) => {
                eprintln!("context: API server stopped: {error}");
                error.to_string()
            }
        };
        let _ = failure_tx.send(failure);
    });

    await_health(&client, health_url, HEALTH_TIMEOUT, &mut failure_rx).await
}

/// Wait for the server to answer `/health`, or report why it never will.
///
/// Stops early with the server's own error, so the operator sees the real
/// cause — a bind failure, for example — instead of a timeout message.
async fn await_health(
    client: &reqwest::Client,
    health_url: &str,
    timeout: Duration,
    failure_rx: &mut tokio::sync::mpsc::UnboundedReceiver<String>,
) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if probe_health(client, health_url).await {
            return Ok(());
        }

        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        tokio::select! {
            failure = failure_rx.recv() => {
                let cause = failure
                    .unwrap_or_else(|| "The context server task ended unexpectedly.".to_string());
                return Err(format!("Failed to start the context server: {cause}"));
            }
            () = tokio::time::sleep(HEALTH_POLL_INTERVAL.min(remaining)) => {}
        }
    }

    Err(format!(
        "The context server did not answer {health_url} within {} seconds.",
        timeout.as_secs_f64()
    ))
}

/// Report whether `url` answers HTTP 200.
pub(crate) async fn probe_health(client: &reqwest::Client, url: &str) -> bool {
    matches!(
        client.get(url).send().await,
        Ok(response) if response.status() == reqwest::StatusCode::OK
    )
}

/// Report whether the SPA origin at `url` answers HTTP 200.
///
/// The URL is a parameter so the recovery watch can probe the origin this build
/// loads, and a test can point it at its own listener instead.
pub(crate) async fn origin_ready(url: &str) -> bool {
    let Ok(client) = health_client() else {
        return false;
    };

    probe_health(&client, url).await
}
