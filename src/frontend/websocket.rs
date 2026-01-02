//! WebSocket connection management for real-time updates

use codee::string::FromToStringCodec;
use leptos::prelude::*;
use leptos_use::core::ConnectionReadyState;
use leptos_use::{UseWebSocketReturn, use_websocket};

use crate::models::UpdateMessage;

// Development: Trunk proxy at /dev/ws forwards to backend ws://localhost:3737/ws
#[cfg(debug_assertions)]
const WS_URL: &str = "ws://localhost:8080/dev/ws";

// Production: Direct connection to backend WebSocket
#[cfg(not(debug_assertions))]
fn get_ws_url() -> String {
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

/// WebSocket context containing connection state and latest update message
#[derive(Clone, Copy)]
pub struct WebSocketContext {
    pub ready_state: Signal<ConnectionReadyState>,
    pub last_message: Signal<Option<UpdateMessage>>,
}

/// Provider component that establishes WebSocket connection and provides context to children
#[component]
pub fn WebSocketProvider(children: Children) -> impl IntoView {
    // Get WebSocket URL based on build mode
    #[cfg(debug_assertions)]
    let url = WS_URL;

    #[cfg(not(debug_assertions))]
    let url = get_ws_url();

    // Create WebSocket connection
    let UseWebSocketReturn {
        ready_state,
        message,
        send: _,
        open: _,
        close: _,
        ..
    } = use_websocket::<String, String, FromToStringCodec>(&url);

    // Parse and store the latest update message
    let last_message = Signal::derive(move || {
        message
            .get()
            .and_then(|msg| match serde_json::from_str::<UpdateMessage>(&msg) {
                Ok(update) => {
                    web_sys::console::log_1(
                        &format!("Received WebSocket update: {:?}", update).into(),
                    );
                    Some(update)
                }
                Err(e) => {
                    web_sys::console::error_1(
                        &format!("Failed to parse WebSocket message: {}", e).into(),
                    );
                    None
                }
            })
    });

    // Provide context
    let context = WebSocketContext {
        ready_state,
        last_message,
    };
    provide_context(context);

    children()
}

/// Hook to access WebSocket connection state
pub fn use_websocket_connection() -> Signal<ConnectionReadyState> {
    expect_context::<WebSocketContext>().ready_state
}

/// Hook to access latest WebSocket update message
pub fn use_websocket_updates() -> Signal<Option<UpdateMessage>> {
    expect_context::<WebSocketContext>().last_message
}
