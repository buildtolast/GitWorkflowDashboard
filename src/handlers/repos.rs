use actix_web::{web, HttpResponse};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::config::{validate_repo_ref, RepoRef};
use crate::database::SharedDatabase;
use crate::models::*;

/// Query params for search.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct SearchQuery {
    pub q: String,
}

/// GET /api/repos — list all monitored repos.
pub(crate) async fn get_repos(repos: web::Data<SharedRepoList>) -> actix_web::Result<HttpResponse> {
    let repo_list = repos.read().await;
    let names: Vec<String> = repo_list.iter().map(|r| r.full_name()).collect();
    Ok(HttpResponse::Ok().json(ReposResponse { repos: names }))
}

/// POST /api/repos — add repos to monitoring.
pub(crate) async fn add_repos(
    body: web::Json<RepoManageRequest>,
    repos: web::Data<SharedRepoList>,
    state: web::Data<SharedDashboardState>,
    ws_tx: web::Data<broadcast::Sender<Arc<String>>>,
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
    {
        let db = db.clone();
        let repos = all_repos_snapshot.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(ref database) = ***db {
                database.save_repos(&repos);
            }
        });
    }

    // Broadcast repos changed
    if let Ok(json) = serde_json::to_string(&WsMessage::ReposChanged {
        repos: all_names.clone(),
    }) {
        let _ = ws_tx.send(Arc::new(json));
    }

    // Trigger immediate poll for newly added repos
    let client = client.into_inner();
    for repo_ref in &parsed {
        let client = client.clone();
        let state = state.clone();
        let ws_tx = ws_tx.clone();
        let repo_ref = repo_ref.clone();

        tokio::spawn(async move {
            let repo_name = repo_ref.full_name();
            tracing::info!(repo = %repo_name, "Immediate poll for new repo");

            match crate::github::poller::poll_single_repo(&client, &repo_ref).await {
                Ok(repo_state) => {
                    // Add to dashboard state
                    {
                        let mut dashboard = state.write().await;
                        dashboard.repos.retain(|r| r.repo != repo_name);
                        dashboard.repos.push(repo_state);
                    }

                    // Broadcast full state update
                    let dashboard = state.read().await.clone();
                    if let Ok(json) = serde_json::to_string(&WsMessage::StateUpdate { data: dashboard }) {
                        let _ = ws_tx.send(Arc::new(json));
                    }
                }
                Err(e) => {
                    tracing::error!(repo = %repo_name, error = %e, "Failed to poll new repo");
                }
            }
        });
    }

    Ok(HttpResponse::Ok().json(ReposResponse { repos: all_names }))
}

/// DELETE /api/repos — remove repos from monitoring.
pub(crate) async fn remove_repos(
    body: web::Json<RepoManageRequest>,
    repos: web::Data<SharedRepoList>,
    state: web::Data<SharedDashboardState>,
    ws_tx: web::Data<broadcast::Sender<Arc<String>>>,
    db: web::Data<SharedDatabase>,
) -> actix_web::Result<HttpResponse> {
    let to_remove = &body.repos;

    let mut repo_list = repos.write().await;
    repo_list.retain(|r| !to_remove.contains(&r.full_name()));

    let remaining_repos: Vec<RepoRef> = repo_list.clone();
    let remaining: Vec<String> = repo_list.iter().map(|r| r.full_name()).collect();
    drop(repo_list);

    // Persist to database
    {
        let db = db.clone();
        let repos = remaining_repos.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(ref database) = ***db {
                database.save_repos(&repos);
            }
        });
    }

    // Remove from dashboard state
    {
        let mut dashboard = state.write().await;
        dashboard.repos.retain(|r| !to_remove.contains(&r.repo));
    }

    // Broadcast
    if let Ok(json) = serde_json::to_string(&WsMessage::ReposChanged {
        repos: remaining.clone(),
    }) {
        let _ = ws_tx.send(Arc::new(json));
    }

    for repo in to_remove {
        tracing::info!(repo = %repo, "Removed repo from monitoring");
    }

    Ok(HttpResponse::Ok().json(ReposResponse { repos: remaining }))
}

/// GET /api/repos/search?q={query} — search GitHub repos by name/org/topic.
pub(crate) async fn search_repos(
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
