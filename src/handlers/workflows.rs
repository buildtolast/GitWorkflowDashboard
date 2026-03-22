use actix_web::{web, HttpResponse};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::sync::Arc;
use std::time::Instant;

use crate::config::{validate_branch_name, validate_repo_ref};
use crate::error::DashboardError;
use crate::github::client::GitHubClient;
use crate::models::*;

/// In-memory cache of recent dispatch idempotency keys with timestamps.
static DISPATCH_CACHE: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Window (in seconds) during which duplicate dispatch keys are rejected.
const DISPATCH_DEDUP_WINDOW_SECS: u64 = 30;

/// GET /api/workflows — all workflow definitions across all monitored repos.
pub(crate) async fn get_all_workflows(
    state: web::Data<SharedDashboardState>,
) -> actix_web::Result<HttpResponse> {
    let dashboard = state.read().await;
    let all_workflows: Vec<&Workflow> = dashboard
        .repos
        .iter()
        .flat_map(|r| r.workflows.iter())
        .collect();
    Ok(HttpResponse::Ok().json(all_workflows))
}

/// GET /api/workflows/{owner}/{repo} — workflows for a specific repo.
pub(crate) async fn get_workflows(
    path: web::Path<(String, String)>,
    state: web::Data<SharedDashboardState>,
    client: web::Data<Arc<GitHubClient>>,
) -> actix_web::Result<HttpResponse> {
    let (owner, repo) = path.into_inner();

    if let Err(e) = validate_repo_ref(&format!("{}/{}", owner, repo)) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({"error": e})));
    }

    let full_name = format!("{}/{}", owner, repo);

    // Try from cache first
    {
        let dashboard = state.read().await;
        if let Some(repo_state) = dashboard.repos.iter().find(|r| r.repo == full_name) {
            return Ok(HttpResponse::Ok().json(&repo_state.workflows));
        }
    }

    // Not cached — fetch from GitHub
    match client.list_workflows(&owner, &repo).await {
        Ok(workflows) => Ok(HttpResponse::Ok().json(workflows)),
        Err(e) => {
            tracing::error!(repo = %full_name, error = %e, "Failed to fetch workflows");
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch workflows: {}", e)
            })))
        }
    }
}

/// POST /api/workflows/dispatch — trigger a workflow dispatch.
pub(crate) async fn dispatch_workflow(
    body: web::Json<DispatchRequest>,
    client: web::Data<Arc<GitHubClient>>,
) -> actix_web::Result<HttpResponse> {
    let req = body.into_inner();

    // Validate repo ref
    let repo_ref = match validate_repo_ref(&req.repo) {
        Ok(r) => r,
        Err(e) => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({"error": e})));
        }
    };

    // Validate ref (branch/tag)
    if req.git_ref.trim().is_empty() {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "ref (branch or tag) must not be empty"
        })));
    }
    if let Err(e) = validate_branch_name(&req.git_ref) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({"error": e})));
    }

    // ─── Idempotency check ───
    if let Some(ref key) = req.idempotency_key {
        let mut cache = DISPATCH_CACHE.lock().unwrap();
        let now = Instant::now();
        let window = std::time::Duration::from_secs(DISPATCH_DEDUP_WINDOW_SECS);

        // Purge expired entries
        cache.retain(|_, ts| now.duration_since(*ts) < window);

        if let Some(prev) = cache.get(key)
            && now.duration_since(*prev) < window
        {
            tracing::info!(
                repo = %req.repo,
                workflow_id = req.workflow_id.0,
                idempotency_key = %key,
                "Duplicate dispatch suppressed"
            );
            return Ok(HttpResponse::Ok().json(ActionResponse {
                success: true,
                message: "Already dispatched (duplicate idempotency key)".to_string(),
            }));
        }
    }

    let dispatch_body = GitHubDispatchBody {
        git_ref: req.git_ref.clone(),
        inputs: req.inputs,
    };

    tracing::info!(
        repo = %req.repo,
        workflow_id = req.workflow_id.0,
        git_ref = %req.git_ref,
        "Dispatching workflow"
    );

    match client
        .dispatch_workflow(&repo_ref.owner, &repo_ref.repo, req.workflow_id, &dispatch_body)
        .await
    {
        Ok(()) => {
            // Cache the idempotency key on success
            if let Some(ref key) = req.idempotency_key {
                let mut cache = DISPATCH_CACHE.lock().unwrap();
                cache.insert(key.clone(), Instant::now());
            }
            Ok(HttpResponse::Ok().json(ActionResponse {
                success: true,
                message: format!(
                    "Workflow {} dispatched on {}",
                    req.workflow_id, req.git_ref
                ),
            }))
        }
        Err(ref e) if matches!(e, DashboardError::Http(he) if he.is_timeout()) => {
            let timeout_err = DashboardError::Timeout {
                operation: format!("dispatch workflow {}", req.workflow_id),
            };
            tracing::warn!(error = %timeout_err, "Dispatch request timed out — may have succeeded");
            Err(timeout_err.into())
        }
        Err(e) => {
            tracing::error!(
                repo = %req.repo,
                workflow_id = req.workflow_id.0,
                git_ref = %req.git_ref,
                error = %e,
                "Failed to dispatch workflow"
            );
            Ok(HttpResponse::InternalServerError().json(ActionResponse {
                success: false,
                message: format!("Dispatch failed: {}", e),
            }))
        }
    }
}

/// GET /api/workflows/{owner}/{repo}/{workflow_id}/inputs — get workflow_dispatch inputs.
pub(crate) async fn get_workflow_inputs(
    path: web::Path<(String, String, u64)>,
    client: web::Data<Arc<GitHubClient>>,
    state: web::Data<SharedDashboardState>,
) -> actix_web::Result<HttpResponse> {
    let (owner, repo, workflow_id) = path.into_inner();
    let full_name = format!("{}/{}", owner, repo);

    if let Err(e) = validate_repo_ref(&full_name) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({"error": e})));
    }

    // Find workflow path from cached state
    let workflow_path = {
        let dashboard = state.read().await;
        dashboard
            .repos
            .iter()
            .find(|r| r.repo == full_name)
            .and_then(|r| r.workflows.iter().find(|w| w.id == WorkflowId(workflow_id)))
            .map(|w| w.path.clone())
    };

    let workflow_path = match workflow_path {
        Some(p) => p,
        None => {
            return Ok(HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Workflow {} not found in {}", workflow_id, full_name)
            })));
        }
    };

    match client
        .get_workflow_inputs(&owner, &repo, &workflow_path)
        .await
    {
        Ok((has_dispatch, inputs)) => {
            Ok(HttpResponse::Ok().json(WorkflowInputsResponse {
                workflow_id: WorkflowId(workflow_id),
                has_dispatch,
                inputs,
            }))
        }
        Err(e) => {
            tracing::error!(
                repo = %full_name,
                workflow_id = workflow_id,
                error = %e,
                "Failed to fetch workflow inputs"
            );
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch workflow inputs: {}", e)
            })))
        }
    }
}

/// GET /api/branches/{owner}/{repo} — list branches for trigger dialog.
pub(crate) async fn get_branches(
    path: web::Path<(String, String)>,
    client: web::Data<Arc<GitHubClient>>,
) -> actix_web::Result<HttpResponse> {
    let (owner, repo) = path.into_inner();

    if let Err(e) = validate_repo_ref(&format!("{}/{}", owner, repo)) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({"error": e})));
    }

    match client.list_branches_sorted(&owner, &repo).await {
        Ok(sorted_names) => {
            // Return as objects with "name" field for frontend compatibility
            let branches: Vec<serde_json::Value> = sorted_names
                .into_iter()
                .map(|name| serde_json::json!({"name": name, "protected": false}))
                .collect();
            Ok(HttpResponse::Ok().json(branches))
        }
        Err(e) => {
            tracing::error!(
                repo = %format!("{}/{}", owner, repo),
                error = %e,
                "Failed to fetch branches"
            );
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch branches: {}", e)
            })))
        }
    }
}
