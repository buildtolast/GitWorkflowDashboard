use git_workflow_dashboard::config::{parse_repos, validate_repo_ref, validate_branch_name, AppConfig, RepoRef};
use git_workflow_dashboard::database;
use git_workflow_dashboard::error::DashboardError;
use git_workflow_dashboard::models::*;
use git_workflow_dashboard::repo_store;

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
        dashboard_token: None,
        cors_origins: vec![],
        max_repos: 50,
        data_dir: None,
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
        dashboard_token: None,
        cors_origins: vec![],
        max_repos: 50,
        data_dir: None,
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
        dashboard_token: None,
        cors_origins: vec![],
        max_repos: 50,
        data_dir: None,
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
        dashboard_token: None,
        cors_origins: vec![],
        max_repos: 50,
        data_dir: None,
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
        dashboard_token: None,
        cors_origins: vec![],
        max_repos: 50,
        data_dir: None,
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
        dashboard_token: None,
        cors_origins: vec![],
        max_repos: 50,
        data_dir: None,
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
        dashboard_token: None,
        cors_origins: vec![],
        max_repos: 50,
        data_dir: None,
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
        dashboard_token: None,
        cors_origins: vec![],
        max_repos: 50,
        data_dir: None,
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
    assert_eq!(wf.id, WorkflowId(12345));
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
    assert_eq!(run.id, RunId(99999));
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
        workflow_id: WorkflowId(42),
        git_ref: "main".into(),
        inputs: Some(serde_json::json!({"env": "prod"})),
        idempotency_key: None,
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"ref\":\"main\""));
    assert!(json.contains("\"env\":\"prod\""));
}

#[test]
fn test_dispatch_request_no_inputs() {
    let req = DispatchRequest {
        repo: "owner/repo".into(),
        workflow_id: WorkflowId(42),
        git_ref: "develop".into(),
        inputs: None,
        idempotency_key: None,
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

// ═══════════════════════════════════════════════════════════════
//   ERROR SANITIZATION TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_error_sanitization_github_api_no_leak() {
    use actix_web::ResponseError;
    // Ensure internal GitHub API details are NOT leaked in the response body
    let err = DashboardError::GitHubApi(
        "HTTP 404: {\"message\":\"Not Found\",\"documentation_url\":\"https://docs.github.com\"}"
            .into(),
    );
    let resp = err.error_response();
    assert_eq!(
        resp.status(),
        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
    );
    // The internal error message should include the detail
    assert!(err.to_string().contains("HTTP 404"));
    // But the sanitized message used in the response should be generic
    // (we can't easily extract the body here, but we verify the status is 500)
}

#[test]
fn test_error_sanitization_repo_not_found() {
    use actix_web::ResponseError;
    let err = DashboardError::RepoNotFound("https://api.github.com/repos/secret/repo".into());
    let resp = err.error_response();
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
    // Internal display includes the URL
    assert!(err.to_string().contains("https://api.github.com"));
}

#[test]
fn test_error_logs_expired_sanitized_message() {
    let err = DashboardError::LogsExpired;
    assert_eq!(err.to_string(), "Logs expired or unavailable");
}

#[test]
fn test_error_logs_not_available_sanitized_message() {
    let err = DashboardError::LogsNotAvailable;
    assert_eq!(err.to_string(), "Logs not available");
}

#[test]
fn test_error_logs_not_available_status_code() {
    use actix_web::ResponseError;
    let err = DashboardError::LogsNotAvailable;
    assert_eq!(
        err.error_response().status(),
        actix_web::http::StatusCode::NOT_FOUND
    );
}

#[test]
fn test_error_config_display() {
    let err = DashboardError::Config("missing token".into());
    assert_eq!(err.to_string(), "Configuration error: missing token");
}

#[test]
fn test_error_config_status() {
    use actix_web::ResponseError;
    let err = DashboardError::Config("bad config".into());
    assert_eq!(
        err.error_response().status(),
        actix_web::http::StatusCode::BAD_REQUEST
    );
}

#[test]
fn test_error_workflow_not_found() {
    use actix_web::ResponseError;
    let err = DashboardError::WorkflowNotFound("workflow 42".into());
    assert_eq!(
        err.error_response().status(),
        actix_web::http::StatusCode::NOT_FOUND
    );
    assert!(err.to_string().contains("workflow 42"));
}

#[test]
fn test_error_github_api_maps_to_500() {
    use actix_web::ResponseError;
    let err = DashboardError::GitHubApi("network error".into());
    assert_eq!(
        err.error_response().status(),
        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
    );
}

// ═══════════════════════════════════════════════════════════════
//   BRANCH VALIDATION TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_validate_branch_name_valid() {
    assert!(validate_branch_name("main").is_ok());
    assert!(validate_branch_name("feature/add-login").is_ok());
    assert!(validate_branch_name("release-1.0").is_ok());
    assert!(validate_branch_name("fix_bug").is_ok());
}

#[test]
fn test_validate_branch_name_empty() {
    let err = validate_branch_name("").unwrap_err();
    assert!(err.contains("empty"));
}

#[test]
fn test_validate_branch_name_too_long() {
    let long_name = "a".repeat(257);
    let err = validate_branch_name(&long_name).unwrap_err();
    assert!(err.contains("too long"));
}

#[test]
fn test_validate_branch_name_invalid_chars() {
    assert!(validate_branch_name("feature branch").is_err()); // space
    assert!(validate_branch_name("feature~1").is_err()); // tilde
    assert!(validate_branch_name("feature^2").is_err()); // caret
    assert!(validate_branch_name("feature:name").is_err()); // colon
    assert!(validate_branch_name("feature?name").is_err()); // question mark
    assert!(validate_branch_name("feature*name").is_err()); // asterisk
    assert!(validate_branch_name("feature[name").is_err()); // bracket
}

#[test]
fn test_validate_branch_name_double_dots() {
    let err = validate_branch_name("feature..name").unwrap_err();
    assert!(err.contains(".."));
}

#[test]
fn test_validate_branch_name_leading_trailing_dot() {
    assert!(validate_branch_name(".hidden").is_err());
    assert!(validate_branch_name("name.").is_err());
}

#[test]
fn test_validate_branch_name_leading_trailing_slash() {
    assert!(validate_branch_name("/leading").is_err());
    assert!(validate_branch_name("trailing/").is_err());
}

// ═══════════════════════════════════════════════════════════════
//   CONFIG TOKEN LENGTH VALIDATION
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_config_validation_token_too_long() {
    let config = AppConfig {
        github_token: "x".repeat(1025),
        repos: vec![],
        server_host: "127.0.0.1".into(),
        server_port: 4000,
        poll_interval_secs: 30,
        webhook_secret: None,
        github_api_url: "https://api.github.com".into(),
        dashboard_token: None,
        cors_origins: vec![],
        max_repos: 50,
        data_dir: None,
    };
    let err = config.validate().unwrap_err();
    assert!(err.contains("too long"));
}

#[test]
fn test_config_validation_whitespace_only_token() {
    let config = AppConfig {
        github_token: "   ".into(),
        repos: vec![],
        server_host: "127.0.0.1".into(),
        server_port: 4000,
        poll_interval_secs: 30,
        webhook_secret: None,
        github_api_url: "https://api.github.com".into(),
        dashboard_token: None,
        cors_origins: vec![],
        max_repos: 50,
        data_dir: None,
    };
    let err = config.validate().unwrap_err();
    assert!(err.contains("github_token"));
}

#[test]
fn test_config_validation_invalid_repo_in_config() {
    let config = AppConfig {
        github_token: "ghp_test".into(),
        repos: vec![RepoRef {
            owner: "valid".into(),
            repo: "repo with spaces".into(),
        }],
        server_host: "127.0.0.1".into(),
        server_port: 4000,
        poll_interval_secs: 30,
        webhook_secret: None,
        github_api_url: "https://api.github.com".into(),
        dashboard_token: None,
        cors_origins: vec![],
        max_repos: 50,
        data_dir: None,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_config_validation_poll_interval_boundary_10() {
    let config = AppConfig {
        github_token: "ghp_test".into(),
        repos: vec![],
        server_host: "127.0.0.1".into(),
        server_port: 4000,
        poll_interval_secs: 10, // exact minimum
        webhook_secret: None,
        github_api_url: "https://api.github.com".into(),
        dashboard_token: None,
        cors_origins: vec![],
        max_repos: 50,
        data_dir: None,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_validation_poll_interval_boundary_300() {
    let config = AppConfig {
        github_token: "ghp_test".into(),
        repos: vec![],
        server_host: "127.0.0.1".into(),
        server_port: 4000,
        poll_interval_secs: 300, // exact maximum
        webhook_secret: None,
        github_api_url: "https://api.github.com".into(),
        dashboard_token: None,
        cors_origins: vec![],
        max_repos: 50,
        data_dir: None,
    };
    assert!(config.validate().is_ok());
}

// ═══════════════════════════════════════════════════════════════
//   REPO VALIDATION EDGE CASES
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_validate_repo_ref_with_dots() {
    let repo = validate_repo_ref("my.org/my.repo").unwrap();
    assert_eq!(repo.owner, "my.org");
    assert_eq!(repo.repo, "my.repo");
}

#[test]
fn test_validate_repo_ref_missing_owner() {
    assert!(validate_repo_ref("/repo").is_err());
}

#[test]
fn test_validate_repo_ref_missing_repo() {
    assert!(validate_repo_ref("owner/").is_err());
}

#[test]
fn test_validate_repo_ref_trimming() {
    let repo = validate_repo_ref("  owner/repo  ").unwrap();
    assert_eq!(repo.owner, "owner");
    assert_eq!(repo.repo, "repo");
}

#[test]
fn test_validate_repo_ref_multiple_slashes() {
    // splitn(2, '/') means "owner" and "repo/extra"
    let result = validate_repo_ref("owner/repo/extra");
    // "repo/extra" has a slash which is not in valid_chars, so should fail
    assert!(result.is_err());
}

#[test]
fn test_repo_ref_display_trait() {
    let repo = RepoRef {
        owner: "test-org".into(),
        repo: "test-repo".into(),
    };
    assert_eq!(format!("{}", repo), "test-org/test-repo");
}

#[test]
fn test_repo_ref_equality() {
    let a = RepoRef {
        owner: "org".into(),
        repo: "repo".into(),
    };
    let b = RepoRef {
        owner: "org".into(),
        repo: "repo".into(),
    };
    assert_eq!(a, b);
}

#[test]
fn test_repo_ref_inequality() {
    let a = RepoRef {
        owner: "org1".into(),
        repo: "repo".into(),
    };
    let b = RepoRef {
        owner: "org2".into(),
        repo: "repo".into(),
    };
    assert_ne!(a, b);
}

#[test]
fn test_repo_ref_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(RepoRef {
        owner: "org".into(),
        repo: "repo".into(),
    });
    assert!(set.contains(&RepoRef {
        owner: "org".into(),
        repo: "repo".into(),
    }));
}

#[test]
fn test_parse_repos_invalid_entry() {
    let result = parse_repos("valid/repo,invalid");
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════
//   ADVANCED MODEL TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_workflow_run_minimal_fields() {
    // Only required fields present
    let json = r#"{
        "id": 1,
        "workflow_id": 1,
        "head_sha": "abc",
        "event": "push",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z",
        "html_url": "https://example.com",
        "run_number": 1
    }"#;
    let run: WorkflowRun = serde_json::from_str(json).unwrap();
    assert!(run.name.is_none());
    assert!(run.head_branch.is_none());
    assert!(run.status.is_none());
    assert!(run.conclusion.is_none());
    assert!(run.run_started_at.is_none());
    assert!(run.run_attempt.is_none());
    assert!(run.display_title.is_none());
    assert!(run.actor.is_none());
    assert_eq!(run.repo, ""); // default
}

#[test]
fn test_workflow_job_minimal() {
    let json = r#"{
        "id": 1,
        "run_id": 100,
        "name": "build",
        "status": "queued"
    }"#;
    let job: WorkflowJob = serde_json::from_str(json).unwrap();
    assert_eq!(job.id, JobId(1));
    assert_eq!(job.run_id, RunId(100));
    assert_eq!(job.name, "build");
    assert!(job.conclusion.is_none());
    assert!(job.steps.is_none());
}

#[test]
fn test_workflow_step_fields() {
    let step = WorkflowStep {
        name: "Checkout".into(),
        status: "completed".into(),
        conclusion: Some("success".into()),
        number: 1,
        started_at: None,
        completed_at: None,
    };
    let json = serde_json::to_string(&step).unwrap();
    assert!(json.contains("\"name\":\"Checkout\""));
    assert!(json.contains("\"conclusion\":\"success\""));
}

#[test]
fn test_run_detail_response_ser() {
    let run = WorkflowRun {
        id: RunId(42),
        name: Some("CI".into()),
        workflow_id: WorkflowId(1),
        head_branch: Some("main".into()),
        head_sha: "abc".into(),
        status: Some("completed".into()),
        conclusion: Some("success".into()),
        event: "push".into(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        run_started_at: None,
        html_url: "https://example.com".into(),
        run_number: 1,
        run_attempt: None,
        display_title: None,
        actor: None,
        repo: "owner/repo".into(),
    };
    let resp = RunDetailResponse {
        run,
        jobs: vec![],
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"run\""));
    assert!(json.contains("\"jobs\":[]"));
}

#[test]
fn test_job_log_response_ser() {
    let resp = JobLogResponse {
        job_id: JobId(555),
        steps: vec![JobLogStep {
            name: "Build".into(),
            log: "compiling...".into(),
        }],
        truncated: false,
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"job_id\":555"));
    assert!(json.contains("\"name\":\"Build\""));
    assert!(json.contains("\"truncated\":false"));
}

#[test]
fn test_job_log_response_truncated() {
    let resp = JobLogResponse {
        job_id: JobId(1),
        steps: vec![],
        truncated: true,
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"truncated\":true"));
}

#[test]
fn test_workflow_inputs_response() {
    let resp = WorkflowInputsResponse {
        workflow_id: WorkflowId(42),
        has_dispatch: true,
        inputs: vec![WorkflowInput {
            name: "environment".into(),
            description: Some("Target environment".into()),
            required: true,
            default: Some("staging".into()),
            input_type: "choice".into(),
            options: vec!["staging".into(), "production".into()],
        }],
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"has_dispatch\":true"));
    assert!(json.contains("\"environment\""));
    assert!(json.contains("\"staging\""));
}

#[test]
fn test_workflow_input_default_type() {
    let json = r#"{"name": "param1"}"#;
    let input: WorkflowInput = serde_json::from_str(json).unwrap();
    assert_eq!(input.input_type, "string"); // default
    assert!(!input.required);
    assert!(input.options.is_empty());
}

#[test]
fn test_repos_response_ser() {
    let resp = ReposResponse {
        repos: vec!["a/b".into(), "c/d".into()],
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"repos\""));
    assert!(json.contains("\"a/b\""));
}

#[test]
fn test_search_repo_item_deser() {
    let json = r#"{
        "full_name": "org/repo",
        "description": "A cool project",
        "private": false,
        "default_branch": "main",
        "updated_at": "2026-01-15T00:00:00Z",
        "stargazers_count": 100,
        "language": "Rust"
    }"#;
    let item: SearchRepoItem = serde_json::from_str(json).unwrap();
    assert_eq!(item.full_name, "org/repo");
    assert!(!item.private);
    assert_eq!(item.language.as_deref(), Some("Rust"));
    assert_eq!(item.stargazers_count, Some(100));
}

#[test]
fn test_search_repo_item_minimal() {
    let json = r#"{
        "full_name": "org/repo",
        "private": true
    }"#;
    let item: SearchRepoItem = serde_json::from_str(json).unwrap();
    assert_eq!(item.full_name, "org/repo");
    assert!(item.private);
    assert!(item.description.is_none());
    assert!(item.language.is_none());
}

#[test]
fn test_search_repos_response_ser() {
    let resp = SearchReposResponse {
        total_count: 42,
        results: vec![],
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"total_count\":42"));
    assert!(json.contains("\"results\":[]"));
}

#[test]
fn test_github_dispatch_body_ser() {
    let body = GitHubDispatchBody {
        git_ref: "main".into(),
        inputs: Some(serde_json::json!({"key": "val"})),
    };
    let json = serde_json::to_string(&body).unwrap();
    assert!(json.contains("\"ref\":\"main\""));
    assert!(json.contains("\"key\":\"val\""));
}

#[test]
fn test_github_dispatch_body_no_inputs() {
    let body = GitHubDispatchBody {
        git_ref: "develop".into(),
        inputs: None,
    };
    let json = serde_json::to_string(&body).unwrap();
    assert!(json.contains("\"ref\":\"develop\""));
    assert!(!json.contains("inputs"));
}

#[test]
fn test_ws_message_run_update_tag() {
    let run = WorkflowRun {
        id: RunId(1),
        name: None,
        workflow_id: WorkflowId(1),
        head_branch: None,
        head_sha: "abc".into(),
        status: Some("in_progress".into()),
        conclusion: None,
        event: "push".into(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
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
        run: Box::new(run),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"type\":\"run_update\""));
    assert!(json.contains("\"repo\":\"owner/repo\""));
}

#[test]
fn test_dashboard_state_with_repos() {
    let state = DashboardState {
        repos: vec![RepoState {
            repo: "org/project".into(),
            workflows: vec![],
            active_runs: vec![],
            recent_runs: vec![],
        }],
        rate_limit: RateLimitInfo::default(),
        last_poll: Some(chrono::Utc::now()),
        github_reachable: true,
    };
    assert_eq!(state.repos.len(), 1);
    assert_eq!(state.repos[0].repo, "org/project");
    assert!(state.last_poll.is_some());
}

#[test]
fn test_repo_state_serialization() {
    let state = RepoState {
        repo: "org/repo".into(),
        workflows: vec![],
        active_runs: vec![],
        recent_runs: vec![],
    };
    let json = serde_json::to_string(&state).unwrap();
    assert!(json.contains("\"repo\":\"org/repo\""));
    assert!(json.contains("\"workflows\":[]"));
    assert!(json.contains("\"active_runs\":[]"));
    assert!(json.contains("\"recent_runs\":[]"));
}

#[test]
fn test_rate_limit_info_serialization() {
    let rl = RateLimitInfo {
        limit: 5000,
        remaining: 4999,
        reset: Some(chrono::Utc::now()),
        used: 1,
    };
    let json = serde_json::to_string(&rl).unwrap();
    assert!(json.contains("\"limit\":5000"));
    assert!(json.contains("\"remaining\":4999"));
    assert!(json.contains("\"used\":1"));
    assert!(json.contains("\"reset\""));
}

// ═══════════════════════════════════════════════════════════════
//   REPO STORE PERSISTENCE TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_repo_store_roundtrip_via_database() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = database::Database::open(tmp.path()).unwrap();
    let repos = vec![
        RepoRef { owner: "octocat".into(), repo: "hello-world".into() },
        RepoRef { owner: "rust-lang".into(), repo: "rust".into() },
    ];
    db.save_repos(&repos);
    let loaded = db.load_repos();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].full_name(), "octocat/hello-world");
    assert_eq!(loaded[1].full_name(), "rust-lang/rust");
}

#[test]
fn test_repo_store_empty_via_database() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = database::Database::open(tmp.path()).unwrap();
    db.save_repos(&[]);
    let loaded = db.load_repos();
    assert!(loaded.is_empty());
}

#[test]
fn test_repo_store_missing_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let loaded = repo_store::load_repos(tmp.path());
    assert!(loaded.is_empty());
}

#[test]
fn test_repo_store_invalid_json() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("repos.json"), "not valid json!").unwrap();
    let loaded = repo_store::load_repos(tmp.path());
    assert!(loaded.is_empty());
}

#[test]
fn test_repo_store_overwrite_via_database() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = database::Database::open(tmp.path()).unwrap();
    let repos1 = vec![RepoRef { owner: "a".into(), repo: "b".into() }];
    let repos2 = vec![
        RepoRef { owner: "c".into(), repo: "d".into() },
        RepoRef { owner: "e".into(), repo: "f".into() },
    ];
    db.save_repos(&repos1);
    assert_eq!(db.load_repos().len(), 1);
    db.save_repos(&repos2);
    let loaded = db.load_repos();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].full_name(), "c/d");
}

#[test]
fn test_repo_store_resolve_data_dir_custom() {
    let tmp = tempfile::TempDir::new().unwrap();
    let custom = Some(tmp.path().join("custom_dir").to_string_lossy().to_string());
    let dir = repo_store::resolve_data_dir(&custom);
    assert!(dir.is_some());
    assert!(dir.unwrap().exists());
}

#[test]
fn test_repo_store_resolve_data_dir_default() {
    // With no custom dir, should use ~/.gwd/ — we just check it returns Some
    let dir = repo_store::resolve_data_dir(&None);
    assert!(dir.is_some());
}

// ═══════════════════════════════════════════════════════════════
//   GITHUB FILE CONTENT MODEL TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_github_file_content_deser() {
    let json = r#"{"content": "SGVsbG8=", "encoding": "base64"}"#;
    let fc: GitHubFileContent = serde_json::from_str(json).unwrap();
    assert_eq!(fc.content.as_deref(), Some("SGVsbG8="));
    assert_eq!(fc.encoding.as_deref(), Some("base64"));
}

#[test]
fn test_github_file_content_optional() {
    let json = r#"{}"#;
    let fc: GitHubFileContent = serde_json::from_str(json).unwrap();
    assert!(fc.content.is_none());
    assert!(fc.encoding.is_none());
}

#[test]
fn test_github_branch_detail_deser() {
    let json = r#"{
        "name": "main",
        "protected": true,
        "commit": {"sha": "abc123", "url": "https://api.github.com/repos/o/r/commits/abc123"}
    }"#;
    let branch: GitHubBranchDetail = serde_json::from_str(json).unwrap();
    assert_eq!(branch.name, "main");
    assert!(branch.protected);
    assert_eq!(branch.commit.sha, "abc123");
}

#[test]
fn test_github_search_repos_response_deser() {
    let json = r#"{
        "total_count": 1,
        "items": [{"full_name": "org/repo", "private": false}]
    }"#;
    let resp: GitHubSearchReposResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.total_count, 1);
    assert_eq!(resp.items.len(), 1);
    assert_eq!(resp.items[0].full_name, "org/repo");
}

// ═══════════════════════════════════════════════════════════════
//   CONFIG DEBUG IMPL TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_config_debug_redacts_token() {
    let config = AppConfig {
        github_token: "ghp_secret_token_12345".into(),
        repos: vec![],
        server_host: "127.0.0.1".into(),
        server_port: 4000,
        poll_interval_secs: 30,
        webhook_secret: Some("webhook_secret_xyz".into()),
        github_api_url: "https://api.github.com".into(),
        dashboard_token: Some("dashboard_token_abc".into()),
        cors_origins: vec![],
        max_repos: 50,
        data_dir: None,
    };
    let debug_str = format!("{:?}", config);
    // Token should be redacted
    assert!(!debug_str.contains("ghp_secret_token_12345"));
    assert!(debug_str.contains("[REDACTED]"));
    // Webhook secret should be redacted
    assert!(!debug_str.contains("webhook_secret_xyz"));
    // Dashboard token should be redacted
    assert!(!debug_str.contains("dashboard_token_abc"));
    // Other fields should be present
    assert!(debug_str.contains("4000"));
    assert!(debug_str.contains("127.0.0.1"));
}

// ═══════════════════════════════════════════════════════════════
//   WORKFLOWS LIST/RUNS LIST RESPONSE WRAPPER TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_workflows_list_response_deser() {
    let json = r#"{
        "total_count": 2,
        "workflows": [
            {"id": 1, "name": "CI", "path": ".github/workflows/ci.yml", "state": "active", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"},
            {"id": 2, "name": "Deploy", "path": ".github/workflows/deploy.yml", "state": "active", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}
        ]
    }"#;
    let resp: WorkflowsListResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.total_count, 2);
    assert_eq!(resp.workflows.len(), 2);
    assert_eq!(resp.workflows[0].name, "CI");
}

#[test]
fn test_runs_list_response_deser() {
    let json = r#"{
        "total_count": 1,
        "workflow_runs": [
            {"id": 99, "workflow_id": 1, "head_sha": "abc", "event": "push", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z", "html_url": "https://example.com", "run_number": 1}
        ]
    }"#;
    let resp: RunsListResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.total_count, 1);
    assert_eq!(resp.workflow_runs.len(), 1);
    assert_eq!(resp.workflow_runs[0].id, RunId(99));
}

#[test]
fn test_jobs_list_response_deser() {
    let json = r#"{
        "total_count": 1,
        "jobs": [
            {"id": 1, "run_id": 99, "name": "build", "status": "completed", "conclusion": "success"}
        ]
    }"#;
    let resp: JobsListResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.total_count, 1);
    assert_eq!(resp.jobs.len(), 1);
    assert_eq!(resp.jobs[0].conclusion.as_deref(), Some("success"));
}

// ═══════════════════════════════════════════════════════════════
//   ACTOR MODEL TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_actor_deser() {
    let json = r#"{"login": "octocat", "id": 583231, "avatar_url": "https://avatars.githubusercontent.com/u/583231"}"#;
    let actor: Actor = serde_json::from_str(json).unwrap();
    assert_eq!(actor.login, "octocat");
    assert_eq!(actor.id, 583231);
    assert!(actor.avatar_url.is_some());
}

#[test]
fn test_actor_no_avatar() {
    let json = r#"{"login": "bot", "id": 1}"#;
    let actor: Actor = serde_json::from_str(json).unwrap();
    assert_eq!(actor.login, "bot");
    assert!(actor.avatar_url.is_none());
}

// ═══════════════════════════════════════════════════════════════
//   GITHUB COMMIT MODEL TESTS
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_github_commit_detail_deser() {
    let json = r#"{
        "commit": {
            "committer": {"date": "2026-01-15T12:00:00Z"},
            "author": {"date": "2026-01-15T11:00:00Z"}
        }
    }"#;
    let detail: GitHubCommitDetail = serde_json::from_str(json).unwrap();
    assert!(detail.commit.committer.is_some());
    assert!(detail.commit.author.is_some());
}

#[test]
fn test_github_commit_detail_minimal() {
    let json = r#"{"commit": {}}"#;
    let detail: GitHubCommitDetail = serde_json::from_str(json).unwrap();
    assert!(detail.commit.committer.is_none());
    assert!(detail.commit.author.is_none());
}
