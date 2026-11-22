//! Native application menu bar.
//!
//! A WKWebView text field edits through the macOS responder chain, so the
//! clipboard and selection shortcuts only work when the app installs an Edit
//! menu with the matching native items. Apart from the close item, every item
//! here is a predefined item: muda maps it to its native selector, accelerator,
//! and localization, which is what sends `undo:`, `cut:`, `copy:`, `paste:`, and
//! `selectAll:` down the responder chain to the focused webview field.
//!
//! The application menu holds no Quit. The tray icon runs the service, so the
//! process outlives the window and the tray owns the only process exit. The
//! predefined Quit is unusable there anyway: it maps to the native `terminate:`
//! selector, whose last-window teardown raises `ExitRequested { code: None }`,
//! which the window-close veto prevents. The custom close item instead closes
//! the window through `WebviewWindow::close`, so Cmd+Q and Cmd+W do the same
//! thing: the view goes away and the server keeps serving.
//!
//! The app is a macOS accessory agent, so the system never displays this menu:
//! the menu bar belongs to the frontmost regular app. The menu stays installed
//! because it still dispatches its key equivalents through the responder chain.
//! That keeps the Edit shortcuts working in webview text fields and the hidden
//! Cmd+Q equivalent closing the view.
//!
//! macOS renders the first submenu as the application menu, so the `context`
//! submenu holds About and Close Window. Plain top-level items are ignored on
//! macOS; every entry below is a submenu.

use tauri::AppHandle;
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};

/// Title of the application menu, shown next to the Apple menu.
const APP_MENU_TITLE: &str = "context";
/// Label of the About item. Without one, macOS names it after the process.
const ABOUT_TEXT: &str = "About context";
/// Label of the Quit item of the tray menu.
pub const QUIT_TEXT: &str = "Quit";
/// Id of the Quit item of the tray menu, matched by the menu event handler in
/// `main`. Only the tray carries this id: the application menu closes the
/// window instead of quitting.
pub const QUIT_ITEM_ID: &str = "quit";
/// Label of the application-menu item that closes the window.
const CLOSE_VIEW_TEXT: &str = "Close Window";
/// Id of the close item, matched by the menu event handler in `main`.
pub const CLOSE_VIEW_ITEM_ID: &str = "close-view";
/// Keyboard accelerator of the close item.
///
/// Cmd+Q is the platform quit gesture. Binding it to the close item keeps muscle
/// memory on an action that is safe and reversible: the view closes, the server
/// keeps running, and the tray brings the view back.
const CLOSE_VIEW_ACCELERATOR: &str = "CmdOrCtrl+Q";
/// Title of the Edit submenu.
const EDIT_MENU_TITLE: &str = "Edit";
/// Title of the Window submenu.
const WINDOW_MENU_TITLE: &str = "Window";

#[cfg(test)]
#[path = "../test/menu.rs"]
mod menu_test;

/// Build the menu bar and install it as the application menu.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let close_view = MenuItemBuilder::with_id(CLOSE_VIEW_ITEM_ID, CLOSE_VIEW_TEXT)
        .accelerator(CLOSE_VIEW_ACCELERATOR)
        .build(app)?;

    let application = SubmenuBuilder::new(app, APP_MENU_TITLE)
        .about_with_text(ABOUT_TEXT, None)
        .separator()
        .item(&close_view)
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
