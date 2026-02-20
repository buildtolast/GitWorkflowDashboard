use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;

use crate::config::RepoRef;
use crate::models::*;

use super::client::GitHubClient;

/// Run the background poller that fetches workflow runs across all repos.
pub async fn run_poller(
    client: Arc<GitHubClient>,
    state: SharedDashboardState,
    repos: SharedRepoList,
    ws_tx: broadcast::Sender<WsMessage>,
    poll_interval: Duration,
    shutdown: CancellationToken,
) {
    tracing::info!(
        interval_secs = poll_interval.as_secs(),
        "Starting background poller"
    );

    let mut interval = tokio::time::interval(poll_interval);
    let mut is_first_poll = true;

    // Track previous run states for diffing
    let previous_runs: Arc<RwLock<std::collections::HashMap<u64, String>>> =
        Arc::new(RwLock::new(std::collections::HashMap::new()));

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                tracing::info!("Poller shutting down");
                break;
            }
            _ = interval.tick() => {
                poll_all_repos(
                    &client,
                    &state,
                    &repos,
                    &ws_tx,
                    &previous_runs,
                    is_first_poll,
                ).await;
                is_first_poll = false;
            }
        }
    }
}

/// Poll all monitored repos for workflow and run data.
async fn poll_all_repos(
    client: &Arc<GitHubClient>,
    state: &SharedDashboardState,
    repos: &SharedRepoList,
    ws_tx: &broadcast::Sender<WsMessage>,
    previous_runs: &Arc<RwLock<std::collections::HashMap<u64, String>>>,
    is_first_poll: bool,
) {
    let repo_list = repos.read().await.clone();

    if repo_list.is_empty() {
        tracing::debug!("No repos to poll");
        return;
    }

    // Check rate limit before polling
    let rate_limit = client.get_rate_limit().await;
    if rate_limit.remaining > 0 && rate_limit.remaining <= 10 {
        tracing::warn!(
            remaining = rate_limit.remaining,
            "Rate limit critically low — skipping poll cycle"
        );
        return;
    }

    tracing::debug!(repo_count = repo_list.len(), "Polling repos");

    let mut repo_states = Vec::new();
    let mut changed_runs = Vec::new();

    for repo_ref in &repo_list {
        match poll_single_repo(client, repo_ref).await {
            Ok(repo_state) => {
                // Diff runs against previous state
                let mut prev = previous_runs.write().await;
                for run in &repo_state.active_runs {
                    let status_key = format!(
                        "{}-{}",
                        run.status.as_deref().unwrap_or("unknown"),
                        run.conclusion.as_deref().unwrap_or("none")
                    );
                    let prev_status = prev.get(&run.id);
                    if prev_status.is_none_or(|s| s != &status_key) {
                        changed_runs.push((repo_ref.full_name(), run.clone()));
                        prev.insert(run.id, status_key);
                    }
                }
                for run in &repo_state.recent_runs {
                    let status_key = format!(
                        "{}-{}",
                        run.status.as_deref().unwrap_or("unknown"),
                        run.conclusion.as_deref().unwrap_or("none")
                    );
                    let prev_status = prev.get(&run.id);
                    if prev_status.is_none_or(|s| s != &status_key) {
                        changed_runs.push((repo_ref.full_name(), run.clone()));
                        prev.insert(run.id, status_key);
                    }
                }
                repo_states.push(repo_state);
            }
            Err(e) => {
                tracing::error!(
                    repo = %repo_ref.full_name(),
                    error = %e,
                    "Failed to poll repo"
                );
            }
        }
    }

    // Update shared dashboard state
    let updated_rate_limit = client.get_rate_limit().await;
    {
        let mut dashboard = state.write().await;
        dashboard.repos = repo_states;
        dashboard.rate_limit = updated_rate_limit.clone();
        dashboard.last_poll = Some(chrono::Utc::now());
    }

    // Broadcast changes
    if is_first_poll {
        // Send full state on first poll
        let dashboard = state.read().await.clone();
        let _ = ws_tx.send(WsMessage::StateUpdate { data: dashboard });
    } else {
        // Send individual run updates
        for (repo, run) in changed_runs {
            let _ = ws_tx.send(WsMessage::RunUpdate { repo, run });
        }
    }

    // Always broadcast rate limit update
    let _ = ws_tx.send(WsMessage::RateLimitUpdate {
        data: updated_rate_limit,
    });
}

/// Poll a single repository for workflows and runs.
async fn poll_single_repo(
    client: &Arc<GitHubClient>,
    repo_ref: &RepoRef,
) -> Result<RepoState, crate::error::DashboardError> {
    let owner = &repo_ref.owner;
    let repo = &repo_ref.repo;

    // Fetch workflows and runs concurrently
    let (workflows_result, active_runs_result, recent_runs_result) = tokio::join!(
        client.list_workflows(owner, repo),
        client.list_runs(owner, repo, Some("in_progress"), 20),
        client.list_runs(owner, repo, None, 20),
    );

    let workflows = workflows_result.unwrap_or_else(|e| {
        tracing::warn!(repo = %repo_ref, error = %e, "Failed to fetch workflows");
        vec![]
    });

    let active_runs = active_runs_result.unwrap_or_else(|e| {
        tracing::warn!(repo = %repo_ref, error = %e, "Failed to fetch active runs");
        vec![]
    });

    // Also fetch queued runs
    let queued_runs = client
        .list_runs(owner, repo, Some("queued"), 20)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(repo = %repo_ref, error = %e, "Failed to fetch queued runs");
            vec![]
        });

    // Merge active + queued (dedup by ID)
    let mut all_active: Vec<WorkflowRun> = active_runs;
    for run in queued_runs {
        if !all_active.iter().any(|r| r.id == run.id) {
            all_active.push(run);
        }
    }

    // Recent runs: completed ones from the general list
    let all_runs = recent_runs_result.unwrap_or_default();
    let recent_runs: Vec<WorkflowRun> = all_runs
        .into_iter()
        .filter(|r| r.status.as_deref() == Some("completed"))
        .take(10)
        .collect();

    Ok(RepoState {
        repo: repo_ref.full_name(),
        workflows,
        active_runs: all_active,
        recent_runs,
    })
}
