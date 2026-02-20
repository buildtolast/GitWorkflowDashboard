use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::DashboardError;
use crate::models::*;

use super::ETagCache;

/// GitHub API client with rate limit tracking and ETag caching.
pub struct GitHubClient {
    client: reqwest::Client,
    token: String,
    api_url: String,
    rate_limit: Arc<RwLock<RateLimitInfo>>,
    etag_cache: ETagCache,
}

impl GitHubClient {
    /// Create a new GitHub API client.
    pub fn new(token: String, api_url: String) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("git-workflow-dashboard/0.1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            token,
            api_url,
            rate_limit: Arc::new(RwLock::new(RateLimitInfo::default())),
            etag_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// List all workflow definitions for a repository.
    pub async fn list_workflows(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<Workflow>, DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/actions/workflows?per_page=100",
            self.api_url, owner, repo
        );
        let body = self.get_with_etag(&url).await?;
        let response: WorkflowsListResponse = serde_json::from_str(&body)?;
        let mut workflows = response.workflows;
        let full_name = format!("{}/{}", owner, repo);
        for wf in &mut workflows {
            wf.repo = full_name.clone();
        }
        Ok(workflows)
    }

    /// List workflow runs for a repository, optionally filtered by status.
    pub async fn list_runs(
        &self,
        owner: &str,
        repo: &str,
        status: Option<&str>,
        per_page: u32,
    ) -> Result<Vec<WorkflowRun>, DashboardError> {
        let mut url = format!(
            "{}/repos/{}/{}/actions/runs?per_page={}",
            self.api_url, owner, repo, per_page
        );
        if let Some(s) = status {
            url.push_str(&format!("&status={}", s));
        }
        let body = self.get_with_etag(&url).await?;
        let response: RunsListResponse = serde_json::from_str(&body)?;
        let mut runs = response.workflow_runs;
        let full_name = format!("{}/{}", owner, repo);
        for run in &mut runs {
            run.repo = full_name.clone();
        }
        Ok(runs)
    }

    /// List jobs for a specific workflow run.
    pub async fn list_jobs(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
    ) -> Result<Vec<WorkflowJob>, DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs/{}/jobs?per_page=100",
            self.api_url, owner, repo, run_id
        );
        let body = self.get_with_etag(&url).await?;
        let response: JobsListResponse = serde_json::from_str(&body)?;
        Ok(response.jobs)
    }

    /// Trigger a workflow dispatch.
    pub async fn dispatch_workflow(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
        body: &GitHubDispatchBody,
    ) -> Result<(), DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/actions/workflows/{}/dispatches",
            self.api_url, owner, repo, workflow_id
        );
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(body)
            .send()
            .await?;

        self.update_rate_limit(&response).await;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let text = response.text().await.unwrap_or_default();
            Err(DashboardError::GitHubApi(format!(
                "Dispatch failed ({}): {}",
                status, text
            )))
        }
    }

    /// Cancel a running workflow.
    pub async fn cancel_run(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
    ) -> Result<(), DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs/{}/cancel",
            self.api_url, owner, repo, run_id
        );
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;

        self.update_rate_limit(&response).await;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let text = response.text().await.unwrap_or_default();
            Err(DashboardError::GitHubApi(format!(
                "Cancel failed ({}): {}",
                status, text
            )))
        }
    }

    /// Re-run a workflow.
    pub async fn rerun(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
    ) -> Result<(), DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs/{}/rerun",
            self.api_url, owner, repo, run_id
        );
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&serde_json::json!({}))
            .send()
            .await?;

        self.update_rate_limit(&response).await;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let text = response.text().await.unwrap_or_default();
            Err(DashboardError::GitHubApi(format!(
                "Rerun failed ({}): {}",
                status, text
            )))
        }
    }

    /// List branches for a repository (for trigger dialog).
    pub async fn list_branches(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<Branch>, DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/branches?per_page=100",
            self.api_url, owner, repo
        );
        let body = self.get_with_etag(&url).await?;
        let branches: Vec<Branch> = serde_json::from_str(&body)?;
        Ok(branches)
    }

    /// Get a single workflow run.
    pub async fn get_run(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
    ) -> Result<WorkflowRun, DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs/{}",
            self.api_url, owner, repo, run_id
        );
        let body = self.get_with_etag(&url).await?;
        let mut run: WorkflowRun = serde_json::from_str(&body)?;
        run.repo = format!("{}/{}", owner, repo);
        Ok(run)
    }

    /// Fetch workflow file content and parse workflow_dispatch inputs.
    pub async fn get_workflow_inputs(
        &self,
        owner: &str,
        repo: &str,
        workflow_path: &str,
    ) -> Result<(bool, Vec<WorkflowInput>), DashboardError> {
        use base64::Engine;

        let url = format!(
            "{}/repos/{}/{}/contents/{}",
            self.api_url, owner, repo, workflow_path
        );
        let body = self.get_with_etag(&url).await?;
        let file_content: GitHubFileContent = serde_json::from_str(&body)?;

        let raw_content = match (&file_content.content, &file_content.encoding) {
            (Some(content), Some(encoding)) if encoding == "base64" => {
                let cleaned: String = content.chars().filter(|c| !c.is_whitespace()).collect();
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(&cleaned)
                    .map_err(|e| {
                        DashboardError::GitHubApi(format!("Base64 decode error: {}", e))
                    })?;
                String::from_utf8(decoded).map_err(|e| {
                    DashboardError::GitHubApi(format!("UTF-8 decode error: {}", e))
                })?
            }
            _ => {
                return Err(DashboardError::GitHubApi(
                    "Unexpected file content format".into(),
                ));
            }
        };

        // Parse YAML and extract workflow_dispatch inputs
        let yaml: serde_yaml::Value = serde_yaml::from_str(&raw_content).map_err(|e| {
            DashboardError::GitHubApi(format!("YAML parse error: {}", e))
        })?;

        // YAML parses bare `on` as boolean `true`, so check both string and bool keys
        let on_section = yaml
            .get("on")
            .or_else(|| yaml.get(serde_yaml::Value::Bool(true)));

        let dispatch = on_section.and_then(|on| on.get("workflow_dispatch"));

        let has_dispatch = dispatch.is_some();

        let inputs_map = dispatch.and_then(|wd| wd.get("inputs"));

        let mut inputs = Vec::new();

        if let Some(serde_yaml::Value::Mapping(map)) = inputs_map {
            for (key, value) in map {
                let name = match key {
                    serde_yaml::Value::String(s) => s.clone(),
                    _ => continue,
                };

                let description = value
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let required = value
                    .get("required")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let default = value.get("default").map(|v| match v {
                    serde_yaml::Value::String(s) => s.clone(),
                    serde_yaml::Value::Bool(b) => b.to_string(),
                    serde_yaml::Value::Number(n) => n.to_string(),
                    _ => String::new(),
                });

                let input_type = value
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("string")
                    .to_string();

                let options = value
                    .get("options")
                    .and_then(|v| v.as_sequence())
                    .map(|seq| {
                        seq.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                inputs.push(WorkflowInput {
                    name,
                    description,
                    required,
                    default,
                    input_type,
                    options,
                });
            }
        }

        Ok((has_dispatch, inputs))
    }

    /// Get current rate limit info.
    pub async fn get_rate_limit(&self) -> RateLimitInfo {
        self.rate_limit.read().await.clone()
    }

    /// Perform a GET request with ETag conditional caching.
    async fn get_with_etag(&self, url: &str) -> Result<String, DashboardError> {
        let mut req = self
            .client
            .get(url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");

        // Add If-None-Match header if we have a cached ETag
        let cached_etag = {
            let cache = self.etag_cache.read().await;
            cache.get(url).map(|(etag, _)| etag.clone())
        };
        if let Some(ref etag) = cached_etag {
            req = req.header("If-None-Match", etag.as_str());
        }

        let response = req.send().await?;
        self.update_rate_limit(&response).await;

        let status = response.status();

        // 304 Not Modified — return cached body
        if status == reqwest::StatusCode::NOT_MODIFIED {
            let cache = self.etag_cache.read().await;
            if let Some((_, body)) = cache.get(url) {
                return Ok(body.clone());
            }
            // No cached body — should not happen, but fall through
        }

        // 403 with rate limit exhausted
        if status == reqwest::StatusCode::FORBIDDEN {
            let rl = self.rate_limit.read().await;
            if rl.remaining == 0 {
                return Err(DashboardError::RateLimit {
                    reset_at: rl
                        .reset
                        .map(|r| r.to_rfc3339())
                        .unwrap_or_else(|| "unknown".into()),
                });
            }
        }

        // 404
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(DashboardError::RepoNotFound(url.to_string()));
        }

        // Other errors
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(DashboardError::GitHubApi(format!(
                "GitHub API error ({}): {}",
                status, text
            )));
        }

        // Extract ETag and cache
        let etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let body = response.text().await?;

        if let Some(etag) = etag {
            let mut cache = self.etag_cache.write().await;
            cache.insert(url.to_string(), (etag, body.clone()));
        }

        Ok(body)
    }

    /// Extract rate limit info from response headers.
    async fn update_rate_limit(&self, response: &reqwest::Response) {
        let headers = response.headers();

        let limit = headers
            .get("x-ratelimit-limit")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);

        let remaining = headers
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);

        let used = headers
            .get("x-ratelimit-used")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);

        let reset = headers
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok())
            .and_then(|ts| {
                chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            });

        if limit > 0 || remaining > 0 || used > 0 {
            let mut rl = self.rate_limit.write().await;
            rl.limit = limit;
            rl.remaining = remaining;
            rl.used = used;
            rl.reset = reset;
        }
    }
}
