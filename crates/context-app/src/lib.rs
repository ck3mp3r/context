//! Shared implementation of the c5t desktop app binaries.
//!
//! Two binaries ship from this crate: the GUI (`context`) wires these modules
//! into a windowed Tauri app, and the service (`context-service`) wires the same
//! modules into a windowless one. `lifecycle` owns the GUI window and its deep
//! links, `menu` and `tray` own the menu bar surfaces, `server` decides the port
//! and attaches to or starts the server in-process, and `autostart` is the
//! start-at-login hook the service calls.

pub mod autostart;
pub mod lifecycle;
pub mod menu;
pub mod server;
pub mod tray;

pub use autostart::apply_start_at_login;
pub use lifecycle::{SETTINGS_PATH, request_main_window, request_window_at};
pub use menu::{CLOSE_VIEW_ITEM_ID, QUIT_ITEM_ID, QUIT_TEXT};
pub use server::{api_port, effective_settings, health_url, spa_origin, start_server};
pub use tray::{SETTINGS_ITEM_ID, SHOW_ITEM_ID, TrayState};
