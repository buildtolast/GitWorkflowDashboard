//! Handle UI commands — dispatched from the tokio background thread.
//! Each command corresponds to a user action in the Slint UI.

use std::sync::Arc;

use slint::Weak;
use tokio::sync::broadcast;

use crate::config;
use crate::github::client::GitHubClient;
use crate::models::*;
use crate::ui::backend::UiCommand;
use crate::ui::bridge;
use crate::MainWindow;

pub async fn handle_command(
    cmd: UiCommand,
    client: Arc<GitHubClient>,
    state: SharedDashboardState,
    repos: SharedRepoList,
    ws_tx: broadcast::Sender<WsMessage>,
    ui: Weak<MainWindow>,
    data_dir: SharedDataDir,
    max_repos: usize,
) {
    match cmd {
        UiCommand::AddRepos(names) => {
            handle_add_repos(names, repos, ws_tx, ui, data_dir, max_repos).await;
        }
        UiCommand::RemoveRepos(names) => {
            handle_remove_repos(names, repos, state, ws_tx, ui, data_dir).await;
        }
        UiCommand::SearchRepos(query) => {
            handle_search_repos(query, client, repos, ui).await;
        }
        UiCommand::ManualRefresh => {
            show_toast(&ui, "Refresh triggered — next poll will update data", "info");
        }
        UiCommand::DispatchWorkflow {
            repo,
            workflow_id,
            git_ref,
            inputs,
        } => {
            handle_dispatch(repo, workflow_id, git_ref, inputs, client, ui).await;
        }
        UiCommand::CancelRun { repo, run_id } => {
            handle_cancel_run(repo, run_id, client, ui).await;
        }
        UiCommand::RerunWorkflow { repo, run_id } => {
            handle_rerun(repo, run_id, client, ui).await;
        }
        UiCommand::FetchRunDetail { repo, run_id } => {
            handle_fetch_run_detail(repo, run_id, client, ui).await;
        }
        UiCommand::FetchJobLog { repo, job_id } => {
            handle_fetch_job_log(repo, job_id, client, ui).await;
        }
        UiCommand::FetchBranches { repo } => {
            handle_fetch_branches(repo, client, ui).await;
        }
        UiCommand::FetchWorkflowInputs { repo, workflow_id } => {
            handle_fetch_workflow_inputs(repo, workflow_id, client, ui).await;
        }
        UiCommand::OpenUrl(url) => {
            let _ = open::that(&url);
        }
    }
}

async fn handle_add_repos(
    names: Vec<String>,
    repos: SharedRepoList,
    ws_tx: broadcast::Sender<WsMessage>,
    ui: Weak<MainWindow>,
    data_dir: SharedDataDir,
    max_repos: usize,
) {
    let mut added = Vec::new();
    {
        let mut repo_list = repos.write().await;
        for name in &names {
            if repo_list.len() >= max_repos {
                show_toast(&ui, &format!("Max repos ({}) reached", max_repos), "error");
                break;
            }
            match config::validate_repo_ref(name) {
                Ok(repo_ref) => {
                    if !repo_list.iter().any(|r| r.full_name() == repo_ref.full_name()) {
                        repo_list.push(repo_ref.clone());
                        added.push(repo_ref.full_name());
                    }
                }
                Err(e) => {
                    show_toast(&ui, &format!("Invalid repo: {}", e), "error");
                }
            }
        }
    }

    if !added.is_empty() {
        // Persist
        if let Some(ref dir) = *data_dir {
            let repo_list = repos.read().await;
            crate::repo_store::save_repos(dir, &repo_list);
        }
        // Broadcast
        let all: Vec<String> = repos.read().await.iter().map(|r| r.full_name()).collect();
        let _ = ws_tx.send(WsMessage::ReposChanged { repos: all });
        show_toast(
            &ui,
            &format!("Added {} repo(s)", added.len()),
            "success",
        );
    }
}

async fn handle_remove_repos(
    names: Vec<String>,
    repos: SharedRepoList,
    state: SharedDashboardState,
    ws_tx: broadcast::Sender<WsMessage>,
    ui: Weak<MainWindow>,
    data_dir: SharedDataDir,
) {
    let mut removed = 0;
    {
        let mut repo_list = repos.write().await;
        for name in &names {
            if let Some(pos) = repo_list.iter().position(|r| r.full_name() == *name) {
                repo_list.remove(pos);
                removed += 1;
            }
        }
    }
    if removed > 0 {
        // Remove from dashboard state
        {
            let mut ds = state.write().await;
            ds.repos.retain(|r| !names.contains(&r.repo));
        }
        // Persist
        if let Some(ref dir) = *data_dir {
            let repo_list = repos.read().await;
            crate::repo_store::save_repos(dir, &repo_list);
        }
        // Broadcast
        let all: Vec<String> = repos.read().await.iter().map(|r| r.full_name()).collect();
        let _ = ws_tx.send(WsMessage::ReposChanged { repos: all.clone() });

        // Push updated state to UI
        let ds = state.read().await;
        let data = ds.clone();
        drop(ds);
        slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui.upgrade() {
                crate::ui::backend::update_ui_full_state(&ui, &data);
            }
        })
        .ok();
    }
}

async fn handle_search_repos(
    query: String,
    client: Arc<GitHubClient>,
    repos: SharedRepoList,
    ui: Weak<MainWindow>,
) {
    if query.trim().is_empty() {
        slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui.upgrade() {
                ui.set_search_results(slint::ModelRc::default());
            }
        })
        .ok();
        return;
    }

    match client.search_repos(&query, 20).await {
        Ok((_total, results)) => {
            // Filter out already-monitored repos
            let monitored: std::collections::HashSet<String> = repos
                .read()
                .await
                .iter()
                .map(|r| r.full_name())
                .collect();
            let filtered: Vec<_> = results
                .into_iter()
                .filter(|r| !monitored.contains(&r.full_name))
                .collect();
            // Create ModelRc on UI thread (Rc is not Send)
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui.upgrade() {
                    let selected = std::collections::HashSet::new();
                    let model = bridge::search_results_to_slint(&filtered, &selected);
                    ui.set_search_results(model);
                }
            })
            .ok();
        }
        Err(e) => {
            tracing::error!("Repo search failed: {}", e);
            show_toast(&ui, "Search failed", "error");
        }
    }
}

async fn handle_dispatch(
    repo: String,
    workflow_id: u64,
    git_ref: String,
    inputs: Option<serde_json::Value>,
    client: Arc<GitHubClient>,
    ui: Weak<MainWindow>,
) {
    let parts: Vec<&str> = repo.splitn(2, '/').collect();
    if parts.len() != 2 {
        show_toast(&ui, "Invalid repo format", "error");
        return;
    }

    let body = GitHubDispatchBody {
        git_ref,
        inputs,
    };

    match client
        .dispatch_workflow(parts[0], parts[1], workflow_id, &body)
        .await
    {
        Ok(()) => {
            show_toast(&ui, "Workflow dispatched successfully", "success");
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui.upgrade() {
                    ui.set_dispatch_show(false);
                }
            })
            .ok();
        }
        Err(e) => {
            tracing::error!("Dispatch failed: {}", e);
            let msg = format!("Dispatch failed: {}", e);
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui.upgrade() {
                    ui.set_dispatch_error(slint::SharedString::from(msg));
                    ui.set_dispatch_loading(false);
                }
            })
            .ok();
        }
    }
}

async fn handle_cancel_run(
    repo: String,
    run_id: u64,
    client: Arc<GitHubClient>,
    ui: Weak<MainWindow>,
) {
    let parts: Vec<&str> = repo.splitn(2, '/').collect();
    if parts.len() != 2 {
        return;
    }
    match client.cancel_run(parts[0], parts[1], run_id).await {
        Ok(()) => show_toast(&ui, "Run cancelled", "success"),
        Err(e) => {
            tracing::error!("Cancel failed: {}", e);
            show_toast(&ui, "Failed to cancel run", "error");
        }
    }
}

async fn handle_rerun(
    repo: String,
    run_id: u64,
    client: Arc<GitHubClient>,
    ui: Weak<MainWindow>,
) {
    let parts: Vec<&str> = repo.splitn(2, '/').collect();
    if parts.len() != 2 {
        return;
    }
    match client.rerun(parts[0], parts[1], run_id).await {
        Ok(()) => show_toast(&ui, "Workflow re-run triggered", "success"),
        Err(e) => {
            tracing::error!("Re-run failed: {}", e);
            show_toast(&ui, "Failed to re-run workflow", "error");
        }
    }
}

async fn handle_fetch_run_detail(
    repo: String,
    run_id: u64,
    client: Arc<GitHubClient>,
    ui: Weak<MainWindow>,
) {
    let parts: Vec<&str> = repo.splitn(2, '/').collect();
    if parts.len() != 2 {
        return;
    }
    match client.list_jobs(parts[0], parts[1], run_id).await {
        Ok(jobs) => {
            // Create ModelRc on UI thread (Rc is not Send)
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui.upgrade() {
                    let model = bridge::jobs_to_slint(&jobs);
                    ui.set_expanded_jobs(model.clone());
                    ui.set_history_expanded_jobs(model);
                    ui.set_expanded_run_id(run_id as i32);
                    ui.set_history_expanded_run_id(run_id as i32);
                }
            })
            .ok();
        }
        Err(e) => {
            tracing::error!("Fetch run detail failed: {}", e);
            show_toast(&ui, "Failed to load run details", "error");
        }
    }
}

async fn handle_fetch_job_log(
    repo: String,
    job_id: u64,
    client: Arc<GitHubClient>,
    ui: Weak<MainWindow>,
) {
    // Show loading state
    {
        let handle = ui.clone();
        slint::invoke_from_event_loop(move || {
            if let Some(ui) = handle.upgrade() {
                ui.set_log_viewer_show(true);
                ui.set_log_viewer_loading(true);
            }
        })
        .ok();
    }

    let parts: Vec<&str> = repo.splitn(2, '/').collect();
    if parts.len() != 2 {
        return;
    }

    let err_ui = ui.clone();
    match client.get_job_log(parts[0], parts[1], job_id).await {
        Ok(response) => {
            // Extract step data on this thread, then create Slint models on UI thread
            let step_data: Vec<(String, String)> = response
                .steps
                .iter()
                .map(|s| (s.name.clone(), s.log.clone()))
                .collect();
            let truncated = response.truncated;

            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui.upgrade() {
                    let items: Vec<crate::LogStepModel> = step_data
                        .into_iter()
                        .map(|(name, log)| crate::LogStepModel {
                            name: slint::SharedString::from(name),
                            log_text: slint::SharedString::from(log),
                        })
                        .collect();
                    let model = slint::ModelRc::new(slint::VecModel::from(items));
                    ui.set_log_viewer_steps(model);
                    ui.set_log_viewer_truncated(truncated);
                    ui.set_log_viewer_loading(false);
                    ui.set_log_viewer_collapsed(false);
                }
            })
            .ok();
        }
        Err(e) => {
            tracing::error!("Fetch job log failed: {}", e);
            let err_handle = err_ui.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = err_handle.upgrade() {
                    ui.set_log_viewer_loading(false);
                }
            })
            .ok();
            show_toast(&err_ui, "Failed to load logs", "error");
        }
    }
}

async fn handle_fetch_branches(repo: String, client: Arc<GitHubClient>, ui: Weak<MainWindow>) {
    let parts: Vec<&str> = repo.splitn(2, '/').collect();
    if parts.len() != 2 {
        return;
    }
    match client.list_branches_sorted(parts[0], parts[1]).await {
        Ok(sorted_names) => {
            // Create ModelRc on UI thread (Rc is not Send)
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui.upgrade() {
                    let ss: Vec<slint::SharedString> = sorted_names.iter().map(|n| slint::SharedString::from(n.as_str())).collect();
                    let model = slint::ModelRc::new(slint::VecModel::from(ss));
                    ui.set_dispatch_branches(model);
                    ui.set_dispatch_loading(false);
                }
            })
            .ok();
        }
        Err(e) => {
            tracing::error!("Fetch branches failed: {}", e);
            show_toast(&ui, "Failed to load branches", "error");
        }
    }
}

async fn handle_fetch_workflow_inputs(
    repo: String,
    workflow_id: u64,
    client: Arc<GitHubClient>,
    ui: Weak<MainWindow>,
) {
    let parts: Vec<&str> = repo.splitn(2, '/').collect();
    if parts.len() != 2 {
        return;
    }

    // First we need the workflow path to fetch inputs
    // Get it from the dashboard state workflows list
    // For now, try using the workflow_id to construct a path
    // The get_workflow_inputs API needs the workflow file path, not the ID
    // We need to look up the workflow path from the workflows list
    // As a workaround, we'll try fetching workflows first
    let workflow_path = {
        match client.list_workflows(parts[0], parts[1]).await {
            Ok(workflows) => {
                workflows
                    .iter()
                    .find(|w| w.id == workflow_id)
                    .map(|w| w.path.clone())
            }
            Err(_) => None,
        }
    };

    let Some(path) = workflow_path else {
        tracing::warn!("Could not find workflow path for id {}", workflow_id);
        return;
    };

    match client
        .get_workflow_inputs(parts[0], parts[1], &path)
        .await
    {
        Ok((_has_dispatch, inputs)) => {
            // Serialize to plain types that are Send, then build ModelRc on UI thread
            let inputs_clone = inputs;
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui.upgrade() {
                    let input_models: Vec<crate::DispatchInputModel> = inputs_clone
                        .iter()
                        .map(|inp| {
                            let options: Vec<slint::SharedString> = inp
                                .options
                                .iter()
                                .map(|o| slint::SharedString::from(o.as_str()))
                                .collect();
                            crate::DispatchInputModel {
                                name: slint::SharedString::from(inp.name.as_str()),
                                description: slint::SharedString::from(
                                    inp.description.as_deref().unwrap_or(""),
                                ),
                                required: inp.required,
                                default_value: slint::SharedString::from(
                                    inp.default.as_deref().unwrap_or(""),
                                ),
                                input_type: slint::SharedString::from(inp.input_type.as_str()),
                                options: slint::ModelRc::new(slint::VecModel::from(options)),
                                value: slint::SharedString::from(inp.default.as_deref().unwrap_or("")),
                                bool_value: inp.default.as_deref() == Some("true"),
                            }
                        })
                        .collect();
                    let model = slint::ModelRc::new(slint::VecModel::from(input_models));
                    ui.set_dispatch_inputs(model);
                }
            })
            .ok();
        }
        Err(e) => {
            tracing::error!("Fetch workflow inputs failed: {}", e);
        }
    }
}

/// Helper to show a toast notification from the backend thread.
fn show_toast(ui: &Weak<MainWindow>, message: &str, toast_type: &str) {
    let msg = message.to_string();
    let tt = toast_type.to_string();
    let handle = ui.clone();
    slint::invoke_from_event_loop(move || {
        if let Some(ui) = handle.upgrade() {
            use slint::Model;
            // Append a new toast to the list
            let mut toasts: Vec<crate::ToastModel> = Vec::new();
            let existing = ui.get_toasts();
            for i in 0..existing.row_count() {
                if let Some(t) = existing.row_data(i) {
                    toasts.push(t);
                }
            }
            let id = toasts.len() as i32 + 1;
            toasts.push(crate::ToastModel {
                id,
                message: slint::SharedString::from(msg),
                toast_type: slint::SharedString::from(tt),
                timestamp: 0,
            });
            // Keep max 5 toasts
            if toasts.len() > 5 {
                toasts.remove(0);
            }
            ui.set_toasts(slint::ModelRc::new(slint::VecModel::from(toasts)));
        }
    })
    .ok();
}
