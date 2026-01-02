//! WebSocket handler for real-time updates.

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use tracing::{debug, error, info};

use super::state::AppState;
use crate::db::Database;
use crate::sync::GitOps;

/// WebSocket upgrade handler.
///
/// Accepts WebSocket upgrade requests and establishes a connection.
/// Once upgraded, streams UpdateMessages from ChangeNotifier to the client.
pub async fn ws_handler<D: Database + 'static, G: GitOps + Send + Sync + 'static>(
    ws: WebSocketUpgrade,
    State(state): State<AppState<D, G>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle an active WebSocket connection.
///
/// Subscribes to the ChangeNotifier and streams updates to the client.
/// Listens for client messages (mainly for connection management).
async fn handle_socket<D: Database, G: GitOps + Send + Sync>(
    mut socket: WebSocket,
    state: AppState<D, G>,
) {
    info!("WebSocket client connected");

    let mut rx = state.notifier().subscribe();

    loop {
        tokio::select! {
            // Receive messages from client
            Some(msg) = socket.recv() => {
                match msg {
                    Ok(Message::Text(text)) => {
                        debug!("Received from client: {}", text);
                        // Future: handle client messages if needed
                    }
                    Ok(Message::Close(_)) => {
                        info!("Client closed connection");
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }

            // Send updates to client
            Ok(update) = rx.recv() => {
                let json = match serde_json::to_string(&update) {
                    Ok(j) => j,
                    Err(e) => {
                        error!("Failed to serialize update: {}", e);
                        continue;
                    }
                };

                if let Err(e) = socket.send(Message::Text(json.into())).await {
                    error!("Failed to send update: {}", e);
                    break;
                }
            }
        }
    }

    info!("WebSocket client disconnected");
}
