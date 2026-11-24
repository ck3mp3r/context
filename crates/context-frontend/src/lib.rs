#![no_main]

use wasm_bindgen::prelude::*;

mod api;
mod app;
mod breadcrumb_state;
mod components;
mod hooks;
mod mermaid;
mod models;
mod pages;
mod theme;
/// Update wording: the phrases the banner renders, kept free of the view layer.
pub mod update_status;
mod utils;
mod websocket;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);

    // Initialize mermaid with default dark theme
    crate::mermaid::mermaid_init_for_theme(false);
}
