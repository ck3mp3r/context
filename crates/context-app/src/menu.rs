//! Native application menu bar.
//!
//! A WKWebView text field edits through the macOS responder chain, so the
//! clipboard and selection shortcuts only work when the app installs an Edit
//! menu with the matching native items. Apart from Close Window and Quit, every
//! item here is a predefined item: muda maps it to its native selector,
//! accelerator, and localization, which is what sends `undo:`, `cut:`, `copy:`,
//! `paste:`, and `selectAll:` down the responder chain to the focused webview
//! field.
//!
//! Quit ends this process only. The service keeps serving, so Cmd+Q on the GUI
//! closes the view and leaves the tray and the API running. That is why Quit is
//! a custom item instead of the predefined one: the predefined Quit maps to the
//! native `terminate:` selector, whose last-window teardown raises
//! `ExitRequested { code: None }`, which the window-close veto prevents.
//!
//! Close Window closes the view and lets the process live, so a Dock click can
//! bring the window back. The Window submenu carries the predefined close item,
//! which is the Cmd+W surface.
//!
//! The GUI runs as a regular app, so the system displays this menu: it belongs to
//! the frontmost app and carries the Dock entry and the Cmd+Tab entry.
//!
//! macOS renders the first submenu as the application menu, so the `context`
//! submenu holds About, Close Window, and Quit. Plain top-level items are ignored
//! on macOS; every entry below is a submenu.

use tauri::AppHandle;
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};

/// Title of the application menu, shown next to the Apple menu.
const APP_MENU_TITLE: &str = "Context";
/// Label of the About item. Without one, macOS names it after the process.
const ABOUT_TEXT: &str = "About Context";
/// Label of the Quit item. The GUI application menu and the service tray both
/// use it.
pub const QUIT_TEXT: &str = "Quit";
/// Id of the Quit item. Both the GUI application menu and the service tray build
/// an item carrying it, and each process handles its own: the GUI's quit ends
/// that process and leaves the service serving, while the tray's quit ends the
/// GUIs the service spawned before the service exits.
pub const QUIT_ITEM_ID: &str = "quit";
/// Label of the application-menu item that closes the window.
const CLOSE_VIEW_TEXT: &str = "Close Window";
/// Id of the close item, matched by the menu event handler in `main`.
pub const CLOSE_VIEW_ITEM_ID: &str = "close-view";
/// Keyboard accelerator of Quit.
///
/// Cmd+Q quits this process only. The service keeps running, and the next GUI
/// launch attaches to it again.
const QUIT_ACCELERATOR: &str = "CmdOrCtrl+Q";
/// Title of the Edit submenu.
const EDIT_MENU_TITLE: &str = "Edit";
/// Title of the Window submenu.
const WINDOW_MENU_TITLE: &str = "Window";

#[cfg(test)]
#[path = "../test/menu.rs"]
mod menu_test;

/// Build the menu bar and install it as the application menu.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let close_view = MenuItemBuilder::with_id(CLOSE_VIEW_ITEM_ID, CLOSE_VIEW_TEXT).build(app)?;
    let quit = MenuItemBuilder::with_id(QUIT_ITEM_ID, QUIT_TEXT)
        .accelerator(QUIT_ACCELERATOR)
        .build(app)?;

    let application = SubmenuBuilder::new(app, APP_MENU_TITLE)
        .about_with_text(ABOUT_TEXT, None)
        .separator()
        .item(&close_view)
        .separator()
        .item(&quit)
        .build()?;

    let edit = SubmenuBuilder::new(app, EDIT_MENU_TITLE)
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let window = SubmenuBuilder::new(app, WINDOW_MENU_TITLE)
        .minimize()
        .maximize()
        .separator()
        .close_window()
        .build()?;

    let menu = MenuBuilder::new(app)
        .item(&application)
        .item(&edit)
        .item(&window)
        .build()?;

    app.set_menu(menu)?;

    Ok(())
}
