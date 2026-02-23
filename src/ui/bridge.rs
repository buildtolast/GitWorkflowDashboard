//! Convert backend Rust types (models.rs) to Slint UI model types.

use slint::{ModelRc, SharedString, VecModel};

use crate::models::{
    JobLogResponse, RepoState, SearchRepoItem, WorkflowJob, WorkflowRun,
};
use crate::{
    ActionModel, JobModel, LogStepModel, RunModel, SearchResult, SidebarRepo, StepModel,
    WorkflowModel,
};

/// Convert a list of RepoState into sidebar repo models.
pub fn repos_to_sidebar(
    repos: &[RepoState],
    selected: &std::collections::HashSet<String>,
) -> ModelRc<SidebarRepo> {
    let items: Vec<SidebarRepo> = repos
        .iter()
        .map(|r| SidebarRepo {
            name: SharedString::from(&r.repo),
            active_run_count: r.active_runs.len() as i32,
            selected: selected.contains(&r.repo),
        })
        .collect();
    ModelRc::new(VecModel::from(items))
}

/// Convert all workflows from all repos into a flat WorkflowModel list.
pub fn workflows_to_slint(repos: &[RepoState]) -> ModelRc<WorkflowModel> {
    let items: Vec<WorkflowModel> = repos
        .iter()
        .flat_map(|r| {
            r.workflows.iter().map(move |w| WorkflowModel {
                id: w.id as i32,
                name: SharedString::from(&w.name),
                path: SharedString::from(&w.path),
                state: SharedString::from(&w.state),
                repo: SharedString::from(&r.repo),
            })
        })
        .collect();
    ModelRc::new(VecModel::from(items))
}

/// Convert a WorkflowRun to a Slint RunModel.
pub fn run_to_slint(run: &WorkflowRun) -> RunModel {
    let duration = compute_duration(run);
    RunModel {
        id: run.id as i32,
        name: SharedString::from(run.name.as_deref().unwrap_or("Unnamed")),
        repo: SharedString::from(&run.repo),
        status: SharedString::from(run.status.as_deref().unwrap_or("unknown")),
        conclusion: SharedString::from(run.conclusion.as_deref().unwrap_or("")),
        branch: SharedString::from(run.head_branch.as_deref().unwrap_or("")),
        event: SharedString::from(&run.event),
        actor_login: SharedString::from(
            run.actor
                .as_ref()
                .map(|a| a.login.as_str())
                .unwrap_or(""),
        ),
        actor_avatar_url: SharedString::from(
            run.actor
                .as_ref()
                .and_then(|a| a.avatar_url.as_deref())
                .unwrap_or(""),
        ),
        html_url: SharedString::from(&run.html_url),
        run_number: run.run_number as i32,
        sha: SharedString::from(&run.head_sha[..7.min(run.head_sha.len())]),
        duration_secs: duration,
        display_title: SharedString::from(
            run.display_title
                .as_deref()
                .or(run.name.as_deref())
                .unwrap_or("Unnamed"),
        ),
        expanded: false,
    }
}

/// Compute duration in seconds for a run.
fn compute_duration(run: &WorkflowRun) -> i32 {
    let start = run
        .run_started_at
        .unwrap_or(run.created_at);
    let end = if run.status.as_deref() == Some("completed") {
        run.updated_at
    } else {
        chrono::Utc::now()
    };
    (end - start).num_seconds().max(0) as i32
}

/// Convert all active runs from all repos into a flat RunModel list.
pub fn active_runs_to_slint(repos: &[RepoState]) -> ModelRc<RunModel> {
    let items: Vec<RunModel> = repos
        .iter()
        .flat_map(|r| r.active_runs.iter().map(run_to_slint))
        .collect();
    ModelRc::new(VecModel::from(items))
}

/// Convert all recent runs from all repos into a flat RunModel list (for History).
pub fn history_runs_to_slint(repos: &[RepoState]) -> ModelRc<RunModel> {
    let mut items: Vec<RunModel> = repos
        .iter()
        .flat_map(|r| r.recent_runs.iter().map(run_to_slint))
        .collect();
    // Sort by id descending (most recent first)
    items.sort_by(|a, b| b.id.cmp(&a.id));
    ModelRc::new(VecModel::from(items))
}

/// Convert WorkflowJobs to Slint JobModels.
pub fn jobs_to_slint(jobs: &[WorkflowJob]) -> ModelRc<JobModel> {
    let items: Vec<JobModel> = jobs
        .iter()
        .map(|j| {
            let duration = if let (Some(start), Some(end)) = (j.started_at, j.completed_at) {
                (end - start).num_seconds().max(0) as i32
            } else if let Some(start) = j.started_at {
                (chrono::Utc::now() - start).num_seconds().max(0) as i32
            } else {
                0
            };

            let steps: Vec<StepModel> = j
                .steps
                .as_ref()
                .map(|ss| {
                    ss.iter()
                        .map(|s| {
                            let step_dur =
                                if let (Some(start), Some(end)) = (s.started_at, s.completed_at) {
                                    (end - start).num_seconds().max(0) as i32
                                } else {
                                    0
                                };
                            StepModel {
                                name: SharedString::from(&s.name),
                                status: SharedString::from(&s.status),
                                conclusion: SharedString::from(
                                    s.conclusion.as_deref().unwrap_or(""),
                                ),
                                number: s.number as i32,
                                duration_secs: step_dur,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

            JobModel {
                id: j.id as i32,
                run_id: j.run_id as i32,
                name: SharedString::from(&j.name),
                status: SharedString::from(&j.status),
                conclusion: SharedString::from(j.conclusion.as_deref().unwrap_or("")),
                duration_secs: duration,
                runner_name: SharedString::from(j.runner_name.as_deref().unwrap_or("")),
                steps: ModelRc::new(VecModel::from(steps)),
            }
        })
        .collect();
    ModelRc::new(VecModel::from(items))
}

/// Convert search results to Slint SearchResult models.
pub fn search_results_to_slint(
    items: &[SearchRepoItem],
    selected: &std::collections::HashSet<String>,
) -> ModelRc<SearchResult> {
    let results: Vec<SearchResult> = items
        .iter()
        .map(|r| SearchResult {
            full_name: SharedString::from(&r.full_name),
            description: SharedString::from(r.description.as_deref().unwrap_or("")),
            language: SharedString::from(r.language.as_deref().unwrap_or("")),
            stars: r.stargazers_count.unwrap_or(0) as i32,
            is_private: r.private,
            selected: selected.contains(&r.full_name),
        })
        .collect();
    ModelRc::new(VecModel::from(results))
}

/// Convert log steps to Slint LogStepModels.
pub fn log_steps_to_slint(response: &JobLogResponse) -> (ModelRc<LogStepModel>, bool) {
    let items: Vec<LogStepModel> = response
        .steps
        .iter()
        .map(|s| LogStepModel {
            name: SharedString::from(&s.name),
            log_text: SharedString::from(&s.log),
        })
        .collect();
    (ModelRc::new(VecModel::from(items)), response.truncated)
}

/// Build ActionModel list from repos (workflows + latest run status).
pub fn actions_to_slint(repos: &[RepoState]) -> ModelRc<ActionModel> {
    let items: Vec<ActionModel> = repos
        .iter()
        .flat_map(|r| {
            r.workflows.iter().map(move |w| {
                // Find the latest run for this workflow
                let latest = r
                    .recent_runs
                    .iter()
                    .chain(r.active_runs.iter())
                    .find(|run| run.workflow_id == w.id);

                ActionModel {
                    workflow_id: w.id as i32,
                    workflow_name: SharedString::from(&w.name),
                    repo: SharedString::from(&r.repo),
                    latest_status: SharedString::from(
                        latest
                            .and_then(|r| r.status.as_deref())
                            .unwrap_or(""),
                    ),
                    latest_conclusion: SharedString::from(
                        latest
                            .and_then(|r| r.conclusion.as_deref())
                            .unwrap_or(""),
                    ),
                    latest_run_time: SharedString::from(
                        latest
                            .map(|r| {
                                let secs = (chrono::Utc::now() - r.updated_at).num_seconds();
                                if secs < 60 {
                                    format!("{}s ago", secs)
                                } else if secs < 3600 {
                                    format!("{}m ago", secs / 60)
                                } else if secs < 86400 {
                                    format!("{}h ago", secs / 3600)
                                } else {
                                    format!("{}d ago", secs / 86400)
                                }
                            })
                            .unwrap_or_default(),
                    ),
                    latest_run_url: SharedString::from(
                        latest.map(|r| r.html_url.as_str()).unwrap_or(""),
                    ),
                }
            })
        })
        .collect();
    ModelRc::new(VecModel::from(items))
}
