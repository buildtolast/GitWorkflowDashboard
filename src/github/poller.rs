use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;

use crate::config::RepoRef;
use crate::database::SharedDatabase;
use crate::models::*;

use super::client::GitHubClient;

/// Shared tracker for previous run states (used for diffing).
pub(crate) type PreviousRunTracker = Arc<RwLock<std::collections::HashMap<RunId, String>>>;

/// Create a new empty run tracker.
pub(crate) fn new_run_tracker() -> PreviousRunTracker {
    Arc::new(RwLock::new(std::collections::HashMap::new()))
}

// ═══════════════════════════════════════════════════════════════
//   ADAPTIVE BACKOFF CONFIGURATION
// ═══════════════════════════════════════════════════════════════

/// Rate limit threshold below which backoff activates.
const BACKOFF_THRESHOLD: u32 = 100;

/// Rate limit threshold below which we skip polls entirely.
const CRITICAL_THRESHOLD: u32 = 10;

/// Maximum backoff multiplier (2^n cap) — limits backoff to 5 minutes.
const MAX_BACKOFF_POWER: u32 = 5;

/// Maximum backoff duration in seconds (5 minutes).
const MAX_BACKOFF_SECS: u64 = 300;

/// Run the background poller that fetches workflow runs across all repos.
///
/// Uses adaptive exponential backoff when rate limits are low:
/// - `remaining > BACKOFF_THRESHOLD` → normal poll interval
/// - `CRITICAL_THRESHOLD < remaining <= BACKOFF_THRESHOLD` → `base × 2^n + jitter`
/// - `remaining <= CRITICAL_THRESHOLD` → skip poll entirely
///
/// Backoff resets automatically when rate limit recovers.
pub(crate) async fn run_poller(
    client: Arc<GitHubClient>,
    state: SharedDashboardState,
    repos: SharedRepoList,
    ws_tx: broadcast::Sender<Arc<String>>,
    poll_interval: Duration,
    shutdown: CancellationToken,
    previous_runs: PreviousRunTracker,
    db: SharedDatabase,
) {
    tracing::info!(
        interval_secs = poll_interval.as_secs(),
        backoff_threshold = BACKOFF_THRESHOLD,
        critical_threshold = CRITICAL_THRESHOLD,
        "Starting background poller with adaptive backoff"
    );

    let base_interval = poll_interval;
    let mut consecutive_low = 0u32;
    let mut is_first_poll = true;

    loop {
        // Calculate next sleep duration based on backoff state
        let sleep_duration = compute_backoff_interval(
            base_interval,
            consecutive_low,
        );

        tokio::select! {
            _ = shutdown.cancelled() => {
                tracing::info!("Poller shutting down");
                break;
            }
            _ = tokio::time::sleep(sleep_duration) => {
                // Check rate limit before deciding whether to poll
                let rate_limit = client.get_rate_limit().await;

                if rate_limit.remaining > 0 && rate_limit.remaining <= CRITICAL_THRESHOLD {
                    // Critical — skip poll entirely
                    consecutive_low += 1;
                    tracing::warn!(
                        remaining = rate_limit.remaining,
                        consecutive_low,
                        "Rate limit critically low — skipping poll cycle"
                    );
                    continue;
                }

                if rate_limit.remaining > 0 && rate_limit.remaining <= BACKOFF_THRESHOLD {
                    // Low but not critical — poll but increment backoff
                    consecutive_low += 1;
                    let next_backoff = compute_backoff_interval(base_interval, consecutive_low);
                    tracing::info!(
                        remaining = rate_limit.remaining,
                        consecutive_low,
                        next_interval_secs = next_backoff.as_secs(),
                        "Rate limit low — backing off"
                    );
                } else {
                    // Healthy — reset backoff counter
                    if consecutive_low > 0 {
                        tracing::info!(
                            remaining = rate_limit.remaining,
                            was_backed_off = consecutive_low,
                            "Rate limit recovered — resuming normal polling"
                        );
                    }
                    consecutive_low = 0;
                }

                poll_all_repos(
                    &client,
                    &state,
                    &repos,
                    &ws_tx,
                    &previous_runs,
                    is_first_poll,
                    &db,
                ).await;
                is_first_poll = false;
            }
        }
    }
}

/// Compute the poll interval with exponential backoff and jitter.
///
/// Formula: `min(base × 2^n + jitter, MAX_BACKOFF_SECS)`
/// where `jitter ∈ [0, base)` and `n` is capped at `MAX_BACKOFF_POWER`.
fn compute_backoff_interval(base: Duration, consecutive_low: u32) -> Duration {
    if consecutive_low == 0 {
        return base;
    }

    let power = consecutive_low.min(MAX_BACKOFF_POWER);
    let multiplier = 2u64.pow(power);
    let base_secs = base.as_secs();
    let backed_off = base_secs.saturating_mul(multiplier);

    // Add random jitter in [0, base_secs) to prevent thundering herd
    let jitter = if base_secs > 0 {
        rand::thread_rng().gen_range(0..base_secs)
    } else {
        0
    };

    let total = backed_off.saturating_add(jitter).min(MAX_BACKOFF_SECS);

    Duration::from_secs(total)
}

/// Poll all monitored repos for workflow and run data.
pub(crate) async fn poll_all_repos(
    client: &Arc<GitHubClient>,
    state: &SharedDashboardState,
    repos: &SharedRepoList,
    ws_tx: &broadcast::Sender<Arc<String>>,
    previous_runs: &Arc<RwLock<std::collections::HashMap<RunId, String>>>,
    is_first_poll: bool,
    db: &SharedDatabase,
) {
    let repo_list = repos.read().await.clone();

    if repo_list.is_empty() {
        tracing::debug!("No repos to poll");
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
                    if prev_status.map_or(true, |s| s != &status_key) {
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
                    if prev_status.map_or(true, |s| s != &status_key) {
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

    // Persist runs to SQLite database
    {
        let db = db.clone();
        let repo_states_for_db: Vec<_> = repo_states.iter().map(|rs| {
            let all_runs: Vec<WorkflowRun> = rs.active_runs.iter()
                .chain(rs.recent_runs.iter())
                .cloned()
                .collect();
            (rs.repo.clone(), all_runs)
        }).collect();
        tokio::task::spawn_blocking(move || {
            if let Some(ref database) = *db {
                for (repo, runs) in &repo_states_for_db {
                    let parts: Vec<&str> = repo.splitn(2, '/').collect();
                    if parts.len() == 2 && !runs.is_empty() {
                        database.upsert_runs(parts[0], parts[1], runs);
                    }
                }
            }
        });
    }

    // Update shared dashboard state
    let updated_rate_limit = client.get_rate_limit().await;
    {
        let mut dashboard = state.write().await;
        dashboard.repos = repo_states;
        dashboard.rate_limit = updated_rate_limit.clone();
        dashboard.last_poll = Some(chrono::Utc::now());
    }

    // Always broadcast full state so all clients stay in sync
    // (workflows, new repos, removed repos, etc. all reflected).
    // Individual run_update messages are also sent for changed runs
    // so clients can animate transitions without a full re-render.
    let dashboard = state.read().await.clone();
    if let Ok(json) = serde_json::to_string(&WsMessage::StateUpdate { data: dashboard }) {
        let _ = ws_tx.send(Arc::new(json));
    }

    if !is_first_poll {
        // Also send granular run updates for clients that want them
        for (repo, run) in changed_runs {
            if let Ok(json) = serde_json::to_string(&WsMessage::RunUpdate { repo, run }) {
                let _ = ws_tx.send(Arc::new(json));
            }
        }
    }

    // Always broadcast rate limit update
    if let Ok(json) = serde_json::to_string(&WsMessage::RateLimitUpdate { data: updated_rate_limit }) {
        let _ = ws_tx.send(Arc::new(json));
    }

    // Evict stale run tracking entries
    {
        let dashboard = state.read().await;
        let current_ids: std::collections::HashSet<_> = dashboard.repos.iter()
            .flat_map(|rs| rs.active_runs.iter().chain(rs.recent_runs.iter()))
            .map(|r| r.id)
            .collect();
        let mut prev = previous_runs.write().await;
        prev.retain(|id, _| current_ids.contains(id));
    }
}

/// Poll a single repository for workflows and runs.
pub(crate) async fn poll_single_repo(
    client: &Arc<GitHubClient>,
    repo_ref: &RepoRef,
) -> Result<RepoState, crate::error::DashboardError> {
    let owner = &repo_ref.owner;
    let repo = &repo_ref.repo;

    // Fetch workflows + one combined run list concurrently (2 API calls instead of 3+)
    let (workflows_result, all_runs_result) = tokio::join!(
        client.list_workflows(owner, repo),
        client.list_runs(owner, repo, None, 50),
    );

    let workflows = workflows_result.unwrap_or_else(|e| {
        tracing::warn!(repo = %repo_ref, error = %e, "Failed to fetch workflows");
        vec![]
    });

    let all_runs = all_runs_result.unwrap_or_else(|e| {
        tracing::warn!(repo = %repo_ref, error = %e, "Failed to fetch runs");
        vec![]
    });

    // Filter client-side: active = in_progress/queued/waiting/pending
    let all_active: Vec<_> = all_runs.iter()
        .filter(|r| matches!(r.status.as_deref(), Some("in_progress") | Some("queued") | Some("waiting") | Some("pending")))
        .cloned()
        .collect();

    // Recent runs: completed ones from the combined list
    let recent_runs: Vec<_> = all_runs.into_iter()
        .filter(|r| matches!(r.status.as_deref(), Some("completed")))
        .take(20)
        .collect();

    Ok(RepoState {
        repo: repo_ref.full_name(),
        workflows,
        active_runs: all_active,
        recent_runs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_backoff_no_backoff() {
        let base = Duration::from_secs(30);
        let result = compute_backoff_interval(base, 0);
        assert_eq!(result, base);
    }

    #[test]
    fn test_compute_backoff_first_level() {
        // 30 * 2^1 = 60, plus jitter in [0, 30)
        let base = Duration::from_secs(30);
        let result = compute_backoff_interval(base, 1);
        assert!(result.as_secs() >= 60);
        assert!(result.as_secs() < 90);
    }

    #[test]
    fn test_compute_backoff_second_level() {
        // 30 * 2^2 = 120, plus jitter in [0, 30)
        let base = Duration::from_secs(30);
        let result = compute_backoff_interval(base, 2);
        assert!(result.as_secs() >= 120);
        assert!(result.as_secs() < 150);
    }

    #[test]
    fn test_compute_backoff_capped_at_max() {
        // Even with high consecutive_low, should never exceed MAX_BACKOFF_SECS
        let base = Duration::from_secs(30);
        let result = compute_backoff_interval(base, 100);
        assert!(result.as_secs() <= MAX_BACKOFF_SECS);
    }

    #[test]
    fn test_compute_backoff_power_cap() {
        // Power capped at MAX_BACKOFF_POWER = 5
        // 30 * 2^5 = 960 -> capped to 300
        let base = Duration::from_secs(30);
        let result = compute_backoff_interval(base, 5);
        assert!(result.as_secs() <= MAX_BACKOFF_SECS);
    }

    #[test]
    fn test_compute_backoff_zero_base() {
        let base = Duration::from_secs(0);
        let result = compute_backoff_interval(base, 5);
        assert_eq!(result.as_secs(), 0);
    }

    #[test]
    fn test_compute_backoff_small_base() {
        // 10 * 2^1 = 20, plus jitter in [0, 10)
        let base = Duration::from_secs(10);
        let result = compute_backoff_interval(base, 1);
        assert!(result.as_secs() >= 20);
        assert!(result.as_secs() < 30);
    }

    #[test]
    fn test_backoff_thresholds() {
        assert!(CRITICAL_THRESHOLD < BACKOFF_THRESHOLD);
        assert!(BACKOFF_THRESHOLD > 0);
        assert!(MAX_BACKOFF_SECS > 0);
    }
}
