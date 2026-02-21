use actix_web::{web, HttpResponse};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::config::validate_repo_ref;
use crate::github::client::GitHubClient;
use crate::github::poller::PreviousRunTracker;
use crate::models::*;

/// GET /api/runs/active — all in_progress + queued runs across all repos.
pub async fn get_active_runs(
    state: web::Data<SharedDashboardState>,
) -> actix_web::Result<HttpResponse> {
    let dashboard = state.read().await;
    let active: Vec<&WorkflowRun> = dashboard
        .repos
        .iter()
        .flat_map(|r| r.active_runs.iter())
        .collect();
    Ok(HttpResponse::Ok().json(active))
}

/// GET /api/runs/{owner}/{repo} — runs for a specific repo.
pub async fn get_repo_runs(
    path: web::Path<(String, String)>,
    state: web::Data<SharedDashboardState>,
) -> actix_web::Result<HttpResponse> {
    let (owner, repo) = path.into_inner();
    let full_name = format!("{}/{}", owner, repo);

    if let Err(e) = validate_repo_ref(&full_name) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({"error": e})));
    }

    let dashboard = state.read().await;
    if let Some(repo_state) = dashboard.repos.iter().find(|r| r.repo == full_name) {
        let all_runs: Vec<&WorkflowRun> = repo_state
            .active_runs
            .iter()
            .chain(repo_state.recent_runs.iter())
            .collect();
        Ok(HttpResponse::Ok().json(all_runs))
    } else {
        Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("Repository {} is not being monitored", full_name)
        })))
    }
}

/// GET /api/runs/{owner}/{repo}/{run_id} — run detail with jobs and steps.
pub async fn get_run_detail(
    path: web::Path<(String, String, u64)>,
    client: web::Data<Arc<GitHubClient>>,
    state: web::Data<SharedDashboardState>,
) -> actix_web::Result<HttpResponse> {
    let (owner, repo, run_id) = path.into_inner();
    let full_name = format!("{}/{}", owner, repo);

    if let Err(e) = validate_repo_ref(&full_name) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({"error": e})));
    }

    // Get run from cached state or fetch
    let run = {
        let dashboard = state.read().await;
        dashboard
            .repos
            .iter()
            .find(|r| r.repo == full_name)
            .and_then(|r| {
                r.active_runs
                    .iter()
                    .chain(r.recent_runs.iter())
                    .find(|run| run.id == run_id)
                    .cloned()
            })
    };

    let run = match run {
        Some(r) => r,
        None => {
            // Not in cache — fetch from GitHub
            match client.get_run(&owner, &repo, run_id).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(run_id = run_id, error = %e, "Failed to fetch run");
                    return Ok(HttpResponse::NotFound().json(serde_json::json!({
                        "error": format!("Run {} not found or logs expired", run_id)
                    })));
                }
            }
        }
    };

    // Fetch jobs for this run
    let jobs = match client.list_jobs(&owner, &repo, run_id).await {
        Ok(j) => j,
        Err(e) => {
            tracing::error!(run_id = run_id, error = %e, "Failed to fetch jobs");
            vec![]
        }
    };

    Ok(HttpResponse::Ok().json(RunDetailResponse { run, jobs }))
}

/// POST /api/runs/{owner}/{repo}/{run_id}/cancel — cancel a running workflow.
pub async fn cancel_run(
    path: web::Path<(String, String, u64)>,
    client: web::Data<Arc<GitHubClient>>,
) -> actix_web::Result<HttpResponse> {
    let (owner, repo, run_id) = path.into_inner();

    if let Err(e) = validate_repo_ref(&format!("{}/{}", owner, repo)) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({"error": e})));
    }

    tracing::info!(
        repo = %format!("{}/{}", owner, repo),
        run_id = run_id,
        "Cancelling workflow run"
    );

    match client.cancel_run(&owner, &repo, run_id).await {
        Ok(()) => Ok(HttpResponse::Ok().json(ActionResponse {
            success: true,
            message: format!("Run {} cancelled", run_id),
        })),
        Err(e) => {
            tracing::error!(run_id = run_id, error = %e, "Failed to cancel run");
            Ok(HttpResponse::InternalServerError().json(ActionResponse {
                success: false,
                message: format!("Failed to cancel: {}", e),
            }))
        }
    }
}

/// POST /api/runs/{owner}/{repo}/{run_id}/rerun — re-run a workflow.
pub async fn rerun(
    path: web::Path<(String, String, u64)>,
    client: web::Data<Arc<GitHubClient>>,
) -> actix_web::Result<HttpResponse> {
    let (owner, repo, run_id) = path.into_inner();

    if let Err(e) = validate_repo_ref(&format!("{}/{}", owner, repo)) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({"error": e})));
    }

    tracing::info!(
        repo = %format!("{}/{}", owner, repo),
        run_id = run_id,
        "Re-running workflow"
    );

    match client.rerun(&owner, &repo, run_id).await {
        Ok(()) => Ok(HttpResponse::Ok().json(ActionResponse {
            success: true,
            message: format!("Run {} re-triggered", run_id),
        })),
        Err(e) => {
            tracing::error!(run_id = run_id, error = %e, "Failed to re-run");
            Ok(HttpResponse::InternalServerError().json(ActionResponse {
                success: false,
                message: format!("Failed to re-run: {}", e),
            }))
        }
    }
}

/// GET /api/runs/{owner}/{repo}/jobs/{job_id}/logs — fetch job logs inline.
pub async fn get_job_log(
    path: web::Path<(String, String, u64)>,
    client: web::Data<Arc<GitHubClient>>,
) -> Result<HttpResponse, crate::error::DashboardError> {
    let (owner, repo, job_id) = path.into_inner();
    let full_name = format!("{}/{}", owner, repo);

    if let Err(e) = validate_repo_ref(&full_name) {
        return Ok(HttpResponse::BadRequest().json(ActionResponse {
            success: false,
            message: e,
        }));
    }

    let log_response = client.get_job_log(&owner, &repo, job_id).await?;
    Ok(HttpResponse::Ok().json(log_response))
}

/// POST /api/refresh — manually trigger a full poll of all repos.
pub async fn manual_refresh(
    client: web::Data<Arc<GitHubClient>>,
    state: web::Data<SharedDashboardState>,
    repos: web::Data<SharedRepoList>,
    ws_tx: web::Data<broadcast::Sender<WsMessage>>,
    tracker: web::Data<PreviousRunTracker>,
) -> actix_web::Result<HttpResponse> {
    tracing::info!("Manual refresh triggered");

    crate::github::poller::poll_all_repos(
        &client,
        &state,
        &repos,
        &ws_tx,
        &tracker,
        false,
    )
    .await;

    let dashboard = state.read().await.clone();
    // Broadcast full state so all connected clients get the update
    let _ = ws_tx.send(WsMessage::StateUpdate {
        data: dashboard.clone(),
    });

    Ok(HttpResponse::Ok().json(ActionResponse {
        success: true,
        message: "Refresh complete".to_string(),
    }))
}
