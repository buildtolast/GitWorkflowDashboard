use git_workflow_dashboard::config::{parse_repos, validate_repo_ref, AppConfig};
use git_workflow_dashboard::error::DashboardError;
use git_workflow_dashboard::models::*;

// ═══════════════════════════════════════════════════════════════
//   CONFIG VALIDATION TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_config_validation_valid() {
    let config = AppConfig {
        github_token: "ghp_test123".into(),
        repos: vec![],
        server_host: "127.0.0.1".into(),
        server_port: 4000,
        poll_interval_secs: 30,
        webhook_secret: None,
        github_api_url: "https://api.github.com".into(),
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_validation_with_repos() {
    let config = AppConfig {
        github_token: "ghp_test123".into(),
        repos: vec![validate_repo_ref("owner/repo").unwrap()],
        server_host: "127.0.0.1".into(),
        server_port: 4000,
        poll_interval_secs: 30,
        webhook_secret: None,
        github_api_url: "https://api.github.com".into(),
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_validation_empty_token() {
    let config = AppConfig {
        github_token: "".into(),
        repos: vec![],
        server_host: "127.0.0.1".into(),
        server_port: 4000,
        poll_interval_secs: 30,
        webhook_secret: None,
        github_api_url: "https://api.github.com".into(),
    };
    let err = config.validate().unwrap_err();
    assert!(err.contains("github_token"));
}

#[test]
fn test_config_validation_zero_port() {
    let config = AppConfig {
        github_token: "ghp_test".into(),
        repos: vec![],
        server_host: "127.0.0.1".into(),
        server_port: 0,
        poll_interval_secs: 30,
        webhook_secret: None,
        github_api_url: "https://api.github.com".into(),
    };
    let err = config.validate().unwrap_err();
    assert!(err.contains("server_port"));
}

#[test]
fn test_config_validation_poll_too_low() {
    let config = AppConfig {
        github_token: "ghp_test".into(),
        repos: vec![],
        server_host: "127.0.0.1".into(),
        server_port: 4000,
        poll_interval_secs: 5,
        webhook_secret: None,
        github_api_url: "https://api.github.com".into(),
    };
    let err = config.validate().unwrap_err();
    assert!(err.contains("at least 10"));
}

#[test]
fn test_config_validation_poll_too_high() {
    let config = AppConfig {
        github_token: "ghp_test".into(),
        repos: vec![],
        server_host: "127.0.0.1".into(),
        server_port: 4000,
        poll_interval_secs: 600,
        webhook_secret: None,
        github_api_url: "https://api.github.com".into(),
    };
    let err = config.validate().unwrap_err();
    assert!(err.contains("at most 300"));
}

#[test]
fn test_config_validation_http_api_url() {
    let config = AppConfig {
        github_token: "ghp_test".into(),
        repos: vec![],
        server_host: "127.0.0.1".into(),
        server_port: 4000,
        poll_interval_secs: 30,
        webhook_secret: None,
        github_api_url: "http://api.github.com".into(),
    };
    let err = config.validate().unwrap_err();
    assert!(err.contains("https://"));
}

#[test]
fn test_config_validation_enterprise_api_url() {
    let config = AppConfig {
        github_token: "ghp_test".into(),
        repos: vec![],
        server_host: "127.0.0.1".into(),
        server_port: 4000,
        poll_interval_secs: 30,
        webhook_secret: None,
        github_api_url: "https://github.mycompany.com/api/v3".into(),
    };
    assert!(config.validate().is_ok());
}

// ═══════════════════════════════════════════════════════════════
//   REPO VALIDATION TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_validate_repo_ref_valid() {
    let repo = validate_repo_ref("octocat/hello-world").unwrap();
    assert_eq!(repo.owner, "octocat");
    assert_eq!(repo.repo, "hello-world");
    assert_eq!(repo.full_name(), "octocat/hello-world");
}

#[test]
fn test_validate_repo_ref_with_underscores() {
    let repo = validate_repo_ref("my_org/my_repo").unwrap();
    assert_eq!(repo.owner, "my_org");
    assert_eq!(repo.repo, "my_repo");
}

#[test]
fn test_validate_repo_ref_empty() {
    assert!(validate_repo_ref("").is_err());
}

#[test]
fn test_validate_repo_ref_no_slash() {
    let err = validate_repo_ref("justrepo").unwrap_err();
    assert!(err.contains("owner/repo"));
}

#[test]
fn test_validate_repo_ref_spaces() {
    assert!(validate_repo_ref("owner/repo name").is_err());
}

#[test]
fn test_validate_repo_ref_special_chars() {
    assert!(validate_repo_ref("owner@/repo").is_err());
}

#[test]
fn test_parse_repos_multiple() {
    let repos = parse_repos("a/b,c/d,e/f").unwrap();
    assert_eq!(repos.len(), 3);
    assert_eq!(repos[0].full_name(), "a/b");
    assert_eq!(repos[2].full_name(), "e/f");
}

#[test]
fn test_parse_repos_empty() {
    let repos = parse_repos("").unwrap();
    assert!(repos.is_empty());
}

#[test]
fn test_parse_repos_whitespace() {
    let repos = parse_repos("  a/b , c/d  ").unwrap();
    assert_eq!(repos.len(), 2);
}

// ═══════════════════════════════════════════════════════════════
//   MODEL SERIALIZATION TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_workflow_from_github_json() {
    let json = r#"{
        "id": 12345,
        "name": "CI",
        "path": ".github/workflows/ci.yml",
        "state": "active",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-15T12:00:00Z"
    }"#;
    let wf: Workflow = serde_json::from_str(json).unwrap();
    assert_eq!(wf.id, 12345);
    assert_eq!(wf.name, "CI");
    assert_eq!(wf.state, "active");
    assert_eq!(wf.repo, ""); // not set from GitHub JSON
}

#[test]
fn test_workflow_run_in_progress() {
    let json = r#"{
        "id": 99999,
        "workflow_id": 12345,
        "head_sha": "abc123def",
        "status": "in_progress",
        "event": "push",
        "created_at": "2026-02-19T10:00:00Z",
        "updated_at": "2026-02-19T10:05:00Z",
        "html_url": "https://github.com/owner/repo/actions/runs/99999",
        "run_number": 42
    }"#;
    let run: WorkflowRun = serde_json::from_str(json).unwrap();
    assert_eq!(run.id, 99999);
    assert_eq!(run.status.as_deref(), Some("in_progress"));
    assert!(run.conclusion.is_none());
}

#[test]
fn test_workflow_run_completed_success() {
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
fn test_workflow_run_with_actor() {
    let json = r#"{
        "id": 200,
        "workflow_id": 1,
        "head_sha": "def",
        "status": "completed",
        "conclusion": "failure",
        "event": "pull_request",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:10:00Z",
        "html_url": "https://example.com",
        "run_number": 5,
        "actor": {"login": "octocat", "id": 1, "avatar_url": "https://avatars.githubusercontent.com/u/1"}
    }"#;
    let run: WorkflowRun = serde_json::from_str(json).unwrap();
    assert_eq!(run.actor.as_ref().unwrap().login, "octocat");
}

#[test]
fn test_dispatch_request_serialization() {
    let req = DispatchRequest {
        repo: "owner/repo".into(),
        workflow_id: 42,
        git_ref: "main".into(),
        inputs: Some(serde_json::json!({"env": "prod"})),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"ref\":\"main\""));
    assert!(json.contains("\"env\":\"prod\""));
}

#[test]
fn test_dispatch_request_no_inputs() {
    let req = DispatchRequest {
        repo: "owner/repo".into(),
        workflow_id: 42,
        git_ref: "develop".into(),
        inputs: None,
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(!json.contains("inputs"));
}

#[test]
fn test_ws_message_state_update_tag() {
    let msg = WsMessage::StateUpdate {
        data: DashboardState::default(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"type\":\"state\""));
    assert!(json.contains("\"data\""));
}

#[test]
fn test_ws_message_rate_limit_tag() {
    let msg = WsMessage::RateLimitUpdate {
        data: RateLimitInfo {
            limit: 5000,
            remaining: 4500,
            reset: None,
            used: 500,
        },
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"type\":\"rate_limit\""));
    assert!(json.contains("\"remaining\":4500"));
}

#[test]
fn test_ws_message_repos_changed_tag() {
    let msg = WsMessage::ReposChanged {
        repos: vec!["a/b".into(), "c/d".into()],
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"type\":\"repos_changed\""));
}

#[test]
fn test_dashboard_state_default() {
    let state = DashboardState::default();
    assert!(state.repos.is_empty());
    assert_eq!(state.rate_limit.limit, 0);
    assert!(state.last_poll.is_none());
}

#[test]
fn test_branch_deser() {
    let json = r#"{"name": "main", "protected": true}"#;
    let branch: Branch = serde_json::from_str(json).unwrap();
    assert_eq!(branch.name, "main");
    assert!(branch.protected);
}

#[test]
fn test_branch_deser_no_protected() {
    let json = r#"{"name": "feature-x"}"#;
    let branch: Branch = serde_json::from_str(json).unwrap();
    assert_eq!(branch.name, "feature-x");
    assert!(!branch.protected);
}

#[test]
fn test_repo_manage_request() {
    let json = r#"{"repos": ["owner/repo1", "owner/repo2"]}"#;
    let req: RepoManageRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.repos.len(), 2);
}

#[test]
fn test_action_response_ser() {
    let resp = ActionResponse {
        success: true,
        message: "Done".into(),
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"success\":true"));
}

// ═══════════════════════════════════════════════════════════════
//   ERROR TYPE TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_error_github_api_display() {
    let err = DashboardError::GitHubApi("not found".into());
    assert_eq!(err.to_string(), "GitHub API error: not found");
}

#[test]
fn test_error_rate_limit_display() {
    let err = DashboardError::RateLimit {
        reset_at: "2026-02-19T12:00:00Z".into(),
    };
    assert!(err.to_string().contains("rate limit exceeded"));
    assert!(err.to_string().contains("2026-02-19"));
}

#[test]
fn test_error_validation_display() {
    let err = DashboardError::Validation("bad input".into());
    assert_eq!(err.to_string(), "Validation error: bad input");
}

#[test]
fn test_error_webhook_display() {
    let err = DashboardError::WebhookSignatureInvalid;
    assert!(err.to_string().contains("signature"));
}

#[test]
fn test_error_repo_not_found_display() {
    let err = DashboardError::RepoNotFound("owner/missing".into());
    assert!(err.to_string().contains("owner/missing"));
}

#[test]
fn test_error_response_status_codes() {
    use actix_web::ResponseError;

    let not_found = DashboardError::RepoNotFound("x".into());
    assert_eq!(
        not_found.error_response().status(),
        actix_web::http::StatusCode::NOT_FOUND
    );

    let bad_req = DashboardError::Validation("x".into());
    assert_eq!(
        bad_req.error_response().status(),
        actix_web::http::StatusCode::BAD_REQUEST
    );

    let rate = DashboardError::RateLimit {
        reset_at: "x".into(),
    };
    assert_eq!(
        rate.error_response().status(),
        actix_web::http::StatusCode::TOO_MANY_REQUESTS
    );

    let webhook = DashboardError::WebhookSignatureInvalid;
    assert_eq!(
        webhook.error_response().status(),
        actix_web::http::StatusCode::UNAUTHORIZED
    );
}
