//! Native application menu bar.
//!
//! A WKWebView text field edits through the macOS responder chain, so the
//! clipboard and selection shortcuts only work when the app installs an Edit
//! menu with the matching native items. Every item here is a predefined item:
//! muda maps it to its native selector, accelerator, and localization, which is
//! what sends `undo:`, `cut:`, `copy:`, `paste:`, and `selectAll:` down the
//! responder chain to the focused webview field.
//!
//! macOS renders the first submenu as the application menu, so the `context`
//! submenu holds About and Quit. Plain top-level items are ignored on macOS;
//! every entry below is a submenu.

use tauri::AppHandle;
use tauri::menu::{MenuBuilder, SubmenuBuilder};

/// Title of the application menu, shown next to the Apple menu.
const APP_MENU_TITLE: &str = "context";
/// Label of the About item. Without one, macOS names it after the process.
const ABOUT_TEXT: &str = "About context";
/// Label of the Quit item.
const QUIT_TEXT: &str = "Quit";
/// Title of the Edit submenu.
const EDIT_MENU_TITLE: &str = "Edit";
/// Title of the Window submenu.
const WINDOW_MENU_TITLE: &str = "Window";

/// Build the menu bar and install it as the application menu.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let application = SubmenuBuilder::new(app, APP_MENU_TITLE)
        .about_with_text(ABOUT_TEXT, None)
        .separator()
        .quit_with_text(QUIT_TEXT)
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
