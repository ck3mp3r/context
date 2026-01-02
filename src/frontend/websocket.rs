//! WebSocket connection management for real-time updates

use codee::string::FromToStringCodec;
use leptos::prelude::*;
use leptos_use::core::ConnectionReadyState;
use leptos_use::{UseWebSocketReturn, use_websocket};

// Development: Trunk proxy at /dev/ws forwards to backend ws://localhost:3737/ws
#[cfg(debug_assertions)]
const WS_URL: &str = "ws://localhost:8080/dev/ws";

// Production: Direct connection to backend WebSocket
#[cfg(not(debug_assertions))]
fn get_ws_url() -> String {
    use wasm_bindgen::JsCast;

    let window = web_sys::window().expect("no window");
    let location = window.location();
    let protocol = location.protocol().unwrap_or_else(|_| "http:".to_string());
    let host = location
        .host()
        .unwrap_or_else(|_| "localhost:3737".to_string());

    // Convert http(s) to ws(s)
    let ws_protocol = if protocol == "https:" { "wss:" } else { "ws:" };

    format!("{}//{}/ws", ws_protocol, host)
}

/// WebSocket connection hook
///
/// Returns the connection ready state signal that can be used to display connection status
pub fn use_websocket_connection() -> Signal<ConnectionReadyState> {
    // Get WebSocket URL based on build mode
    #[cfg(debug_assertions)]
    let url = WS_URL;

    #[cfg(not(debug_assertions))]
    let url = get_ws_url();

    // Create WebSocket connection with leptos-use
    // Using FromToStringCodec for simple string messages (will expand later for JSON)
    let UseWebSocketReturn {
        ready_state,
        message: _,
        send: _,
        open: _,
        close: _,
        ..
    } = use_websocket::<String, String, FromToStringCodec>(&url);

    // For now, just return the ready state
    // Later we'll expand this to handle actual messages
    ready_state
}
