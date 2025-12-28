use leptos::prelude::*;
use wasm_bindgen::prelude::*;

mod api;
mod app;
mod components;
mod models;
mod pages;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
