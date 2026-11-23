//! Shared implementation of the c5t desktop app binaries.
//!
//! The GUI binary (`context`) wires these modules into a Tauri app: `lifecycle`
//! owns the window and its deep links, `menu` and `tray` own the menu bar
//! surfaces, and `server` decides the port and attaches to or starts the server
//! in-process. A future service binary reuses the same modules, so nothing here
//! may depend on how many binaries ship the crate.

pub mod lifecycle;
pub mod menu;
pub mod server;
pub mod tray;

pub use lifecycle::{SETTINGS_PATH, request_main_window, request_window_at};
pub use menu::{CLOSE_VIEW_ITEM_ID, QUIT_ITEM_ID, QUIT_TEXT};
pub use server::{api_port, health_url, spa_origin, start_server};
pub use tray::{SETTINGS_ITEM_ID, SHOW_ITEM_ID, TrayState};
