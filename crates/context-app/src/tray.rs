//! macOS menu bar status item.
//!
//! The app keeps serving MCP clients after the window closes, so a closed
//! window would leave the app reachable only through the dock. The tray gives
//! the app a permanent handle: it reveals the main window, or quits.
//!
//! Both actions reuse delivered code. Show calls `lifecycle::request_main_window`,
//! the same path the dock reopen uses. Quit is a custom item carrying
//! `menu::QUIT_ITEM_ID`, so it flows into the existing `AppHandle::exit(0)`
//! handler and never touches the vetoed `terminate:` path.
//!
//! The MCP item is informational: it is disabled, so it renders grayed and a
//! click is inert. Its label carries the MCP endpoint of the build profile, the
//! URL a client needs while it is configured.
//!
//! The glyph is an alpha-only template image. macOS composites a template image
//! for the light and the dark menu bar, so one asset serves both appearances.
//!
//! The sync row is informational too: it reports whether the sync repository is
//! initialized and whether its working tree is clean. There is nothing to report
//! before sync is initialized, so the row exists only after initialization. Two
//! menus are built once — one with the row, one without — and the background poll
//! swaps them when presence changes and rewrites the row text in place when only
//! the status changes.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Deserialize;
use tauri::image::Image;
use tauri::menu::{Menu, MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry, include_image};

use crate::menu;

/// Id of the Show item, matched by the menu event handler in `main`.
pub const SHOW_ITEM_ID: &str = "show";
/// Label of the Show item.
const SHOW_TEXT: &str = "Show context";
/// Id of the informational MCP item. It never fires a menu event.
const MCP_ITEM_ID: &str = "mcp";
/// Prefix of the MCP item label, followed by the endpoint URL.
const MCP_TEXT_PREFIX: &str = "MCP: ";
/// Tooltip of the status item, shown on hover.
const TRAY_TOOLTIP: &str = "context";
/// Id of the status item, used to find it again for menu swaps.
const TRAY_ID: &str = "context";
/// Id of the informational sync item. It never fires a menu event.
const SYNC_ITEM_ID: &str = "sync";
/// Prefix of the sync item label, followed by the status sentence.
const SYNC_TEXT_PREFIX: &str = "Sync: ";
/// Word between the sync prefix and the last sync timestamp of a clean tree.
const SYNC_CLEAN_TEXT: &str = "clean · ";
/// Label shown when sync is initialized but has never exported.
const SYNC_NEVER_SYNCED_TEXT: &str = "Sync: never synced";
/// Label shown when the sync working tree has uncommitted changes.
const SYNC_PENDING_TEXT: &str = "Sync: pending changes";
/// Delay between sync status polls.
const SYNC_POLL_INTERVAL: Duration = Duration::from_secs(60);

/// Monochrome template glyph, derived from the favicon "C" mark.
///
/// Template images carry the shape in the alpha channel only; the RGB channels
/// stay black and macOS ignores them.
const TRAY_ICON: Image<'static> = include_image!("icons/tray-template.png");

#[cfg(test)]
#[path = "../test/tray.rs"]
mod tray_test;

/// Label of the informational MCP item for the current build profile.
///
/// The port comes from `crate::api_port()`, the single port decision point, so
/// the row always names the endpoint the app actually serves.
fn mcp_label() -> String {
    format!(
        "{MCP_TEXT_PREFIX}http://localhost:{}/mcp",
        crate::api_port()
    )
}

/// Handles the sync status poll needs to keep the tray in step with the server.
///
/// Both menus are built once at setup. A presence change swaps the whole menu;
/// a text change rewrites the one live item. The tray is reached through its id
/// so the poll never holds the icon handle across an await point.
pub struct TrayState {
    /// Menu that contains the sync row.
    with_sync: Menu<Wry>,
    /// Menu that omits the sync row.
    without_sync: Menu<Wry>,
    /// The sync row whose text carries the status.
    sync_item: MenuItem<Wry>,
    /// Whether the tray currently shows the sync row.
    sync_shown: AtomicBool,
    /// Text currently set on the sync row.
    sync_text: Mutex<Option<String>>,
}

/// Build the menu bar status item and install it without the sync row.
///
/// Returns the handles the status poll needs; the caller manages them.
pub fn build(app: &AppHandle) -> tauri::Result<TrayState> {
    let show = MenuItemBuilder::with_id(SHOW_ITEM_ID, SHOW_TEXT).build(app)?;
    let mcp = MenuItemBuilder::with_id(MCP_ITEM_ID, mcp_label())
        .enabled(false)
        .build(app)?;
    let sync = MenuItemBuilder::with_id(SYNC_ITEM_ID, SYNC_NEVER_SYNCED_TEXT)
        .enabled(false)
        .build(app)?;
    let quit = MenuItemBuilder::with_id(menu::QUIT_ITEM_ID, menu::QUIT_TEXT).build(app)?;

    let without_sync = MenuBuilder::new(app)
        .item(&show)
        .separator()
        .item(&mcp)
        .separator()
        .item(&quit)
        .build()?;

    let with_sync = MenuBuilder::new(app)
        .item(&show)
        .separator()
        .item(&mcp)
        .item(&sync)
        .separator()
        .item(&quit)
        .build()?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(TRAY_ICON)
        .icon_as_template(true)
        .tooltip(TRAY_TOOLTIP)
        .show_menu_on_left_click(true)
        .menu(&without_sync)
        .build(app)?;

    Ok(TrayState {
        with_sync,
        without_sync,
        sync_item: sync,
        sync_shown: AtomicBool::new(false),
        sync_text: Mutex::new(None),
    })
}

/// Start the background task that mirrors the sync status onto the tray.
///
/// The task fetches once at startup and then every `SYNC_POLL_INTERVAL`. A failed
/// fetch is skipped, so the row keeps its last known state.
pub fn spawn_status_poll(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let client = match crate::health_client() {
            Ok(client) => client,
            Err(error) => {
                eprintln!("context: {error}");
                return;
            }
        };

        loop {
            refresh_status(&app, &client).await;
            tokio::time::sleep(SYNC_POLL_INTERVAL).await;
        }
    });
}

/// Response subset of `GET /api/v1/sync/status` the tray row depends on.
#[derive(Debug, Deserialize)]
struct SyncStatusResponse {
    #[serde(default)]
    data: Option<SyncData>,
}

/// The `data` object of the status response.
#[derive(Debug, Deserialize)]
struct SyncData {
    #[serde(default)]
    initialized: bool,
    #[serde(default)]
    last_synced_at: Option<String>,
    #[serde(default)]
    git: Option<SyncGit>,
}

/// Git working tree state of the sync repository.
#[derive(Debug, Deserialize)]
struct SyncGit {
    #[serde(default)]
    clean: bool,
}

/// Text of the sync row, or `None` when the row must be absent.
fn sync_label(data: Option<&SyncData>) -> Option<String> {
    let data = data?;
    if !data.initialized {
        return None;
    }

    if data.git.as_ref().is_some_and(|git| !git.clean) {
        return Some(SYNC_PENDING_TEXT.to_string());
    }

    Some(match data.last_synced_at.as_deref() {
        Some(timestamp) => format!("{SYNC_TEXT_PREFIX}{SYNC_CLEAN_TEXT}{timestamp}"),
        None => SYNC_NEVER_SYNCED_TEXT.to_string(),
    })
}

/// Loopback status endpoint of the server on the build profile's port.
fn status_url() -> String {
    format!("http://127.0.0.1:{}/api/v1/sync/status", crate::api_port())
}

/// Fetch one status snapshot, or `None` when the server cannot answer.
async fn fetch_status(client: &reqwest::Client) -> Option<SyncStatusResponse> {
    let response = client.get(status_url()).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    response.json::<SyncStatusResponse>().await.ok()
}

/// Apply one status snapshot to the tray; a fetch failure leaves it untouched.
async fn refresh_status(app: &AppHandle, client: &reqwest::Client) {
    let Some(status) = fetch_status(client).await else {
        return;
    };

    if let Err(error) = apply_status(app, &status) {
        eprintln!("context: failed to update the sync status row: {error}");
    }
}

/// Rewrite the row text and swap the menu when sync presence changed.
fn apply_status(app: &AppHandle, status: &SyncStatusResponse) -> tauri::Result<()> {
    let state = app.state::<TrayState>();
    let label = sync_label(status.data.as_ref());

    if let Some(label) = &label {
        let mut current = state
            .sync_text
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if current.as_deref() != Some(label.as_str()) {
            state.sync_item.set_text(label)?;
            *current = Some(label.clone());
        }
    }

    let show = label.is_some();
    if state.sync_shown.load(Ordering::SeqCst) != show
        && let Some(tray) = app.tray_by_id(TRAY_ID)
    {
        let menu = if show {
            state.with_sync.clone()
        } else {
            state.without_sync.clone()
        };
        tray.set_menu(Some(menu))?;
        state.sync_shown.store(show, Ordering::SeqCst);
    }

    Ok(())
}
