//! Native application menu bar.
//!
//! A WKWebView text field edits through the macOS responder chain, so the
//! clipboard and selection shortcuts only work when the app installs an Edit
//! menu with the matching native items. Apart from Quit, every item here is a
//! predefined item: muda maps it to its native selector, accelerator, and
//! localization, which is what sends `undo:`, `cut:`, `copy:`, `paste:`, and
//! `selectAll:` down the responder chain to the focused webview field.
//!
//! Quit is a custom item with its own handler, because the predefined Quit
//! maps to the native `terminate:` selector. Termination tears the window down
//! and the last-window teardown raises `ExitRequested { code: None }`, which the
//! window-close veto prevents — the predefined item could never exit. The
//! custom item requests the exit through `AppHandle::exit(0)`, which raises
//! `ExitRequested { code: Some(0) }` and is not vetoed.
//!
//! macOS renders the first submenu as the application menu, so the `context`
//! submenu holds About and Quit. Plain top-level items are ignored on macOS;
//! every entry below is a submenu.

use tauri::AppHandle;
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};

/// Title of the application menu, shown next to the Apple menu.
const APP_MENU_TITLE: &str = "context";
/// Label of the About item. Without one, macOS names it after the process.
const ABOUT_TEXT: &str = "About context";
/// Label of the Quit item, shared by the application menu and the tray menu.
pub const QUIT_TEXT: &str = "Quit";
/// Id of the Quit item, matched by the menu event handler in `main`.
pub const QUIT_ITEM_ID: &str = "quit";
/// Keyboard accelerator of the Quit item.
const QUIT_ACCELERATOR: &str = "CmdOrCtrl+Q";
/// Title of the Edit submenu.
const EDIT_MENU_TITLE: &str = "Edit";
/// Title of the Window submenu.
const WINDOW_MENU_TITLE: &str = "Window";

/// Build the menu bar and install it as the application menu.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let quit = MenuItemBuilder::with_id(QUIT_ITEM_ID, QUIT_TEXT)
        .accelerator(QUIT_ACCELERATOR)
        .build(app)?;

    let application = SubmenuBuilder::new(app, APP_MENU_TITLE)
        .about_with_text(ABOUT_TEXT, None)
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
