use actix_web::{web, HttpRequest, HttpResponse};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::models::*;

type HmacSha256 = Hmac<Sha256>;

/// Handle incoming GitHub webhook events.
pub(crate) async fn webhook_handler(
    req: HttpRequest,
    body: web::Bytes,
    secret: web::Data<Option<String>>,
    state: web::Data<SharedDashboardState>,
    ws_tx: web::Data<tokio::sync::broadcast::Sender<WsMessage>>,
) -> actix_web::Result<HttpResponse> {
    // 1. Verify HMAC signature if webhook secret is configured
    if let Some(ref webhook_secret) = **secret {
        let signature = req
            .headers()
            .get("X-Hub-Signature-256")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if !verify_signature(webhook_secret, &body, signature) {
            tracing::warn!("Webhook signature verification failed");
            return Ok(HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "Invalid webhook signature"
            })));
        }
    }

    // 2. Parse event type
    let event_type = req
        .headers()
        .get("X-GitHub-Event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    tracing::info!(event = %event_type, "Received webhook event");

    match event_type {
        "workflow_run" => handle_workflow_run_event(&body, &state, &ws_tx).await,
        "workflow_job" => handle_workflow_job_event(&body).await,
        "ping" => {
            tracing::info!("Received webhook ping");
            Ok(HttpResponse::Ok().json(serde_json::json!({"status": "pong"})))
        }
        _ => {
            tracing::debug!(event = %event_type, "Ignoring unhandled webhook event");
            Ok(HttpResponse::Ok().json(serde_json::json!({"status": "ignored"})))
        }
    }
}

/// Handle workflow_run webhook events.
async fn handle_workflow_run_event(
    body: &web::Bytes,
    state: &web::Data<SharedDashboardState>,
    ws_tx: &web::Data<tokio::sync::broadcast::Sender<WsMessage>>,
) -> actix_web::Result<HttpResponse> {
    #[derive(serde::Deserialize)]
    struct WorkflowRunEvent {
        action: String,
        workflow_run: WorkflowRun,
        repository: Option<RepoInfo>,
    }

    #[derive(serde::Deserialize)]
    struct RepoInfo {
        full_name: String,
    }

    let event: WorkflowRunEvent = match serde_json::from_slice(body) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!(error = %e, "Failed to parse workflow_run event");
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Invalid event payload: {}", e)
            })));
        }
    };

    let repo_name = event
        .repository
        .map(|r| r.full_name)
        .unwrap_or_else(|| event.workflow_run.repo.clone());

    tracing::info!(
        action = %event.action,
        repo = %repo_name,
        run_id = event.workflow_run.id.0,
        status = ?event.workflow_run.status,
        "Workflow run event"
    );

    // Update shared state
    let mut run = event.workflow_run;
    run.repo = repo_name.clone();

    {
        let mut dashboard = state.write().await;
        if let Some(repo_state) = dashboard.repos.iter_mut().find(|r| r.repo == repo_name) {
            match event.action.as_str() {
                "requested" | "in_progress" => {
                    // Add or update in active_runs
                    if let Some(existing) = repo_state.active_runs.iter_mut().find(|r| r.id == run.id) {
                        *existing = run.clone();
                    } else {
                        repo_state.active_runs.push(run.clone());
                    }
                }
                "completed" => {
                    // Move from active to recent
                    repo_state.active_runs.retain(|r| r.id != run.id);
                    repo_state.recent_runs.insert(0, run.clone());
                    repo_state.recent_runs.truncate(10);
                }
                _ => {}
            }
        }
    }

    // Broadcast to WebSocket clients
    let _ = ws_tx.send(WsMessage::RunUpdate {
        repo: repo_name,
        run,
    });

    Ok(HttpResponse::Ok().json(serde_json::json!({"status": "processed"})))
}

/// Handle workflow_job webhook events (logged but not yet surfaced in state).
async fn handle_workflow_job_event(body: &web::Bytes) -> actix_web::Result<HttpResponse> {
    #[derive(serde::Deserialize)]
    struct WorkflowJobEvent {
        action: String,
        workflow_job: JobInfo,
    }

    #[derive(serde::Deserialize)]
    struct JobInfo {
        id: u64,
        run_id: u64,
        name: String,
        status: String,
    }

    match serde_json::from_slice::<WorkflowJobEvent>(body) {
        Ok(event) => {
            tracing::info!(
                action = %event.action,
                job_id = event.workflow_job.id,
                run_id = event.workflow_job.run_id,
                job_name = %event.workflow_job.name,
                status = %event.workflow_job.status,
                "Workflow job event"
            );
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to parse workflow_job event");
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({"status": "processed"})))
}

/// Verify GitHub webhook HMAC-SHA256 signature.
fn verify_signature(secret: &str, body: &[u8], signature: &str) -> bool {
    let expected_prefix = "sha256=";
    if !signature.starts_with(expected_prefix) {
        return false;
    }
    let sig_hex = &signature[expected_prefix.len()..];
    let sig_bytes = match hex::decode(sig_hex) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(body);

    mac.verify_slice(&sig_bytes).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compute_signature(secret: &str, body: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let result = mac.finalize();
        format!("sha256={}", hex::encode(result.into_bytes()))
    }

    #[test]
    fn test_verify_signature_valid() {
        let secret = "test-secret";
        let body = b"test payload";
        let sig = compute_signature(secret, body);
        assert!(verify_signature(secret, body, &sig));
    }

    #[test]
    fn test_verify_signature_invalid() {
        assert!(!verify_signature("secret", b"body", "sha256=invalidhex"));
    }

    #[test]
    fn test_verify_signature_wrong_prefix() {
        assert!(!verify_signature("secret", b"body", "sha1=abc"));
    }

    #[test]
    fn test_verify_signature_empty_signature() {
        assert!(!verify_signature("secret", b"body", ""));
    }

    #[test]
    fn test_verify_signature_wrong_secret() {
        let body = b"test payload";
        let sig = compute_signature("correct-secret", body);
        assert!(!verify_signature("wrong-secret", body, &sig));
    }

    #[test]
    fn test_verify_signature_different_body() {
        let body = b"original payload";
        let sig = compute_signature("secret", body);
        assert!(!verify_signature("secret", b"tampered payload", &sig));
    }

    #[test]
    fn test_verify_signature_empty_body() {
        let body = b"";
        let sig = compute_signature("secret", body);
        assert!(verify_signature("secret", body, &sig));
    }

    #[test]
    fn test_verify_signature_large_body() {
        let body = vec![0u8; 10_000];
        let sig = compute_signature("secret", &body);
        assert!(verify_signature("secret", &body, &sig));
    }

    #[test]
    fn test_verify_signature_non_hex_chars() {
        assert!(!verify_signature("secret", b"body", "sha256=xyz_not_hex!"));
    }

    #[test]
    fn test_verify_signature_truncated_hex() {
        // Valid hex but wrong length
        assert!(!verify_signature("secret", b"body", "sha256=abcdef"));
    }
}
