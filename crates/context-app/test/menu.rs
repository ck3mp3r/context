//! Tests for the application menu contract.
//!
//! The menu itself needs a running `AppHandle`, so the headless coverage pins the
//! constants that decide what the handler in `main` does: a distinct id for the
//! close item, Quit on the platform shortcut, and the shared Quit id.
//!
//! Cmd+W belongs to the webview (decision 17), so the accelerators are pinned too:
//! Close Window takes Cmd+Shift+W and no item claims Cmd+W.

use super::*;

#[test]
fn application_menu_item_closes_the_view() {
    assert_eq!(CLOSE_VIEW_TEXT, "Close Window");
}

#[test]
fn application_menu_title_names_the_app() {
    assert_eq!(APP_MENU_TITLE, "Context");
}

#[test]
fn about_item_names_the_app() {
    assert_eq!(ABOUT_TEXT, "About Context");
}

#[test]
fn quit_item_keeps_the_platform_shortcut() {
    assert_eq!(QUIT_ACCELERATOR, "CmdOrCtrl+Q");
}

#[test]
fn close_item_id_does_not_reach_the_exit_arm() {
    assert_ne!(CLOSE_VIEW_ITEM_ID, QUIT_ITEM_ID);
}

#[test]
fn tray_quit_keeps_its_label_and_id() {
    assert_eq!(QUIT_TEXT, "Quit");
    assert_eq!(QUIT_ITEM_ID, "quit");
}

#[test]
fn close_view_item_takes_the_window_shortcut() {
    assert_eq!(CLOSE_VIEW_ACCELERATOR, "CmdOrCtrl+Shift+W");
}

#[test]
fn no_menu_item_takes_cmd_w_from_the_webview() {
    for accelerator in [CLOSE_VIEW_ACCELERATOR, QUIT_ACCELERATOR] {
        assert_ne!(
            accelerator, "CmdOrCtrl+W",
            "Cmd+W closes a tab in the webview, so no native item may claim it (decision 17)"
        );
    }
}
