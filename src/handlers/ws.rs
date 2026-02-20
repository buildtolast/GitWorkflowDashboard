use actix_ws::AggregatedMessage;
use actix_web::{web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

use crate::models::*;

/// Maximum concurrent WebSocket connections.
const MAX_WS_CONNECTIONS: usize = 50;

/// Shared connection counter type.
pub type WsConnectionCount = Arc<AtomicUsize>;

/// Create a new WebSocket connection counter.
pub fn new_connection_counter() -> WsConnectionCount {
    Arc::new(AtomicUsize::new(0))
}

/// GET /ws/live — WebSocket handler for real-time updates.
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    ws_tx: web::Data<broadcast::Sender<WsMessage>>,
    state: web::Data<SharedDashboardState>,
    dashboard_token: web::Data<Option<String>>,
    connection_count: web::Data<WsConnectionCount>,
) -> actix_web::Result<HttpResponse> {
    // Check auth if dashboard token is configured
    if let Some(ref expected) = **dashboard_token {
        let query_string = req.query_string();
        let token_from_query = query_string
            .split('&')
            .find_map(|param| {
                let mut parts = param.splitn(2, '=');
                match (parts.next(), parts.next()) {
                    (Some("token"), Some(v)) => Some(v.to_string()),
                    _ => None,
                }
            });

        let is_valid = token_from_query
            .as_deref()
            .map(|t| t == expected)
            .unwrap_or(false);

        if !is_valid {
            return Ok(HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "Invalid or missing authentication token"
            })));
        }
    }

    // Check connection limit
    let current = connection_count.load(Ordering::Relaxed);
    if current >= MAX_WS_CONNECTIONS {
        tracing::warn!(
            current = current,
            max = MAX_WS_CONNECTIONS,
            "WebSocket connection rejected — limit reached"
        );
        return Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "Too many WebSocket connections"
        })));
    }

    let (res, session, msg_stream) = actix_ws::handle(&req, stream)?;
    let rx = ws_tx.subscribe();

    // Get initial state to send to client
    let initial_state = state.read().await.clone();

    // Increment connection counter
    connection_count.fetch_add(1, Ordering::Relaxed);
    let counter = connection_count.into_inner().clone();

    tracing::info!(
        connections = counter.load(Ordering::Relaxed),
        "WebSocket client connected"
    );

    actix_rt::spawn(async move {
        ws_session(session, msg_stream, rx, initial_state).await;
        // Decrement on disconnect
        counter.fetch_sub(1, Ordering::Relaxed);
        tracing::info!(
            connections = counter.load(Ordering::Relaxed),
            "WebSocket client disconnected"
        );
    });

    Ok(res)
}

/// WebSocket session loop — forwards broadcast messages to client.
async fn ws_session(
    mut session: actix_ws::Session,
    msg_stream: actix_ws::MessageStream,
    mut rx: broadcast::Receiver<WsMessage>,
    initial_state: DashboardState,
) {
    let mut msg_stream = msg_stream
        .aggregate_continuations()
        .max_continuation_size(64 * 1024);

    let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(10));

    // Send initial state snapshot
    let initial_msg = WsMessage::StateUpdate {
        data: initial_state,
    };
    if let Ok(json) = serde_json::to_string(&initial_msg) {
        if session.text(json).await.is_err() {
            tracing::warn!("Failed to send initial state to WebSocket client");
            return;
        }
    }

    loop {
        tokio::select! {
            // Broadcast message from poller/webhook
            result = rx.recv() => {
                match result {
                    Ok(ws_msg) => {
                        if let Ok(json) = serde_json::to_string(&ws_msg) {
                            if session.text(json).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(lagged = n, "WebSocket client lagged behind");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::warn!("Broadcast channel closed");
                        break;
                    }
                }
            }
            // Client messages (ping/pong/close)
            Some(msg) = msg_stream.next() => {
                match msg {
                    Ok(AggregatedMessage::Ping(bytes)) => {
                        if session.pong(&bytes).await.is_err() {
                            break;
                        }
                    }
                    Ok(AggregatedMessage::Close(reason)) => {
                        let _ = session.close(reason).await;
                        break;
                    }
                    Ok(AggregatedMessage::Text(_text)) => {
                        // Could handle client-side filtering requests here
                    }
                    _ => {}
                }
            }
            // Heartbeat
            _ = heartbeat_interval.tick() => {
                if session.ping(b"").await.is_err() {
                    break;
                }
            }
        }
    }
}
