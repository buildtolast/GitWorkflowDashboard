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
}

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
            _ => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        actix_web::HttpResponse::build(status).json(serde_json::json!({
            "error": self.to_string()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::ResponseError;

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

    #[test]
    fn test_repo_not_found_status() {
        let err = DashboardError::RepoNotFound("owner/repo".into());
        let resp = err.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_validation_error_status() {
        let err = DashboardError::Validation("bad input".into());
        let resp = err.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_rate_limit_status() {
        let err = DashboardError::RateLimit {
            reset_at: "soon".into(),
        };
        let resp = err.error_response();
        assert_eq!(
            resp.status(),
            actix_web::http::StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[test]
    fn test_webhook_invalid_status() {
        let err = DashboardError::WebhookSignatureInvalid;
        let resp = err.error_response();
        assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }
}
