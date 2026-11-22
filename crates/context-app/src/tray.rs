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

use tauri::image::Image;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, include_image};

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

/// Build the menu bar status item and install it.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id(SHOW_ITEM_ID, SHOW_TEXT).build(app)?;
    let mcp = MenuItemBuilder::with_id(MCP_ITEM_ID, mcp_label())
        .enabled(false)
        .build(app)?;
    let quit = MenuItemBuilder::with_id(menu::QUIT_ITEM_ID, menu::QUIT_TEXT).build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show)
        .separator()
        .item(&mcp)
        .separator()
        .item(&quit)
        .build()?;

    TrayIconBuilder::new()
        .icon(TRAY_ICON)
        .icon_as_template(true)
        .tooltip(TRAY_TOOLTIP)
        .show_menu_on_left_click(true)
        .menu(&menu)
        .build(app)?;

    Ok(())
}
