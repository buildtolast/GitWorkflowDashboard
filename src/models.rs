use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::RepoRef;

// ═══════════════════════════════════════════════════════════════
//   GITHUB API RESPONSE TYPES
// ═══════════════════════════════════════════════════════════════

/// A workflow definition (from GET /repos/{owner}/{repo}/actions/workflows).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Populated after fetch — not from GitHub API.
    #[serde(default)]
    pub repo: String,
}

/// A workflow run (from GET /repos/{owner}/{repo}/actions/runs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: u64,
    pub name: Option<String>,
    pub workflow_id: u64,
    pub head_branch: Option<String>,
    pub head_sha: String,
    pub status: Option<String>,
    pub conclusion: Option<String>,
    pub event: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub run_started_at: Option<DateTime<Utc>>,
    pub html_url: String,
    pub run_number: u64,
    pub run_attempt: Option<u64>,
    pub display_title: Option<String>,
    /// Actor who triggered the run.
    pub actor: Option<Actor>,
    /// Populated after fetch — not from GitHub API.
    #[serde(default)]
    pub repo: String,
}

/// GitHub user/actor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor {
    pub login: String,
    pub id: u64,
    pub avatar_url: Option<String>,
}

/// A job within a workflow run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowJob {
    pub id: u64,
    pub run_id: u64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub steps: Option<Vec<WorkflowStep>>,
    pub html_url: Option<String>,
    pub runner_name: Option<String>,
    pub labels: Option<Vec<String>>,
}

/// A step within a job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub number: u64,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

// ═══════════════════════════════════════════════════════════════
//   GITHUB API LIST RESPONSE WRAPPERS
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct WorkflowsListResponse {
    pub total_count: u64,
    pub workflows: Vec<Workflow>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RunsListResponse {
    pub total_count: u64,
    pub workflow_runs: Vec<WorkflowRun>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct JobsListResponse {
    pub total_count: u64,
    pub jobs: Vec<WorkflowJob>,
}

// ═══════════════════════════════════════════════════════════════
//   DISPATCH (TRIGGER) TYPES
// ═══════════════════════════════════════════════════════════════

/// Request body for triggering a workflow dispatch (from frontend).
#[derive(Debug, Serialize, Deserialize)]
pub struct DispatchRequest {
    pub repo: String,
    pub workflow_id: u64,
    #[serde(rename = "ref")]
    pub git_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<serde_json::Value>,
}

/// GitHub dispatch API body (what we POST to GitHub).
#[derive(Debug, Serialize)]
pub struct GitHubDispatchBody {
    #[serde(rename = "ref")]
    pub git_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<serde_json::Value>,
}

// ═══════════════════════════════════════════════════════════════
//   DASHBOARD STATE TYPES
// ═══════════════════════════════════════════════════════════════

/// Full dashboard state snapshot.
#[derive(Debug, Clone, Serialize, Default)]
pub struct DashboardState {
    pub repos: Vec<RepoState>,
    pub rate_limit: RateLimitInfo,
    pub last_poll: Option<DateTime<Utc>>,
}

/// State for a single repository.
#[derive(Debug, Clone, Serialize)]
pub struct RepoState {
    pub repo: String,
    pub workflows: Vec<Workflow>,
    pub active_runs: Vec<WorkflowRun>,
    pub recent_runs: Vec<WorkflowRun>,
}

/// GitHub API rate limit tracking.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RateLimitInfo {
    pub limit: u32,
    pub remaining: u32,
    pub reset: Option<DateTime<Utc>>,
    pub used: u32,
}

// ═══════════════════════════════════════════════════════════════
//   WEBSOCKET MESSAGE TYPES
// ═══════════════════════════════════════════════════════════════

/// WebSocket message types sent to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    /// Full state snapshot (sent on connect + periodically).
    #[serde(rename = "state")]
    StateUpdate { data: DashboardState },

    /// A single run changed status.
    #[serde(rename = "run_update")]
    RunUpdate { repo: String, run: WorkflowRun },

    /// Rate limit status changed.
    #[serde(rename = "rate_limit")]
    RateLimitUpdate { data: RateLimitInfo },

    /// A repo was added or removed.
    #[serde(rename = "repos_changed")]
    ReposChanged { repos: Vec<String> },
}

// ═══════════════════════════════════════════════════════════════
//   API REQUEST/RESPONSE TYPES (dashboard REST API)
// ═══════════════════════════════════════════════════════════════

/// Request to add or remove repos at runtime.
#[derive(Debug, Deserialize)]
pub struct RepoManageRequest {
    pub repos: Vec<String>,
}

/// Response for repo listing.
#[derive(Debug, Serialize)]
pub struct ReposResponse {
    pub repos: Vec<String>,
}

/// GitHub branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    #[serde(default)]
    pub protected: bool,
}

/// Response for run detail including jobs.
#[derive(Debug, Serialize)]
pub struct RunDetailResponse {
    pub run: WorkflowRun,
    pub jobs: Vec<WorkflowJob>,
}

/// Response for on-demand job logs used in step expansion.
#[derive(Debug, Serialize)]
pub struct JobLogsResponse {
    pub job_id: u64,
    pub logs: String,
    pub truncated: bool,
}

/// Request to post a text payload to a Slack Incoming Webhook.
#[derive(Debug, Deserialize)]
pub struct SlackMessageRequest {
    pub webhook_url: String,
    pub text: String,
}

/// Response from Slack message relay endpoint.
#[derive(Debug, Serialize)]
pub struct SlackMessageResponse {
    pub success: bool,
    pub message: String,
}

/// Response for cancelling or re-running a workflow.
#[derive(Debug, Serialize)]
pub struct ActionResponse {
    pub success: bool,
    pub message: String,
}

// ═══════════════════════════════════════════════════════════════
//   WORKFLOW INPUT TYPES (parsed from workflow YAML)
// ═══════════════════════════════════════════════════════════════

/// GitHub Contents API response for a single file.
#[derive(Debug, Deserialize)]
pub struct GitHubFileContent {
    pub content: Option<String>,
    pub encoding: Option<String>,
}

/// A single workflow_dispatch input definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInput {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    pub default: Option<String>,
    /// One of: string, boolean, choice, number, environment
    #[serde(rename = "type", default = "default_input_type")]
    pub input_type: String,
    /// Only for type=choice
    #[serde(default)]
    pub options: Vec<String>,
}

fn default_input_type() -> String {
    "string".into()
}

/// Response for GET /api/workflows/{owner}/{repo}/{workflow_id}/inputs
#[derive(Debug, Serialize)]
pub struct WorkflowInputsResponse {
    pub workflow_id: u64,
    pub has_dispatch: bool,
    pub inputs: Vec<WorkflowInput>,
}

// ═══════════════════════════════════════════════════════════════
//   REPO SEARCH TYPES (GitHub Search API)
// ═══════════════════════════════════════════════════════════════

/// A repository returned from GitHub search API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRepoItem {
    pub full_name: String,
    pub description: Option<String>,
    pub private: bool,
    pub default_branch: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
    pub stargazers_count: Option<u64>,
    pub language: Option<String>,
}

/// GitHub search API response wrapper.
#[derive(Debug, Deserialize)]
pub struct GitHubSearchReposResponse {
    pub total_count: u64,
    pub items: Vec<SearchRepoItem>,
}

/// Our API response for repo search.
#[derive(Debug, Serialize)]
pub struct SearchReposResponse {
    pub total_count: u64,
    pub results: Vec<SearchRepoItem>,
}

/// Shared state type aliases.
pub type SharedDashboardState = std::sync::Arc<tokio::sync::RwLock<DashboardState>>;
pub type SharedRepoList = std::sync::Arc<tokio::sync::RwLock<Vec<RepoRef>>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_roundtrip() {
        let wf = Workflow {
            id: 12345,
            name: "CI".into(),
            path: ".github/workflows/ci.yml".into(),
            state: "active".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            repo: "owner/repo".into(),
        };
        let json = serde_json::to_string(&wf).unwrap();
        let decoded: Workflow = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, 12345);
        assert_eq!(decoded.name, "CI");
        assert_eq!(decoded.path, ".github/workflows/ci.yml");
    }

    #[test]
    fn test_workflow_run_roundtrip() {
        let run = WorkflowRun {
            id: 99999,
            name: Some("Build".into()),
            workflow_id: 12345,
            head_branch: Some("main".into()),
            head_sha: "abc123".into(),
            status: Some("in_progress".into()),
            conclusion: None,
            event: "push".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            run_started_at: Some(Utc::now()),
            html_url: "https://github.com/owner/repo/actions/runs/99999".into(),
            run_number: 42,
            run_attempt: Some(1),
            display_title: Some("Fix build".into()),
            actor: Some(Actor {
                login: "octocat".into(),
                id: 1,
                avatar_url: Some("https://avatars.githubusercontent.com/u/1".into()),
            }),
            repo: "owner/repo".into(),
        };
        let json = serde_json::to_string(&run).unwrap();
        let decoded: WorkflowRun = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, 99999);
        assert_eq!(decoded.status.as_deref(), Some("in_progress"));
        assert!(decoded.conclusion.is_none());
    }

    #[test]
    fn test_workflow_run_completed() {
        let json = r#"{
            "id": 100,
            "workflow_id": 1,
            "head_sha": "abc",
            "status": "completed",
            "conclusion": "success",
            "event": "push",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:05:00Z",
            "html_url": "https://example.com",
            "run_number": 1
        }"#;
        let run: WorkflowRun = serde_json::from_str(json).unwrap();
        assert_eq!(run.conclusion.as_deref(), Some("success"));
    }

    #[test]
    fn test_workflow_job_with_steps() {
        let job = WorkflowJob {
            id: 555,
            run_id: 99999,
            name: "build".into(),
            status: "completed".into(),
            conclusion: Some("success".into()),
            started_at: Some(Utc::now()),
            completed_at: Some(Utc::now()),
            steps: Some(vec![WorkflowStep {
                name: "Checkout".into(),
                status: "completed".into(),
                conclusion: Some("success".into()),
                number: 1,
                started_at: Some(Utc::now()),
                completed_at: Some(Utc::now()),
            }]),
            html_url: Some("https://example.com".into()),
            runner_name: Some("ubuntu-latest".into()),
            labels: Some(vec!["ubuntu-latest".into()]),
        };
        let json = serde_json::to_string(&job).unwrap();
        let decoded: WorkflowJob = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.steps.as_ref().unwrap().len(), 1);
        assert_eq!(decoded.steps.unwrap()[0].name, "Checkout");
    }

    #[test]
    fn test_dispatch_request_ser() {
        let req = DispatchRequest {
            repo: "owner/repo".into(),
            workflow_id: 12345,
            git_ref: "main".into(),
            inputs: Some(serde_json::json!({"env": "production"})),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"ref\":\"main\""));
        assert!(json.contains("\"env\":\"production\""));
    }

    #[test]
    fn test_dispatch_request_no_inputs() {
        let req = DispatchRequest {
            repo: "owner/repo".into(),
            workflow_id: 12345,
            git_ref: "main".into(),
            inputs: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("inputs"));
    }

    #[test]
    fn test_ws_message_state_update() {
        let msg = WsMessage::StateUpdate {
            data: DashboardState::default(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"state\""));
    }

    #[test]
    fn test_ws_message_run_update() {
        let run = WorkflowRun {
            id: 1,
            name: None,
            workflow_id: 1,
            head_branch: None,
            head_sha: "abc".into(),
            status: Some("completed".into()),
            conclusion: Some("failure".into()),
            event: "push".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            run_started_at: None,
            html_url: "https://example.com".into(),
            run_number: 1,
            run_attempt: None,
            display_title: None,
            actor: None,
            repo: "owner/repo".into(),
        };
        let msg = WsMessage::RunUpdate {
            repo: "owner/repo".into(),
            run,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"run_update\""));
        assert!(json.contains("\"conclusion\":\"failure\""));
    }

    #[test]
    fn test_ws_message_rate_limit() {
        let msg = WsMessage::RateLimitUpdate {
            data: RateLimitInfo {
                limit: 5000,
                remaining: 4832,
                reset: None,
                used: 168,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"rate_limit\""));
        assert!(json.contains("\"remaining\":4832"));
    }

    #[test]
    fn test_rate_limit_info_default() {
        let rl = RateLimitInfo::default();
        assert_eq!(rl.limit, 0);
        assert_eq!(rl.remaining, 0);
        assert_eq!(rl.used, 0);
        assert!(rl.reset.is_none());
    }

    #[test]
    fn test_repo_manage_request() {
        let json = r#"{"repos": ["owner/repo1", "owner/repo2"]}"#;
        let req: RepoManageRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.repos.len(), 2);
    }

    #[test]
    fn test_branch_deser() {
        let json = r#"{"name": "main", "protected": true}"#;
        let branch: Branch = serde_json::from_str(json).unwrap();
        assert_eq!(branch.name, "main");
        assert!(branch.protected);
    }
}
