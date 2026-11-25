//! macOS menu bar status item.
//!
//! The service binary is the macOS accessory agent: no Dock icon and no Cmd+Tab
//! entry. It owns the tray, so the menu outlives a closed view; the GUI runs a
//! regular activation policy and is a disposable window over that service.
//!
//! Show, Settings… and Quit route through [`handle_menu_event`]. Show brings the
//! GUI forward, launching it when it is not running; Settings… does the same and
//! lands it on the SPA settings page. Both pass their route through
//! `ServiceState::launch_gui`. Quit ends the GUIs this service spawned before the
//! service exits. It is a custom item carrying `menu::QUIT_ITEM_ID`, so it flows
//! into the service's exit path and never touches the vetoed `terminate:` path.
//!
//! Three rows are informational: each is disabled, so it renders grayed and a
//! click is inert.
//!
//! - The MCP row names the MCP endpoint of the build profile, the URL a client
//!   needs while it is configured.
//! - The sync row reports whether the sync repository is initialized and whether
//!   its working tree is clean. There is nothing to report before sync is
//!   initialized, so the row exists only after initialization: two menus are
//!   built once — one with the row, one without — and the background poll swaps
//!   them when presence changes and rewrites the text in place when only the
//!   status changes.
//! - The update row names the update phase the service reports. Consent and
//!   install belong to the SPA banner, so the row only reports.
//!
//! The glyph is an alpha-only template image. macOS composites a template image
//! for the light and the dark menu bar, so one asset serves both appearances.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Deserialize;
use tauri::image::Image;
use tauri::menu::{Menu, MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry, include_image};

use crate::launcher::ServiceState;
use crate::menu;

/// Id of the Show item, routed by [`handle_menu_event`].
pub const SHOW_ITEM_ID: &str = "show";
/// Label of the Show item.
const SHOW_TEXT: &str = "Show Context";
/// Id of the Settings item, routed by [`handle_menu_event`].
pub const SETTINGS_ITEM_ID: &str = "settings";
/// Label of the Settings item, which deep links to the SPA settings page.
const SETTINGS_TEXT: &str = "Settings…";
/// Id of the informational MCP item. It never fires a menu event.
const MCP_ITEM_ID: &str = "mcp";
/// Prefix of the MCP item label, followed by the endpoint URL.
const MCP_TEXT_PREFIX: &str = "MCP: ";
/// Tooltip of the status item, shown on hover.
const TRAY_TOOLTIP: &str = "Context";
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
/// Delay between status polls of the informational rows.
const SYNC_POLL_INTERVAL: Duration = Duration::from_secs(60);
/// Id of the informational update item. It never fires a menu event.
pub const UPDATE_ITEM_ID: &str = "update";
/// Prefix of the update item label, followed by the status sentence.
const UPDATE_TEXT_PREFIX: &str = "Update: ";
/// Label shown when no candidate waits.
const UPDATE_IDLE_TEXT: &str = "Update: up to date";
/// Label shown while a check runs.
const UPDATE_CHECKING_TEXT: &str = "Update: checking…";
/// Label shown once the candidate installed; the restart waits for the user.
const UPDATE_READY_TEXT: &str = "Update: ready — restart to finish";
/// Label shown while the candidate installs.
const UPDATE_INSTALLING_TEXT: &str = "Update: installing";
/// Label shown when the last attempt failed.
const UPDATE_FAILED_TEXT: &str = "Update: failed";
/// Suffix of a candidate that came from a pre-release.
const UPDATE_PRERELEASE_SUFFIX: &str = " (pre-release)";

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
/// The port comes from `crate::server::api_port()`, the single port decision point, so
/// the row always names the endpoint the app actually serves.
fn mcp_label() -> String {
    format!(
        "{MCP_TEXT_PREFIX}http://localhost:{}/mcp",
        crate::server::api_port()
    )
}

/// Tooltip of the status item for the current build profile.
///
/// A debug build names the profile, so the dev icon and the installed app icon
/// are distinguishable on hover; a release build keeps the bare app name.
fn tray_tooltip() -> &'static str {
    if cfg!(debug_assertions) {
        "Context (dev)"
    } else {
        TRAY_TOOLTIP
    }
}

/// Route one tray menu event to its action.
///
/// The service binary hands every menu event here, so a new firing tray item is
/// a match arm in this module instead of a second edit in the binary.
pub fn handle_menu_event(app: &AppHandle, id: &str) {
    match action_for(id) {
        TrayAction::LaunchGui(route) => app.state::<ServiceState>().launch_gui(route),
        TrayAction::Quit => {
            app.state::<ServiceState>().terminate_children();
            app.exit(0);
        }
        TrayAction::Ignore => {}
    }
}

/// What one tray menu event asks the service to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayAction {
    /// Bring the GUI forward, on `route` when the item deep links.
    LaunchGui(Option<&'static str>),
    /// End the GUIs this service started, then the service itself.
    Quit,
    /// An informational row, or an item this module does not own.
    Ignore,
}

/// Action the tray item `id` asks for.
///
/// The informational rows (MCP, sync, update) are disabled, so they never fire a
/// menu event and route nowhere.
pub(crate) fn action_for(id: &str) -> TrayAction {
    match id {
        SHOW_ITEM_ID => TrayAction::LaunchGui(None),
        SETTINGS_ITEM_ID => TrayAction::LaunchGui(Some(crate::SETTINGS_PATH)),
        menu::QUIT_ITEM_ID => TrayAction::Quit,
        _ => TrayAction::Ignore,
    }
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
    /// The update row whose text carries the update status.
    update_item: MenuItem<Wry>,
    /// Text currently set on the update row.
    update_text: Mutex<Option<String>>,
}

/// Build the menu bar status item and install it without the sync row.
///
/// Returns the handles the status poll needs; the caller manages them.
pub fn build(app: &AppHandle) -> tauri::Result<TrayState> {
    let show = MenuItemBuilder::with_id(SHOW_ITEM_ID, SHOW_TEXT).build(app)?;
    let settings = MenuItemBuilder::with_id(SETTINGS_ITEM_ID, SETTINGS_TEXT).build(app)?;
    let mcp = MenuItemBuilder::with_id(MCP_ITEM_ID, mcp_label())
        .enabled(false)
        .build(app)?;
    let sync = MenuItemBuilder::with_id(SYNC_ITEM_ID, SYNC_NEVER_SYNCED_TEXT)
        .enabled(false)
        .build(app)?;
    let update = MenuItemBuilder::with_id(UPDATE_ITEM_ID, UPDATE_IDLE_TEXT)
        .enabled(false)
        .build(app)?;
    let quit = MenuItemBuilder::with_id(menu::QUIT_ITEM_ID, menu::QUIT_TEXT).build(app)?;

    let without_sync = MenuBuilder::new(app)
        .item(&show)
        .item(&settings)
        .separator()
        .item(&mcp)
        .item(&update)
        .separator()
        .item(&quit)
        .build()?;

    let with_sync = MenuBuilder::new(app)
        .item(&show)
        .item(&settings)
        .separator()
        .item(&mcp)
        .item(&update)
        .item(&sync)
        .separator()
        .item(&quit)
        .build()?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(TRAY_ICON)
        .icon_as_template(true)
        .tooltip(tray_tooltip())
        .show_menu_on_left_click(true)
        .menu(&without_sync)
        .build(app)?;

    Ok(TrayState {
        with_sync,
        without_sync,
        sync_item: sync,
        sync_shown: AtomicBool::new(false),
        sync_text: Mutex::new(None),
        update_item: update,
        update_text: Mutex::new(None),
    })
}

/// Start the background task that mirrors the sync and update status onto the
/// tray.
///
/// The task fetches once at startup and then every `SYNC_POLL_INTERVAL`. Each row
/// reads its own endpoint, so a failed fetch is skipped and that row keeps its
/// last known state.
pub fn spawn_status_poll(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let client = match crate::server::health_client() {
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

/// Response subset of `GET /api/v1/update/status` the update row depends on.
///
/// The row never shows the free-text `message` the endpoint also carries.
#[derive(Debug, Deserialize)]
struct UpdateStatusResponse {
    #[serde(default)]
    status: String,
    #[serde(default)]
    candidate_version: Option<String>,
    #[serde(default)]
    candidate_is_prerelease: Option<bool>,
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

/// Text of the update row for one status snapshot.
///
/// A `None` snapshot is a failed fetch, which the caller skips; the idempotent
/// label it maps to is never applied.
fn update_label(status: Option<&UpdateStatusResponse>) -> String {
    let Some(status) = status else {
        return UPDATE_IDLE_TEXT.to_string();
    };

    match status.status.as_str() {
        "checking" => UPDATE_CHECKING_TEXT.to_string(),
        "available" => {
            let version = status.candidate_version.as_deref().unwrap_or_default();
            let mut label = if version.is_empty() {
                format!("{UPDATE_TEXT_PREFIX}available")
            } else {
                format!("{UPDATE_TEXT_PREFIX}{version} available")
            };

            if status.candidate_is_prerelease == Some(true) {
                label.push_str(UPDATE_PRERELEASE_SUFFIX);
            }

            label
        }
        "downloading" => match status.candidate_version.as_deref() {
            Some(version) => format!("{UPDATE_TEXT_PREFIX}downloading {version}"),
            None => format!("{UPDATE_TEXT_PREFIX}downloading"),
        },
        "installing" => UPDATE_INSTALLING_TEXT.to_string(),
        "ready" => UPDATE_READY_TEXT.to_string(),
        "failed" => UPDATE_FAILED_TEXT.to_string(),
        _ => UPDATE_IDLE_TEXT.to_string(),
    }
}

/// Loopback status endpoint of the server on the build profile's port.
fn status_url() -> String {
    format!(
        "http://127.0.0.1:{}/api/v1/sync/status",
        crate::server::api_port()
    )
}

/// Loopback update status endpoint of the server on the build profile's port.
fn update_status_url() -> String {
    format!(
        "http://127.0.0.1:{}/api/v1/update/status",
        crate::server::api_port()
    )
}

/// Fetch one status snapshot, or `None` when the server cannot answer.
async fn fetch_status(client: &reqwest::Client) -> Option<SyncStatusResponse> {
    let response = client.get(status_url()).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    response.json::<SyncStatusResponse>().await.ok()
}

/// Fetch one update snapshot, or `None` when the server cannot answer.
async fn fetch_update_status(client: &reqwest::Client) -> Option<UpdateStatusResponse> {
    let response = client.get(update_status_url()).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    response.json::<UpdateStatusResponse>().await.ok()
}

/// Apply one poll to both rows; a fetch failure leaves its row untouched.
async fn refresh_status(app: &AppHandle, client: &reqwest::Client) {
    if let Some(status) = fetch_status(client).await
        && let Err(error) = apply_status(app, &status)
    {
        eprintln!("context: failed to update the sync status row: {error}");
    }

    if let Some(status) = fetch_update_status(client).await
        && let Err(error) = apply_update_status(app, &status)
    {
        eprintln!("context: failed to update the update row: {error}");
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

/// Rewrite the update row text when the status changed.
///
/// The row is informational and always present, so only its text moves; a
/// repeated status writes nothing.
fn apply_update_status(app: &AppHandle, status: &UpdateStatusResponse) -> tauri::Result<()> {
    let state = app.state::<TrayState>();
    let label = update_label(Some(status));

    let mut current = state
        .update_text
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if current.as_deref() != Some(label.as_str()) {
        state.update_item.set_text(&label)?;
        *current = Some(label);
    }

    Ok(())
}
