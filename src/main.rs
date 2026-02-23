//! GitWorkflowDashboard — native desktop application.
//!
//! Uses Slint for the UI (main thread) and a tokio background thread
//! for GitHub API polling, command handling, and state management.

use clap::Parser;
use slint::{ComponentHandle, Model};
use tokio::sync::mpsc;

use git_workflow_dashboard::config::{AppConfig, Args};
use git_workflow_dashboard::ui;
use git_workflow_dashboard::{
    DispatchInputModel, MainWindow, ToastModel,
};

/// Build metadata embedded at compile time.
const GIT_HASH: &str = env!("GWD_GIT_HASH");
const BUILT_AT: &str = env!("GWD_BUILT_AT");

fn main() {
    // ─── 1. Initialize Structured Logging ───
    let json_logging = std::env::var("LOG_FORMAT")
        .ok()
        .map(|v| v == "json")
        .unwrap_or(false);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if json_logging {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_thread_ids(true)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .init();
    }

    // ─── 2. Parse CLI Args & Merge with Env Vars ───
    let args = Args::parse();
    let config = AppConfig::from_args_and_env(args);

    // ─── 3. Validate Config at Startup ───
    if let Err(e) = config.validate() {
        tracing::error!(error = %e, "Invalid configuration");
        eprintln!("Configuration error: {}", e);
        std::process::exit(1);
    }

    tracing::info!(
        repos = config.repos.len(),
        poll_interval = config.poll_interval_secs,
        "Starting GitWorkflowDashboard (native)"
    );
    tracing::info!("Build: commit={} built_at={}", GIT_HASH, BUILT_AT);

    // ─── 4. Create Slint UI Window ───
    let main_window = MainWindow::new().expect("Failed to create main window");

    // Set build info
    main_window.set_build_hash(slint::SharedString::from(GIT_HASH));
    main_window.set_build_time(slint::SharedString::from(BUILT_AT));

    // ─── 5. Apply Saved Theme ───
    let saved_theme = ui::theme::load_saved_theme();
    ui::theme::apply_theme(&main_window, saved_theme);

    // ─── 6. Create Command Channel (UI → Backend) ───
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<ui::backend::UiCommand>();

    // ─── 7. Wire UI Callbacks to Command Channel ───
    wire_callbacks(&main_window, cmd_tx);

    // ─── 8. Start Backend Thread (tokio runtime + poller + command handler) ───
    ui::backend::start_backend(main_window.as_weak(), config, cmd_rx);

    // ─── 9. Run Slint Event Loop (blocks until window closes) ───
    main_window.run().expect("Slint event loop failed");

    tracing::info!("Window closed — shutting down");
}

/// Wire all Slint UI callbacks to send UiCommands over the mpsc channel.
fn wire_callbacks(ui: &MainWindow, tx: mpsc::UnboundedSender<ui::backend::UiCommand>) {
    use ui::backend::UiCommand;

    // Theme changed
    {
        let handle = ui.as_weak();
        ui.on_theme_changed(move |idx| {
            let idx = idx as usize;
            if let Some(ui) = handle.upgrade() {
                git_workflow_dashboard::ui::theme::apply_theme(&ui, idx);
                git_workflow_dashboard::ui::theme::save_theme(idx);
            }
        });
    }

    // Refresh
    {
        let tx = tx.clone();
        ui.on_refresh_clicked(move || {
            let _ = tx.send(UiCommand::ManualRefresh);
        });
    }

    // Add repo
    {
        let tx = tx.clone();
        ui.on_add_repo(move |name| {
            let name = name.to_string();
            if !name.trim().is_empty() {
                let _ = tx.send(UiCommand::AddRepos(vec![name]));
            }
        });
    }

    // Remove repo
    {
        let tx = tx.clone();
        ui.on_remove_repo(move |name| {
            let _ = tx.send(UiCommand::RemoveRepos(vec![name.to_string()]));
        });
    }

    // Search repos
    {
        let tx = tx.clone();
        ui.on_search_repos(move |query| {
            let _ = tx.send(UiCommand::SearchRepos(query.to_string()));
        });
    }

    // Toggle search result (select/deselect in search results — handled client-side)
    ui.on_toggle_search_result(move |_name| {});

    // Add selected search results
    {
        let tx = tx.clone();
        let handle = ui.as_weak();
        ui.on_add_selected_search_results(move || {
            if let Some(ui) = handle.upgrade() {
                let results = ui.get_search_results();
                let mut names = Vec::new();
                for i in 0..results.row_count() {
                    if let Some(r) = results.row_data(i) {
                        if r.selected {
                            names.push(r.full_name.to_string());
                        }
                    }
                }
                if !names.is_empty() {
                    let _ = tx.send(UiCommand::AddRepos(names));
                }
                ui.set_search_results(slint::ModelRc::default());
            }
        });
    }

    // Toggle sidebar repo selection (handled client-side)
    ui.on_toggle_sidebar_repo(move |_name| {});

    // Toggle run detail (expand/collapse)
    {
        let tx = tx.clone();
        let handle = ui.as_weak();
        ui.on_toggle_run_detail(move |run_id, repo| {
            if let Some(ui) = handle.upgrade() {
                let current = ui.get_expanded_run_id();
                if current == run_id {
                    ui.set_expanded_run_id(-1);
                    ui.set_expanded_jobs(slint::ModelRc::default());
                } else {
                    ui.set_expanded_run_id(run_id);
                    let _ = tx.send(UiCommand::FetchRunDetail {
                        repo: repo.to_string(),
                        run_id: run_id as u64,
                    });
                }
            }
        });
    }

    // Cancel run
    {
        let tx = tx.clone();
        ui.on_cancel_run(move |run_id, repo| {
            let _ = tx.send(UiCommand::CancelRun {
                repo: repo.to_string(),
                run_id: run_id as u64,
            });
        });
    }

    // Re-run
    {
        let tx = tx.clone();
        ui.on_rerun_run(move |run_id, repo| {
            let _ = tx.send(UiCommand::RerunWorkflow {
                repo: repo.to_string(),
                run_id: run_id as u64,
            });
        });
    }

    // View logs
    {
        let tx = tx.clone();
        ui.on_view_logs(move |job_id, repo| {
            let _ = tx.send(UiCommand::FetchJobLog {
                repo: repo.to_string(),
                job_id: job_id as u64,
            });
        });
    }

    // Open URL in browser
    {
        let tx = tx.clone();
        ui.on_open_url(move |url| {
            let _ = tx.send(UiCommand::OpenUrl(url.to_string()));
        });
    }

    // Workflow search (client-side filtering)
    ui.on_workflow_search_changed(move |_q| {});

    // Trigger workflow (open dispatch dialog)
    {
        let tx = tx.clone();
        let handle = ui.as_weak();
        ui.on_trigger_workflow(move |repo, workflow_id, name| {
            if let Some(ui) = handle.upgrade() {
                ui.set_dispatch_repo(repo.clone());
                ui.set_dispatch_workflow_name(name);
                ui.set_dispatch_loading(true);
                ui.set_dispatch_error(slint::SharedString::default());
                ui.set_dispatch_inputs(slint::ModelRc::default());
                ui.set_dispatch_branches(slint::ModelRc::default());
                ui.set_dispatch_show(true);
            }
            let _ = tx.send(UiCommand::FetchBranches {
                repo: repo.to_string(),
            });
            let _ = tx.send(UiCommand::FetchWorkflowInputs {
                repo: repo.to_string(),
                workflow_id: workflow_id as u64,
            });
        });
    }

    // Action search (client-side)
    ui.on_action_search_changed(move |_q| {});

    // Quick run (dispatch with default branch)
    {
        let tx = tx.clone();
        ui.on_quick_run(move |repo, workflow_id, _name| {
            let _ = tx.send(UiCommand::DispatchWorkflow {
                repo: repo.to_string(),
                workflow_id: workflow_id as u64,
                git_ref: "main".to_string(),
                inputs: None,
            });
        });
    }

    // Configure run (open dispatch dialog)
    {
        let tx = tx.clone();
        let handle = ui.as_weak();
        ui.on_configure_run(move |repo, workflow_id, name| {
            if let Some(ui) = handle.upgrade() {
                ui.set_dispatch_repo(repo.clone());
                ui.set_dispatch_workflow_name(name);
                ui.set_dispatch_loading(true);
                ui.set_dispatch_error(slint::SharedString::default());
                ui.set_dispatch_inputs(slint::ModelRc::default());
                ui.set_dispatch_branches(slint::ModelRc::default());
                ui.set_dispatch_show(true);
            }
            let _ = tx.send(UiCommand::FetchBranches {
                repo: repo.to_string(),
            });
            let _ = tx.send(UiCommand::FetchWorkflowInputs {
                repo: repo.to_string(),
                workflow_id: workflow_id as u64,
            });
        });
    }

    // History search (client-side)
    ui.on_history_search_changed(move |_q| {});

    // History time range changed
    {
        let handle = ui.as_weak();
        ui.on_history_time_range_changed(move |range| {
            if let Some(ui) = handle.upgrade() {
                ui.set_history_time_range(range.clone());
                if range.as_str() == "custom" {
                    ui.set_history_show_custom(true);
                } else {
                    ui.set_history_show_custom(false);
                }
            }
        });
    }

    // History toggle run detail
    {
        let tx = tx.clone();
        let handle = ui.as_weak();
        ui.on_history_toggle_run_detail(move |run_id, repo| {
            if let Some(ui) = handle.upgrade() {
                let current = ui.get_history_expanded_run_id();
                if current == run_id {
                    ui.set_history_expanded_run_id(-1);
                    ui.set_history_expanded_jobs(slint::ModelRc::default());
                } else {
                    ui.set_history_expanded_run_id(run_id);
                    let _ = tx.send(UiCommand::FetchRunDetail {
                        repo: repo.to_string(),
                        run_id: run_id as u64,
                    });
                }
            }
        });
    }

    // History re-run
    {
        let tx = tx.clone();
        ui.on_history_rerun(move |run_id, repo| {
            let _ = tx.send(UiCommand::RerunWorkflow {
                repo: repo.to_string(),
                run_id: run_id as u64,
            });
        });
    }

    // History apply custom range
    {
        let handle = ui.as_weak();
        ui.on_history_apply_custom_range(move |_from, _to| {
            if let Some(ui) = handle.upgrade() {
                ui.set_history_time_range(slint::SharedString::from("custom"));
            }
        });
    }

    // What's Happening time range
    {
        let handle = ui.as_weak();
        ui.on_wh_time_range_changed(move |range| {
            if let Some(ui) = handle.upgrade() {
                ui.set_wh_time_range(range);
            }
        });
    }

    // Analytics time range
    {
        let handle = ui.as_weak();
        ui.on_analytics_time_range_changed(move |range| {
            if let Some(ui) = handle.upgrade() {
                ui.set_analytics_time_range(range);
            }
        });
    }

    // Dispatch run
    {
        let tx = tx.clone();
        let handle = ui.as_weak();
        ui.on_dispatch_run(move |repo, _id, branch| {
            if let Some(ui) = handle.upgrade() {
                ui.set_dispatch_loading(true);
                ui.set_dispatch_error(slint::SharedString::default());

                let workflows = ui.get_workflows();
                let dispatch_name = ui.get_dispatch_workflow_name();
                let dispatch_repo = ui.get_dispatch_repo();
                let mut found_id: u64 = 0;

                for i in 0..workflows.row_count() {
                    if let Some(w) = workflows.row_data(i) {
                        if w.name == dispatch_name && w.repo == dispatch_repo {
                            found_id = w.id as u64;
                            break;
                        }
                    }
                }

                if found_id > 0 {
                    let dispatch_inputs = ui.get_dispatch_inputs();
                    let inputs = collect_dispatch_inputs(&dispatch_inputs);
                    let _ = tx.send(UiCommand::DispatchWorkflow {
                        repo: repo.to_string(),
                        workflow_id: found_id,
                        git_ref: branch.to_string(),
                        inputs,
                    });
                }
            }
        });
    }

    // Dispatch close
    {
        let handle = ui.as_weak();
        ui.on_dispatch_close(move || {
            if let Some(ui) = handle.upgrade() {
                ui.set_dispatch_show(false);
            }
        });
    }

    // Log viewer close
    {
        let handle = ui.as_weak();
        ui.on_log_viewer_close(move || {
            if let Some(ui) = handle.upgrade() {
                ui.set_log_viewer_show(false);
            }
        });
    }

    // Dismiss toast
    {
        let handle = ui.as_weak();
        ui.on_dismiss_toast(move |id| {
            if let Some(ui) = handle.upgrade() {
                let existing = ui.get_toasts();
                let mut toasts: Vec<ToastModel> = Vec::new();
                for i in 0..existing.row_count() {
                    if let Some(t) = existing.row_data(i) {
                        if t.id != id {
                            toasts.push(t);
                        }
                    }
                }
                ui.set_toasts(slint::ModelRc::new(slint::VecModel::from(toasts)));
            }
        });
    }
}

/// Collect dispatch input values into a JSON Value (if any inputs have values).
fn collect_dispatch_inputs(
    inputs: &slint::ModelRc<DispatchInputModel>,
) -> Option<serde_json::Value> {
    let mut map = serde_json::Map::new();
    for i in 0..inputs.row_count() {
        if let Some(inp) = inputs.row_data(i) {
            let name = inp.name.to_string();
            if inp.input_type.as_str() == "boolean" {
                map.insert(
                    name,
                    serde_json::Value::String(inp.bool_value.to_string()),
                );
            } else if !inp.value.is_empty() {
                map.insert(name, serde_json::Value::String(inp.value.to_string()));
            } else if !inp.default_value.is_empty() {
                map.insert(
                    name,
                    serde_json::Value::String(inp.default_value.to_string()),
                );
            }
        }
    }
    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map))
    }
}
