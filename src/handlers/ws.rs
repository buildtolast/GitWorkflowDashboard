use actix_ws::AggregatedMessage;
use actix_web::{web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use std::time::Duration;
use tokio::sync::broadcast;

use crate::models::*;

/// GET /ws/live — WebSocket handler for real-time updates.
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    ws_tx: web::Data<broadcast::Sender<WsMessage>>,
    state: web::Data<SharedDashboardState>,
) -> actix_web::Result<HttpResponse> {
    let (res, session, msg_stream) = actix_ws::handle(&req, stream)?;
    let rx = ws_tx.subscribe();

    // Get initial state to send to client
    let initial_state = state.read().await.clone();

    tracing::info!("WebSocket client connected");

    actix_rt::spawn(ws_session(session, msg_stream, rx, initial_state));

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

    tracing::info!("WebSocket client disconnected");
}
