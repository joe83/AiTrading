use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use crate::AppState;

/// WebSocket server for pushing real-time data to mobile app clients.
pub fn create_ws_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    info!("New WebSocket connection request");
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to broadcast channel for updates
    let mut rx = state.ws_broadcast.subscribe();

    info!("WebSocket client connected");

    // Send initial state
    let initial = serde_json::json!({
        "type": "connected",
        "message": "AI Trading Platform WebSocket connected",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    if let Ok(msg) = serde_json::to_string(&initial) {
        let _ = sender.send(Message::Text(msg)).await;
    }

    // Spawn a task to forward broadcast messages to this client
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(text) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming messages from the client
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    debug!("WS received: {text}");
                    // Handle client commands (subscribe to specific symbols, etc.)
                    if let Ok(cmd) = serde_json::from_str::<WsCommand>(&text) {
                        match cmd.action.as_str() {
                            "ping" => {
                                debug!("WS ping received");
                            }
                            "subscribe" => {
                                info!("WS subscribe: {:?}", cmd.symbols);
                            }
                            _ => {
                                debug!("Unknown WS command: {}", cmd.action);
                            }
                        }
                    }
                }
                Message::Ping(data) => {
                    debug!("WS ping");
                }
                Message::Close(_) => {
                    info!("WebSocket client disconnected");
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort();
        }
        _ = (&mut recv_task) => {
            send_task.abort();
        }
    }

    info!("WebSocket client handler exited");
}

#[derive(serde::Deserialize)]
struct WsCommand {
    action: String,
    symbols: Option<Vec<String>>,
}

/// Types of messages that can be broadcast to WebSocket clients.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum WsBroadcast {
    #[serde(rename = "price_update")]
    PriceUpdate {
        symbol: String,
        exchange: String,
        price: String,
        change_pct: f64,
        timestamp: String,
    },
    #[serde(rename = "signal")]
    Signal {
        symbol: String,
        action: String,
        confidence: f64,
        reasoning: String,
        timestamp: String,
    },
    #[serde(rename = "position_update")]
    PositionUpdate {
        symbol: String,
        side: String,
        pnl: String,
        pnl_pct: f64,
        timestamp: String,
    },
    #[serde(rename = "trade_executed")]
    TradeExecuted {
        symbol: String,
        side: String,
        quantity: String,
        price: String,
        timestamp: String,
    },
    #[serde(rename = "alert")]
    Alert {
        level: String,
        message: String,
        timestamp: String,
    },
    #[serde(rename = "system_status")]
    SystemStatus {
        trading_mode: String,
        is_paused: bool,
        open_positions: usize,
        timestamp: String,
    },
}
