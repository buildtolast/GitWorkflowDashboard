use actix_web::{web, HttpResponse};
use std::sync::Arc;

use crate::config::validate_repo_ref;
use crate::github::client::GitHubClient;
use crate::models::*;

/// GET /api/workflows — all workflow definitions across all monitored repos.
pub async fn get_all_workflows(
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
pub async fn get_workflows(
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
pub async fn dispatch_workflow(
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

    let dispatch_body = GitHubDispatchBody {
        git_ref: req.git_ref.clone(),
        inputs: req.inputs,
    };

    tracing::info!(
        repo = %req.repo,
        workflow_id = req.workflow_id,
        git_ref = %req.git_ref,
        "Dispatching workflow"
    );

    match client
        .dispatch_workflow(&repo_ref.owner, &repo_ref.repo, req.workflow_id, &dispatch_body)
        .await
    {
        Ok(()) => Ok(HttpResponse::Ok().json(ActionResponse {
            success: true,
            message: format!(
                "Workflow {} dispatched on {}",
                req.workflow_id, req.git_ref
            ),
        })),
        Err(e) => {
            tracing::error!(error = %e, "Failed to dispatch workflow");
            Ok(HttpResponse::InternalServerError().json(ActionResponse {
                success: false,
                message: format!("Dispatch failed: {}", e),
            }))
        }
    }
}

/// GET /api/branches/{owner}/{repo} — list branches for trigger dialog.
pub async fn get_branches(
    path: web::Path<(String, String)>,
    client: web::Data<Arc<GitHubClient>>,
) -> actix_web::Result<HttpResponse> {
    let (owner, repo) = path.into_inner();

    if let Err(e) = validate_repo_ref(&format!("{}/{}", owner, repo)) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({"error": e})));
    }

    match client.list_branches(&owner, &repo).await {
        Ok(branches) => Ok(HttpResponse::Ok().json(branches)),
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
