#![no_main]

use wasm_bindgen::prelude::*;

mod api;
mod app;
mod components;
mod hooks;
mod models;
mod pages;
mod theme;
mod websocket;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
