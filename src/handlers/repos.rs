use actix_web::{web, HttpResponse};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::config::{validate_repo_ref, RepoRef};
use crate::database::SharedDatabase;
use crate::models::*;

/// Query params for search.
#[derive(Debug, serde::Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

/// GET /api/repos — list all monitored repos.
pub async fn get_repos(repos: web::Data<SharedRepoList>) -> actix_web::Result<HttpResponse> {
    let repo_list = repos.read().await;
    let names: Vec<String> = repo_list.iter().map(|r| r.full_name()).collect();
    Ok(HttpResponse::Ok().json(ReposResponse { repos: names }))
}

/// POST /api/repos — add repos to monitoring.
pub async fn add_repos(
    body: web::Json<RepoManageRequest>,
    repos: web::Data<SharedRepoList>,
    state: web::Data<SharedDashboardState>,
    ws_tx: web::Data<broadcast::Sender<WsMessage>>,
    client: web::Data<Arc<crate::github::client::GitHubClient>>,
    max_repos: web::Data<usize>,
    db: web::Data<SharedDatabase>,
) -> actix_web::Result<HttpResponse> {
    let new_repos = &body.repos;

    // Validate all repo refs
    let mut parsed: Vec<RepoRef> = Vec::new();
    for repo_str in new_repos {
        match validate_repo_ref(repo_str) {
            Ok(r) => parsed.push(r),
            Err(e) => {
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "error": e
                })));
            }
        }
    }

    // Add to list (dedup)
    let mut repo_list = repos.write().await;

    // Check max repos cap
    let max = **max_repos;
    if repo_list.len() + parsed.len() > max {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Cannot exceed {} monitored repos (currently {}, adding {})", max, repo_list.len(), parsed.len())
        })));
    }
    for repo_ref in &parsed {
        if !repo_list.iter().any(|r| r.full_name() == repo_ref.full_name()) {
            repo_list.push(repo_ref.clone());
            tracing::info!(repo = %repo_ref, "Added repo to monitoring");
        }
    }

    let all_repos_snapshot: Vec<RepoRef> = repo_list.clone();
    let all_names: Vec<String> = repo_list.iter().map(|r| r.full_name()).collect();
    drop(repo_list);

    // Persist to database
    if let Some(ref database) = ***db {
        database.save_repos(&all_repos_snapshot);
    }

    // Broadcast repos changed
    let _ = ws_tx.send(WsMessage::ReposChanged {
        repos: all_names.clone(),
    });

    // Trigger immediate poll for newly added repos
    let client = client.into_inner();
    for repo_ref in &parsed {
        let owner = repo_ref.owner.clone();
        let repo = repo_ref.repo.clone();
        let client = client.clone();
        let state = state.clone();
        let ws_tx = ws_tx.clone();
        let repo_name = repo_ref.full_name();

        tokio::spawn(async move {
            tracing::info!(repo = %repo_name, "Immediate poll for new repo");
            let workflows = client.list_workflows(&owner, &repo).await.unwrap_or_default();
            let active = client
                .list_runs(&owner, &repo, Some("in_progress"), 20)
                .await
                .unwrap_or_default();
            let queued = client
                .list_runs(&owner, &repo, Some("queued"), 20)
                .await
                .unwrap_or_default();
            let recent_all = client.list_runs(&owner, &repo, None, 20).await.unwrap_or_default();

            let mut all_active = active;
            for run in queued {
                if !all_active.iter().any(|r| r.id == run.id) {
                    all_active.push(run);
                }
            }

            let recent_runs: Vec<WorkflowRun> = recent_all
                .into_iter()
                .filter(|r| r.status.as_deref() == Some("completed"))
                .take(10)
                .collect();

            let repo_state = RepoState {
                repo: repo_name.clone(),
                workflows,
                active_runs: all_active,
                recent_runs,
            };

            // Add to dashboard state
            {
                let mut dashboard = state.write().await;
                dashboard.repos.retain(|r| r.repo != repo_name);
                dashboard.repos.push(repo_state);
            }

            // Broadcast full state update
            let dashboard = state.read().await.clone();
            let _ = ws_tx.send(WsMessage::StateUpdate { data: dashboard });
        });
    }

    Ok(HttpResponse::Ok().json(ReposResponse { repos: all_names }))
}

/// DELETE /api/repos — remove repos from monitoring.
pub async fn remove_repos(
    body: web::Json<RepoManageRequest>,
    repos: web::Data<SharedRepoList>,
    state: web::Data<SharedDashboardState>,
    ws_tx: web::Data<broadcast::Sender<WsMessage>>,
    db: web::Data<SharedDatabase>,
) -> actix_web::Result<HttpResponse> {
    let to_remove = &body.repos;

    let mut repo_list = repos.write().await;
    repo_list.retain(|r| !to_remove.contains(&r.full_name()));

    let remaining_repos: Vec<RepoRef> = repo_list.clone();
    let remaining: Vec<String> = repo_list.iter().map(|r| r.full_name()).collect();
    drop(repo_list);

    // Persist to database
    if let Some(ref database) = ***db {
        database.save_repos(&remaining_repos);
    }

    // Remove from dashboard state
    {
        let mut dashboard = state.write().await;
        dashboard.repos.retain(|r| !to_remove.contains(&r.repo));
    }

    // Broadcast
    let _ = ws_tx.send(WsMessage::ReposChanged {
        repos: remaining.clone(),
    });

    for repo in to_remove {
        tracing::info!(repo = %repo, "Removed repo from monitoring");
    }

    Ok(HttpResponse::Ok().json(ReposResponse { repos: remaining }))
}

/// GET /api/repos/search?q={query} — search GitHub repos by name/org/topic.
pub async fn search_repos(
    query: web::Query<SearchQuery>,
    repos: web::Data<SharedRepoList>,
    client: web::Data<Arc<crate::github::client::GitHubClient>>,
) -> actix_web::Result<HttpResponse> {
    let q = query.q.trim();
    if q.is_empty() {
        return Ok(HttpResponse::Ok().json(SearchReposResponse {
            total_count: 0,
            results: vec![],
        }));
    }

    let (total_count, items) = client
        .search_repos(q, 20)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // Filter out repos already being monitored
    let monitored: Vec<String> = {
        let repo_list = repos.read().await;
        repo_list.iter().map(|r| r.full_name()).collect()
    };

    let results: Vec<SearchRepoItem> = items
        .into_iter()
        .filter(|item| !monitored.contains(&item.full_name))
        .collect();

    Ok(HttpResponse::Ok().json(SearchReposResponse {
        total_count,
        results,
    }))
}
