use thiserror::Error;

/// Application-level errors for the dashboard.
#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum DashboardError {
    #[error("GitHub API error: {0}")]
    GitHubApi(String),

    #[error("GitHub rate limit exceeded — resets at {reset_at}")]
    RateLimit { reset_at: String },

    #[error("Repository not found: {0}")]
    RepoNotFound(String),

    #[error("Workflow not found: {0}")]
    WorkflowNotFound(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Webhook signature verification failed")]
    WebhookSignatureInvalid,

    #[error("Logs expired or unavailable")]
    LogsExpired,

    #[error("Logs not available")]
    LogsNotAvailable,
}

#[cfg(feature = "web")]
impl actix_web::ResponseError for DashboardError {
    fn error_response(&self) -> actix_web::HttpResponse {
        let status = match self {
            DashboardError::RepoNotFound(_) | DashboardError::WorkflowNotFound(_) => {
                actix_web::http::StatusCode::NOT_FOUND
            }
            DashboardError::Config(_) | DashboardError::Validation(_) => {
                actix_web::http::StatusCode::BAD_REQUEST
            }
            DashboardError::RateLimit { .. } => actix_web::http::StatusCode::TOO_MANY_REQUESTS,
            DashboardError::WebhookSignatureInvalid => actix_web::http::StatusCode::UNAUTHORIZED,
            DashboardError::LogsExpired => actix_web::http::StatusCode::GONE,
            DashboardError::LogsNotAvailable => actix_web::http::StatusCode::NOT_FOUND,
            _ => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        // Sanitize error messages — don't leak GitHub API response bodies or internal URLs
        let safe_message = match self {
            DashboardError::GitHubApi(_) => "GitHub API request failed".to_string(),
            DashboardError::Http(_) => "External HTTP request failed".to_string(),
            DashboardError::RepoNotFound(_) => "Repository not found".to_string(),
            DashboardError::Serialization(_) => "Data processing error".to_string(),
            DashboardError::LogsExpired => "Logs have expired (older than 90 days)".to_string(),
            DashboardError::LogsNotAvailable => "Logs not available yet — job may still be in progress".to_string(),
            _ => self.to_string(),
        };
        // Log the full error detail server-side
        if status == actix_web::http::StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = %self, "Internal error");
        }
        actix_web::HttpResponse::build(status).json(serde_json::json!({
            "error": safe_message
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_api_error_display() {
        let err = DashboardError::GitHubApi("bad request".into());
        assert_eq!(err.to_string(), "GitHub API error: bad request");
    }

    #[test]
    fn test_rate_limit_error_display() {
        let err = DashboardError::RateLimit {
            reset_at: "2026-02-19T12:00:00Z".into(),
        };
        assert!(err.to_string().contains("rate limit exceeded"));
    }

    #[cfg(feature = "web")]
    #[test]
    fn test_repo_not_found_status() {
        use actix_web::ResponseError;
        let err = DashboardError::RepoNotFound("owner/repo".into());
        let resp = err.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
    }

    #[cfg(feature = "web")]
    #[test]
    fn test_validation_error_status() {
        use actix_web::ResponseError;
        let err = DashboardError::Validation("bad input".into());
        let resp = err.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_REQUEST);
    }

    #[cfg(feature = "web")]
    #[test]
    fn test_rate_limit_status() {
        use actix_web::ResponseError;
        let err = DashboardError::RateLimit {
            reset_at: "soon".into(),
        };
        let resp = err.error_response();
        assert_eq!(
            resp.status(),
            actix_web::http::StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[cfg(feature = "web")]
    #[test]
    fn test_webhook_invalid_status() {
        use actix_web::ResponseError;
        let err = DashboardError::WebhookSignatureInvalid;
        let resp = err.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[cfg(feature = "web")]
    #[test]
    fn test_logs_expired_status() {
        use actix_web::ResponseError;
        let err = DashboardError::LogsExpired;
        let resp = err.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::GONE);
    }

    #[cfg(feature = "web")]
    #[test]
    fn test_logs_not_available_status() {
        use actix_web::ResponseError;
        let err = DashboardError::LogsNotAvailable;
        let resp = err.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
    }

    #[cfg(feature = "web")]
    #[test]
    fn test_config_error_status() {
        use actix_web::ResponseError;
        let err = DashboardError::Config("missing field".into());
        let resp = err.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_REQUEST);
    }

    #[cfg(feature = "web")]
    #[test]
    fn test_workflow_not_found_status() {
        use actix_web::ResponseError;
        let err = DashboardError::WorkflowNotFound("missing workflow".into());
        let resp = err.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
    }

    #[cfg(feature = "web")]
    #[test]
    fn test_github_api_error_status_is_500() {
        use actix_web::ResponseError;
        let err = DashboardError::GitHubApi("some internal detail".into());
        let resp = err.error_response();
        assert_eq!(
            resp.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[cfg(feature = "web")]
    #[test]
    fn test_serialization_error_from() {
        use actix_web::ResponseError;
        // Test the From<serde_json::Error> impl
        let bad_json = serde_json::from_str::<serde_json::Value>("not json");
        let serde_err = bad_json.unwrap_err();
        let dashboard_err: DashboardError = serde_err.into();
        assert!(dashboard_err.to_string().contains("Serialization error"));
        let resp = dashboard_err.error_response();
        assert_eq!(
            resp.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_error_display_messages() {
        assert_eq!(
            DashboardError::Config("bad".into()).to_string(),
            "Configuration error: bad"
        );
        assert_eq!(
            DashboardError::Validation("invalid".into()).to_string(),
            "Validation error: invalid"
        );
        assert_eq!(
            DashboardError::WebhookSignatureInvalid.to_string(),
            "Webhook signature verification failed"
        );
        assert_eq!(
            DashboardError::LogsExpired.to_string(),
            "Logs expired or unavailable"
        );
        assert_eq!(
            DashboardError::LogsNotAvailable.to_string(),
            "Logs not available"
        );
    }

    #[test]
    fn test_error_debug_impl() {
        // All error variants should have Debug impl
        let err = DashboardError::GitHubApi("test".into());
        let debug = format!("{:?}", err);
        assert!(debug.contains("GitHubApi"));
    }

    #[test]
    fn test_rate_limit_error_display_includes_reset() {
        let err = DashboardError::RateLimit {
            reset_at: "2026-02-21T15:00:00Z".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("2026-02-21T15:00:00Z"));
        assert!(msg.contains("rate limit exceeded"));
    }
}
