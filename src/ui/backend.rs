//! Background tokio runtime that runs the poller, handles UI commands,
//! and pushes state updates to the Slint UI thread.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use slint::Weak;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;
use crate::github;
use crate::models::*;
use crate::ui::bridge;
use crate::MainWindow;

/// Commands sent from the UI thread to the backend tokio runtime.
#[derive(Debug)]
pub enum UiCommand {
    AddRepos(Vec<String>),
    RemoveRepos(Vec<String>),
    SearchRepos(String),
    ManualRefresh,
    DispatchWorkflow {
        repo: String,
        workflow_id: u64,
        git_ref: String,
        inputs: Option<serde_json::Value>,
    },
    CancelRun {
        repo: String,
        run_id: u64,
    },
    RerunWorkflow {
        repo: String,
        run_id: u64,
    },
    FetchRunDetail {
        repo: String,
        run_id: u64,
    },
    FetchJobLog {
        repo: String,
        job_id: u64,
    },
    FetchBranches {
        repo: String,
    },
    FetchWorkflowInputs {
        repo: String,
        workflow_id: u64,
    },
    OpenUrl(String),
}

/// Start the background tokio runtime.
/// Returns the mpsc sender for UI commands.
pub fn start_backend(
    ui_handle: Weak<MainWindow>,
    config: AppConfig,
    cmd_rx: mpsc::UnboundedReceiver<UiCommand>,
) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        rt.block_on(async move {
            run_backend(ui_handle, config, cmd_rx).await;
        });
    });
}

async fn run_backend(
    ui_handle: Weak<MainWindow>,
    config: AppConfig,
    mut cmd_rx: mpsc::UnboundedReceiver<UiCommand>,
) {
    // Create shared state (mirrors current main.rs setup)
    let github_client = Arc::new(github::client::GitHubClient::new(
        config.github_token.clone(),
        config.github_api_url.clone(),
    ));

    let dashboard_state: SharedDashboardState = Arc::new(RwLock::new(DashboardState::default()));
    let repo_list: SharedRepoList = Arc::new(RwLock::new(config.repos.clone()));
    let (ws_tx, _ws_rx) = broadcast::channel::<WsMessage>(4096);
    let shutdown = CancellationToken::new();

    // Resolve data directory and merge persisted repos
    let data_dir = crate::repo_store::resolve_data_dir(&config.data_dir);
    if let Some(ref dir) = data_dir {
        let persisted = crate::repo_store::load_repos(dir);
        let mut repos = repo_list.write().await;
        for repo in persisted {
            if !repos.iter().any(|r| r.full_name() == repo.full_name()) {
                repos.push(repo);
            }
        }
    }
    let shared_data_dir: SharedDataDir = Arc::new(data_dir);

    // Spawn background poller
    let run_tracker = github::poller::new_run_tracker();
    let poll_interval = Duration::from_secs(config.poll_interval_secs);

    let poller_client = github_client.clone();
    let poller_state = dashboard_state.clone();
    let poller_repos = repo_list.clone();
    let poller_ws_tx = ws_tx.clone();
    let poller_shutdown = shutdown.clone();
    let poller_tracker = run_tracker.clone();

    tokio::spawn(async move {
        github::poller::run_poller(
            poller_client,
            poller_state,
            poller_repos,
            poller_ws_tx,
            poll_interval,
            poller_shutdown,
            poller_tracker,
        )
        .await;
    });

    // Mark connected
    {
        let handle = ui_handle.clone();
        slint::invoke_from_event_loop(move || {
            if let Some(ui) = handle.upgrade() {
                ui.set_connected(true);
            }
        })
        .ok();
    }

    // Spawn broadcast listener → pushes state to UI
    let mut rx = ws_tx.subscribe();
    let broadcast_ui = ui_handle.clone();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    let handle = broadcast_ui.clone();
                    match msg {
                        WsMessage::StateUpdate { data } => {
                            slint::invoke_from_event_loop(move || {
                                if let Some(ui) = handle.upgrade() {
                                    update_ui_full_state(&ui, &data);
                                }
                            })
                            .ok();
                        }
                        WsMessage::RateLimitUpdate { data } => {
                            slint::invoke_from_event_loop(move || {
                                if let Some(ui) = handle.upgrade() {
                                    ui.set_rate_limit_remaining(data.remaining as i32);
                                    ui.set_rate_limit_total(data.limit as i32);
                                }
                            })
                            .ok();
                        }
                        WsMessage::ReposChanged { repos } => {
                            slint::invoke_from_event_loop(move || {
                                if let Some(ui) = handle.upgrade() {
                                    // Repos changed — full state will follow
                                    let _ = repos;
                                    let _ = ui;
                                }
                            })
                            .ok();
                        }
                        WsMessage::RunUpdate { .. } => {
                            // Individual run updates are handled by the next full state update
                            // For more granular updates, we could parse and merge here
                        }
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("UI broadcast listener lagged by {} messages", n);
                }
            }
        }
    });

    // Command handler loop
    while let Some(cmd) = cmd_rx.recv().await {
        let client = github_client.clone();
        let state = dashboard_state.clone();
        let repos = repo_list.clone();
        let tx = ws_tx.clone();
        let ui = ui_handle.clone();
        let data_dir = shared_data_dir.clone();
        let max_repos = config.max_repos;

        tokio::spawn(async move {
            crate::ui::actions::handle_command(cmd, client, state, repos, tx, ui, data_dir, max_repos)
                .await;
        });
    }

    // Shutdown
    shutdown.cancel();
}

/// Push a full DashboardState to the Slint UI (public for use by actions.rs).
pub fn update_ui_full_state(ui: &MainWindow, data: &DashboardState) {
    // Selected sidebar repos (maintain existing selection)
    let selected = HashSet::<String>::new(); // TODO: track selection state

    // Sidebar repos
    ui.set_sidebar_repos(bridge::repos_to_sidebar(&data.repos, &selected));

    // Active runs
    let active = bridge::active_runs_to_slint(&data.repos);
    let count = data.repos.iter().map(|r| r.active_runs.len()).sum::<usize>() as i32;
    ui.set_active_runs(active);
    ui.set_active_runs_count(count);

    // History runs
    ui.set_history_runs(bridge::history_runs_to_slint(&data.repos));

    // Workflows
    ui.set_workflows(bridge::workflows_to_slint(&data.repos));

    // Actions
    ui.set_actions(bridge::actions_to_slint(&data.repos));

    // Rate limit
    ui.set_rate_limit_remaining(data.rate_limit.remaining as i32);
    ui.set_rate_limit_total(data.rate_limit.limit as i32);

    // Last poll
    if let Some(last) = data.last_poll {
        let secs = (chrono::Utc::now() - last).num_seconds();
        let text = if secs < 60 {
            format!("{}s ago", secs)
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else {
            format!("{}h ago", secs / 3600)
        };
        ui.set_last_poll_text(slint::SharedString::from(text));
    }

    // Analytics (compute from data)
    compute_and_push_analytics(ui, data);
}

/// Compute What's Happening + Analytics data and push to UI.
fn compute_and_push_analytics(ui: &MainWindow, data: &DashboardState) {
    use crate::{
        ActorStat, RepoBreakdown, RunSummary, SuccessRateStat,
    };
    use slint::{ModelRc, SharedString, VecModel};

    let all_runs: Vec<&WorkflowRun> = data
        .repos
        .iter()
        .flat_map(|r| r.active_runs.iter().chain(r.recent_runs.iter()))
        .collect();

    // Run summary
    let mut summary = RunSummary {
        total: all_runs.len() as i32,
        ..Default::default()
    };
    for run in &all_runs {
        match run.status.as_deref() {
            Some("in_progress") => summary.in_progress += 1,
            Some("queued") => summary.queued += 1,
            Some("completed") => match run.conclusion.as_deref() {
                Some("success") => summary.success += 1,
                Some("failure") => summary.failure += 1,
                Some("cancelled") => summary.cancelled += 1,
                _ => {}
            },
            _ => {}
        }
    }
    ui.set_wh_summary(summary);

    // Repo breakdowns
    let repo_bds: Vec<RepoBreakdown> = data
        .repos
        .iter()
        .map(|r| {
            let total = r.active_runs.len() + r.recent_runs.len();
            let success = r
                .recent_runs
                .iter()
                .filter(|run| run.conclusion.as_deref() == Some("success"))
                .count();
            let failure = r
                .recent_runs
                .iter()
                .filter(|run| run.conclusion.as_deref() == Some("failure"))
                .count();
            let rate = if total > 0 {
                success as f32 / total as f32
            } else {
                0.0
            };
            RepoBreakdown {
                repo: SharedString::from(&r.repo),
                total: total as i32,
                success: success as i32,
                failure: failure as i32,
                success_rate: rate,
            }
        })
        .collect();
    ui.set_wh_repo_breakdowns(ModelRc::new(VecModel::from(repo_bds)));

    // Actor leaderboard
    let mut actor_map: std::collections::HashMap<String, (i32, i32, i32)> =
        std::collections::HashMap::new();
    for run in &all_runs {
        if let Some(actor) = &run.actor {
            let entry = actor_map.entry(actor.login.clone()).or_default();
            entry.0 += 1; // total
            if run.conclusion.as_deref() == Some("success") {
                entry.1 += 1;
            }
            if run.conclusion.as_deref() == Some("failure") {
                entry.2 += 1;
            }
        }
    }
    let mut actors: Vec<ActorStat> = actor_map
        .into_iter()
        .map(|(login, (total, success, failure))| ActorStat {
            login: SharedString::from(login),
            run_count: total,
            success_count: success,
            failure_count: failure,
        })
        .collect();
    actors.sort_by(|a, b| b.run_count.cmp(&a.run_count));
    actors.truncate(10);
    ui.set_analytics_actors(ModelRc::new(VecModel::from(actors)));

    // Success rates per repo
    let success_rates: Vec<SuccessRateStat> = data
        .repos
        .iter()
        .filter(|r| !r.recent_runs.is_empty())
        .map(|r| {
            let total = r.recent_runs.len();
            let success = r
                .recent_runs
                .iter()
                .filter(|run| run.conclusion.as_deref() == Some("success"))
                .count();
            SuccessRateStat {
                name: SharedString::from(&r.repo),
                total: total as i32,
                success_rate: if total > 0 {
                    success as f32 / total as f32
                } else {
                    0.0
                },
            }
        })
        .collect();
    ui.set_analytics_success_rates(ModelRc::new(VecModel::from(success_rates)));
}
