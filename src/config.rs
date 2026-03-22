use clap::Parser;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;

/// CLI arguments for the dashboard server.
#[derive(Parser, Debug)]
#[command(
    name = "git-workflow-dashboard",
    about = "GitHub Actions monitoring dashboard — single pane of glass"
)]
pub struct Args {
    /// GitHub Personal Access Token (PAT)
    #[arg(long)]
    pub github_token: Option<String>,

    /// Repositories to monitor (comma-separated, e.g. "owner/repo1,owner/repo2")
    #[arg(long)]
    pub repos: Option<String>,

    /// Server port
    #[arg(long, default_value = "4000")]
    pub port: u16,

    /// Server host
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Polling interval in seconds (how often to fetch runs from GitHub)
    #[arg(long, default_value = "30")]
    pub poll_interval: u64,

    /// GitHub webhook secret (optional — for push-based real-time updates)
    #[arg(long)]
    pub webhook_secret: Option<String>,

    /// GitHub API base URL (for GitHub Enterprise Server)
    #[arg(long, default_value = "https://api.github.com")]
    pub github_api_url: String,

    /// Dashboard access token (optional — protects API endpoints)
    #[arg(long)]
    pub dashboard_token: Option<String>,

    /// Additional CORS origins (comma-separated, e.g. "http://localhost:3000")
    #[arg(long)]
    pub cors_origins: Option<String>,

    /// Maximum number of monitored repos (default: 50)
    #[arg(long, default_value = "50")]
    pub max_repos: usize,

    /// Data directory for persistent storage (default: ~/.gwd)
    #[arg(long)]
    pub data_dir: Option<String>,
}

/// A parsed owner/repo reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RepoRef {
    pub owner: String,
    pub repo: String,
}

impl RepoRef {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

impl fmt::Display for RepoRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.repo)
    }
}

/// Application configuration, merged from CLI args and environment variables.
#[derive(Clone)]
pub struct AppConfig {
    pub github_token: String,
    pub repos: Vec<RepoRef>,
    pub server_host: String,
    pub server_port: u16,
    pub poll_interval_secs: u64,
    pub webhook_secret: Option<String>,
    pub github_api_url: String,
    pub dashboard_token: Option<String>,
    pub cors_origins: Vec<String>,
    pub max_repos: usize,
    pub data_dir: Option<String>,
}

/// Custom Debug that redacts sensitive fields.
impl fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppConfig")
            .field("github_token", &"[REDACTED]")
            .field("repos", &self.repos)
            .field("server_host", &self.server_host)
            .field("server_port", &self.server_port)
            .field("poll_interval_secs", &self.poll_interval_secs)
            .field("webhook_secret", &self.webhook_secret.as_ref().map(|_| "[REDACTED]"))
            .field("github_api_url", &self.github_api_url)
            .field("dashboard_token", &self.dashboard_token.as_ref().map(|_| "[REDACTED]"))
            .field("cors_origins", &self.cors_origins)
            .field("max_repos", &self.max_repos)
            .field("data_dir", &self.data_dir)
            .finish()
    }
}

impl AppConfig {
    /// Build config from parsed CLI args with env var overrides.
    /// Env vars take precedence over CLI args.
    pub fn from_args_and_env(args: Args) -> Self {
        let github_token = env::var("GITHUB_TOKEN")
            .ok()
            .or(args.github_token)
            .unwrap_or_default();

        let repos_str = env::var("GITHUB_REPOS").ok().or(args.repos).unwrap_or_default();
        let repos = parse_repos(&repos_str).unwrap_or_default();

        Self {
            github_token,
            repos,
            server_host: env::var("SERVER_HOST").unwrap_or(args.host),
            server_port: env::var("SERVER_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(args.port),
            poll_interval_secs: env::var("POLL_INTERVAL")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(args.poll_interval),
            webhook_secret: env::var("WEBHOOK_SECRET").ok().or(args.webhook_secret),
            github_api_url: env::var("GITHUB_API_URL").unwrap_or(args.github_api_url),
            dashboard_token: env::var("DASHBOARD_TOKEN").ok().or(args.dashboard_token),
            cors_origins: env::var("CORS_ORIGINS")
                .ok()
                .or(args.cors_origins)
                .map(|s| s.split(',').map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect())
                .unwrap_or_default(),
            max_repos: env::var("MAX_REPOS")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(args.max_repos),
            data_dir: env::var("GWD_DATA_DIR").ok().or(args.data_dir),
        }
    }

    /// Validate configuration at startup.
    pub fn validate(&self) -> Result<(), String> {
        if self.github_token.trim().is_empty() {
            return Err(
                "github_token is required — set via --github-token or GITHUB_TOKEN env var".into(),
            );
        }
        if self.github_token.len() > 1024 {
            return Err(format!(
                "github_token too long ({} chars, max 1024)",
                self.github_token.len()
            ));
        }
        if self.server_port == 0 {
            return Err("server_port must not be 0".into());
        }
        if self.poll_interval_secs < 10 {
            return Err(format!(
                "poll_interval must be at least 10 seconds (got {})",
                self.poll_interval_secs
            ));
        }
        if self.poll_interval_secs > 300 {
            return Err(format!(
                "poll_interval must be at most 300 seconds (got {})",
                self.poll_interval_secs
            ));
        }
        if !self.github_api_url.starts_with("https://") {
            return Err(format!(
                "github_api_url must start with https:// (got '{}')",
                self.github_api_url
            ));
        }
        // Validate each repo ref if any are provided
        for repo in &self.repos {
            validate_repo_ref(&repo.full_name())?;
        }
        Ok(())
    }
}

/// Parse a "owner/repo" string into a RepoRef.
pub fn validate_repo_ref(s: &str) -> Result<RepoRef, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("repository reference must not be empty".into());
    }
    let parts: Vec<&str> = s.splitn(2, '/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(format!(
            "invalid repository format '{}' — expected 'owner/repo'",
            s
        ));
    }
    let owner = parts[0];
    let repo = parts[1];

    // Validate characters (alphanumeric, hyphens, underscores, dots)
    let valid_chars = |c: char| c.is_alphanumeric() || c == '-' || c == '_' || c == '.';
    if !owner.chars().all(valid_chars) {
        return Err(format!(
            "invalid owner '{}' — only alphanumeric, hyphens, underscores, and dots allowed",
            owner
        ));
    }
    if !repo.chars().all(valid_chars) {
        return Err(format!(
            "invalid repo '{}' — only alphanumeric, hyphens, underscores, and dots allowed",
            repo
        ));
    }

    Ok(RepoRef {
        owner: owner.to_string(),
        repo: repo.to_string(),
    })
}

/// Validate a branch name — only safe characters allowed.
pub fn validate_branch_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("branch name must not be empty".into());
    }
    if name.len() > 256 {
        return Err(format!("branch name too long ({} chars, max 256)", name.len()));
    }
    // Git branch names: alphanumeric, hyphens, underscores, dots, slashes (for namespaced)
    // No: spaces, .., ~, ^, :, ?, *, [, \, control chars, leading/trailing dots or slashes
    let valid_char = |c: char| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/';
    if !name.chars().all(valid_char) {
        return Err(format!(
            "invalid branch name '{}' — only alphanumeric, hyphens, underscores, dots, and slashes allowed",
            name
        ));
    }
    if name.contains("..") || name.starts_with('.') || name.ends_with('.') || name.starts_with('/') || name.ends_with('/') {
        return Err(format!(
            "invalid branch name '{}' — must not start/end with dot or slash, or contain '..'",
            name
        ));
    }
    Ok(())
}

/// Parse a comma-separated list of "owner/repo" strings.
pub fn parse_repos(s: &str) -> Result<Vec<RepoRef>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(vec![]);
    }
    s.split(',')
        .map(|part| validate_repo_ref(part.trim()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let args = Args {
            github_token: Some("ghp_test123".into()),
            repos: None,
            port: 4000,
            host: "127.0.0.1".into(),
            poll_interval: 30,
            webhook_secret: None,
            github_api_url: "https://api.github.com".into(),
            dashboard_token: None,
            cors_origins: None,
            max_repos: 50,
            data_dir: None,
        };
        let config = AppConfig::from_args_and_env(args);
        assert_eq!(config.server_port, 4000);
        assert_eq!(config.server_host, "127.0.0.1");
        assert_eq!(config.poll_interval_secs, 30);
        assert_eq!(config.github_api_url, "https://api.github.com");
        assert!(config.repos.is_empty());
    }

    #[test]
    fn test_validate_valid_config() {
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
    fn test_validate_empty_token() {
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
        assert!(config.validate().is_err());
        assert!(config.validate().unwrap_err().contains("github_token"));
    }

    #[test]
    fn test_validate_zero_port() {
        let config = AppConfig {
            github_token: "ghp_test123".into(),
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
        assert!(config.validate().is_err());
        assert!(config.validate().unwrap_err().contains("server_port"));
    }

    #[test]
    fn test_validate_poll_interval_too_low() {
        let config = AppConfig {
            github_token: "ghp_test123".into(),
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
        assert!(config.validate().is_err());
        assert!(config.validate().unwrap_err().contains("at least 10"));
    }

    #[test]
    fn test_validate_poll_interval_too_high() {
        let config = AppConfig {
            github_token: "ghp_test123".into(),
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
        assert!(config.validate().is_err());
        assert!(config.validate().unwrap_err().contains("at most 300"));
    }

    #[test]
    fn test_validate_invalid_api_url() {
        let config = AppConfig {
            github_token: "ghp_test123".into(),
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
        assert!(config.validate().is_err());
        assert!(config.validate().unwrap_err().contains("https://"));
    }

    #[test]
    fn test_validate_repo_ref_valid() {
        let repo = validate_repo_ref("octocat/hello-world").unwrap();
        assert_eq!(repo.owner, "octocat");
        assert_eq!(repo.repo, "hello-world");
        assert_eq!(repo.full_name(), "octocat/hello-world");
    }

    #[test]
    fn test_validate_repo_ref_with_dots() {
        let repo = validate_repo_ref("my.org/my.repo").unwrap();
        assert_eq!(repo.owner, "my.org");
        assert_eq!(repo.repo, "my.repo");
    }

    #[test]
    fn test_validate_repo_ref_empty() {
        assert!(validate_repo_ref("").is_err());
    }

    #[test]
    fn test_validate_repo_ref_no_slash() {
        assert!(validate_repo_ref("justrepo").is_err());
    }

    #[test]
    fn test_validate_repo_ref_invalid_chars() {
        assert!(validate_repo_ref("owner/repo with spaces").is_err());
        assert!(validate_repo_ref("own@er/repo").is_err());
    }

    #[test]
    fn test_parse_repos_single() {
        let repos = parse_repos("octocat/hello-world").unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].full_name(), "octocat/hello-world");
    }

    #[test]
    fn test_parse_repos_multiple() {
        let repos = parse_repos("owner/repo1,owner/repo2,org/repo3").unwrap();
        assert_eq!(repos.len(), 3);
        assert_eq!(repos[0].full_name(), "owner/repo1");
        assert_eq!(repos[1].full_name(), "owner/repo2");
        assert_eq!(repos[2].full_name(), "org/repo3");
    }

    #[test]
    fn test_parse_repos_empty() {
        let repos = parse_repos("").unwrap();
        assert!(repos.is_empty());
    }

    #[test]
    fn test_parse_repos_whitespace() {
        let repos = parse_repos("  owner/repo1 , owner/repo2  ").unwrap();
        assert_eq!(repos.len(), 2);
    }

    #[test]
    fn test_repo_ref_display() {
        let repo = RepoRef {
            owner: "octocat".into(),
            repo: "hello-world".into(),
        };
        assert_eq!(format!("{}", repo), "octocat/hello-world");
    }
}
