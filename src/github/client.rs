use std::sync::Arc;
use tokio::sync::RwLock;

use chrono::{DateTime, Utc};
use crate::error::DashboardError;
use crate::models::*;

use super::ETagCache;
use super::log_parser::{strip_ansi, parse_step_logs};

/// GitHub API client with rate limit tracking and ETag caching.
#[derive(Clone)]
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

    /// Build a GET request with standard GitHub API headers.
    fn github_get(&self, url: &str) -> reqwest::RequestBuilder {
        self.client
            .get(url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
    }

    /// Build a POST request with standard GitHub API headers.
    fn github_post(&self, url: &str) -> reqwest::RequestBuilder {
        self.client
            .post(url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
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
            url.push_str(&format!("&status={}", urlencoding::encode(s)));
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
        run_id: RunId,
    ) -> Result<Vec<WorkflowJob>, DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs/{}/jobs?per_page=100",
            self.api_url, owner, repo, run_id
        );
        let body = self.get_with_etag(&url).await?;
        let response: JobsListResponse = serde_json::from_str(&body)?;
        Ok(response.jobs)
    }

    /// Fetch full plain-text logs for a specific job (simpler variant for orchestration UI).
    pub async fn get_job_logs(
        &self,
        owner: &str,
        repo: &str,
        job_id: JobId,
    ) -> Result<String, DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/actions/jobs/{}/logs",
            self.api_url, owner, repo, job_id
        );
        let response = self
            .github_get(&url)
            .send()
            .await?;

        self.update_rate_limit(&response).await;

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(DashboardError::GitHubApi(format!(
                "Job {} logs not found",
                job_id
            )));
        }
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(DashboardError::GitHubApi(format!(
                "Job log fetch failed ({}): {}",
                status, text
            )));
        }

        Ok(response.text().await?)
    }

    /// Trigger a workflow dispatch.
    pub async fn dispatch_workflow(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: WorkflowId,
        body: &GitHubDispatchBody,
    ) -> Result<(), DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/actions/workflows/{}/dispatches",
            self.api_url, owner, repo, workflow_id
        );
        let response = self
            .github_post(&url)
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
        run_id: RunId,
    ) -> Result<(), DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs/{}/cancel",
            self.api_url, owner, repo, run_id
        );
        let response = self
            .github_post(&url)
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
        run_id: RunId,
    ) -> Result<(), DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs/{}/rerun",
            self.api_url, owner, repo, run_id
        );
        let response = self
            .github_post(&url)
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

    /// List all branches for a repository (paginated, for trigger dialog).
    /// Deduplicates by name and ensures the default branch is always present.
    #[allow(dead_code)]
    pub async fn list_branches(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<Branch>, DashboardError> {
        let mut all_branches: Vec<Branch> = Vec::new();
        let mut page = 1u32;
        let per_page = 100;

        loop {
            let url = format!(
                "{}/repos/{}/{}/branches?per_page={}&page={}",
                self.api_url, owner, repo, per_page, page
            );
            let body = self.get_with_etag(&url).await?;
            let branches: Vec<Branch> = serde_json::from_str(&body)?;
            let count = branches.len();
            all_branches.extend(branches);

            // If we got fewer than per_page, we've reached the last page
            if count < per_page as usize {
                break;
            }
            page += 1;

            // Safety cap: don't fetch more than 20 pages (2000 branches)
            if page > 20 {
                break;
            }
        }

        // Deduplicate by branch name across pages
        let mut seen = std::collections::HashSet::new();
        all_branches.retain(|b| seen.insert(b.name.clone()));

        // Ensure default branch is present even if omitted from paged branch list
        if let Ok(default_branch) = self.get_default_branch(owner, repo).await
            && !all_branches.iter().any(|b| b.name == default_branch)
        {
            all_branches.insert(
                0,
                Branch {
                    name: default_branch,
                    protected: false,
                },
            );
        }

        Ok(all_branches)
    }

    /// List branches sorted for the dispatch dialog.
    /// Priority branches (main, integration, development) appear first,
    /// followed by remaining branches sorted by last commit date (newest first).
    pub async fn list_branches_sorted(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<String>, DashboardError> {
        // 1. Fetch all branches with commit SHAs
        let mut all_branches: Vec<GitHubBranchDetail> = Vec::new();
        let mut page = 1u32;
        let per_page = 100;

        loop {
            let url = format!(
                "{}/repos/{}/{}/branches?per_page={}&page={}",
                self.api_url, owner, repo, per_page, page
            );
            let body = self.get_with_etag(&url).await?;
            let branches: Vec<GitHubBranchDetail> = serde_json::from_str(&body)?;
            let count = branches.len();
            all_branches.extend(branches);
            if count < per_page as usize {
                break;
            }
            page += 1;
            if page > 20 {
                break;
            }
        }

        // 2. Separate priority branches from the rest
        const PRIORITY_NAMES: &[&str] = &["development", "integration", "main"];

        let mut priority: Vec<(usize, &GitHubBranchDetail)> = Vec::new();
        let mut rest: Vec<&GitHubBranchDetail> = Vec::new();

        for branch in &all_branches {
            if let Some(pos) = PRIORITY_NAMES.iter().position(|&p| p == branch.name) {
                priority.push((pos, branch));
            } else {
                rest.push(branch);
            }
        }

        priority.sort_by_key(|(pos, _)| *pos);

        // 3. Batch-fetch commit dates for non-priority branches (10 at a time)
        //    Cap at 100 branches for date fetching to protect rate limit.
        let date_fetch_limit = 100;
        let (to_date, overflow) = if rest.len() > date_fetch_limit {
            (&rest[..date_fetch_limit], &rest[date_fetch_limit..])
        } else {
            (rest.as_slice(), &[] as &[&GitHubBranchDetail])
        };

        let batch_size = 10;
        let mut dated: Vec<(&GitHubBranchDetail, Option<DateTime<Utc>>)> = Vec::new();
        for chunk in to_date.chunks(batch_size) {
            let futs: Vec<_> = chunk
                .iter()
                .map(|branch| {
                    let client = self.clone();
                    let o = owner.to_string();
                    let r = repo.to_string();
                    let sha = branch.commit.sha.clone();
                    async move {
                        client
                            .get_commit_date(&o, &r, &sha)
                            .await
                            .unwrap_or(None)
                    }
                })
                .collect();
            let dates = futures_util::future::join_all(futs).await;
            for (branch, date) in chunk.iter().zip(dates) {
                dated.push((branch, date));
            }
        }

        // 4. Sort by commit date descending (newest first); no-date branches go last
        dated.sort_by(|a, b| match (b.1, a.1) {
            (Some(db), Some(da)) => db.cmp(&da),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.0.name.cmp(&b.0.name),
        });

        // 5. Assemble final list: priority → dated → overflow (alphabetical)
        let mut result: Vec<String> =
            Vec::with_capacity(all_branches.len());
        for (_, branch) in &priority {
            result.push(branch.name.clone());
        }
        for (branch, _) in &dated {
            result.push(branch.name.clone());
        }
        // Overflow branches (beyond date_fetch_limit) sorted alphabetically
        let mut overflow_names: Vec<&str> =
            overflow.iter().map(|b| b.name.as_str()).collect();
        overflow_names.sort();
        for name in overflow_names {
            result.push(name.to_string());
        }

        Ok(result)
    }

    /// Get a single workflow run.
    pub async fn get_run(
        &self,
        owner: &str,
        repo: &str,
        run_id: RunId,
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
        let yaml: serde_yml::Value = serde_yml::from_str(&raw_content).map_err(|e| {
            DashboardError::GitHubApi(format!("YAML parse error: {}", e))
        })?;

        // YAML parses bare `on` as boolean `true`, so check both string and bool keys
        let on_section = yaml
            .get("on")
            .or_else(|| yaml.get(serde_yml::Value::Bool(true)));

        let dispatch = on_section.and_then(|on| on.get("workflow_dispatch"));

        let has_dispatch = dispatch.is_some();

        let inputs_map = dispatch.and_then(|wd| wd.get("inputs"));

        let mut inputs = Vec::new();

        if let Some(serde_yml::Value::Mapping(map)) = inputs_map {
            for (key, value) in map {
                let name = match key {
                    serde_yml::Value::String(s) => s.clone(),
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
                    serde_yml::Value::String(s) => s.clone(),
                    serde_yml::Value::Bool(b) => b.to_string(),
                    serde_yml::Value::Number(n) => n.to_string(),
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

    /// Search repositories via GitHub Search API.
    pub async fn search_repos(
        &self,
        query: &str,
        per_page: u32,
    ) -> Result<(u64, Vec<SearchRepoItem>), DashboardError> {
        // Build a smarter search query for GitHub's search API.
        // If the query looks like "owner/" or "owner/partial", use user/org qualifier.
        // Otherwise, search by name with "in:name" for better partial matching.
        let search_q = if query.contains('/') {
            // User typed "owner/repo" pattern — search within that user/org
            let parts: Vec<&str> = query.splitn(2, '/').collect();
            let owner = parts[0].trim();
            let repo_part = parts.get(1).map(|s| s.trim()).unwrap_or("");
            if repo_part.is_empty() {
                format!("user:{}", owner)
            } else {
                format!("{} in:name user:{}", repo_part, owner)
            }
        } else {
            // Plain query — search in repo name for better partial matching
            format!("{} in:name", query)
        };

        let url = format!(
            "{}/search/repositories?q={}&per_page={}&sort=updated",
            self.api_url,
            urlencoding::encode(&search_q),
            per_page.min(30)
        );

        let response = self
            .github_get(&url)
            .send()
            .await?;

        self.update_rate_limit(&response).await;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(DashboardError::GitHubApi(format!(
                "Search failed ({}): {}",
                status, text
            )));
        }

        let body = response.text().await?;
        let search_response: GitHubSearchReposResponse = serde_json::from_str(&body)?;
        Ok((search_response.total_count, search_response.items))
    }

    /// Get the commit date for a specific commit SHA.
    #[allow(dead_code)]
    pub async fn get_commit_date(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> Result<Option<DateTime<Utc>>, DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/commits/{}",
            self.api_url, owner, repo, sha
        );
        let body = self.get_with_etag(&url).await?;
        let detail: GitHubCommitDetail = serde_json::from_str(&body)?;
        let date = detail
            .commit
            .committer
            .and_then(|c| c.date)
            .or_else(|| detail.commit.author.and_then(|a| a.date));
        Ok(date)
    }

    /// Get current rate limit info.
    pub async fn get_rate_limit(&self) -> RateLimitInfo {
        self.rate_limit.read().await.clone()
    }

    /// Fetch the log for a specific job, with ANSI stripping and step parsing.
    /// Uses a direct GET (not `get_with_etag`) because the endpoint returns a 302
    /// redirect to a temporary blob URL.
    pub async fn get_job_log(
        &self,
        owner: &str,
        repo: &str,
        job_id: JobId,
    ) -> Result<JobLogResponse, DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/actions/jobs/{}/logs",
            self.api_url, owner, repo, job_id
        );

        let response = self
            .github_get(&url)
            .send()
            .await?;

        self.update_rate_limit(&response).await;

        let status = response.status();

        if status == reqwest::StatusCode::GONE {
            return Err(DashboardError::LogsExpired);
        }

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(DashboardError::LogsNotAvailable);
        }

        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(DashboardError::GitHubApi(format!(
                "Job log fetch failed ({}): {}",
                status, text
            )));
        }

        let bytes = response.bytes().await?;
        let max_bytes: usize = 2 * 1024 * 1024; // 2 MB
        let truncated = bytes.len() > max_bytes;
        let raw = if truncated {
            let mut s = String::from_utf8_lossy(&bytes[..max_bytes]).into_owned();
            s.push_str("\n\n--- Log truncated (exceeded 2 MB) ---");
            s
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        };

        let cleaned = strip_ansi(&raw);
        let steps = parse_step_logs(&cleaned);

        Ok(JobLogResponse {
            job_id,
            steps,
            truncated,
        })
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
            // Evict oldest entries if cache exceeds max size
            const MAX_ETAG_CACHE_SIZE: usize = 1000;
            if cache.len() >= MAX_ETAG_CACHE_SIZE {
                // Remove ~10% of entries to avoid evicting on every insert
                let to_remove: Vec<String> = cache
                    .keys()
                    .take(MAX_ETAG_CACHE_SIZE / 10)
                    .cloned()
                    .collect();
                for key in to_remove {
                    cache.remove(&key);
                }
                tracing::debug!(
                    cache_size = cache.len(),
                    "ETag cache eviction triggered"
                );
            }
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

    /// Fetch repository metadata and return the default branch name.
    async fn get_default_branch(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<String, DashboardError> {
        #[derive(serde::Deserialize)]
        struct RepoMeta {
            default_branch: String,
        }

        let url = format!("{}/repos/{}/{}", self.api_url, owner, repo);
        let body = self.get_with_etag(&url).await?;
        let meta: RepoMeta = serde_json::from_str(&body)?;
        Ok(meta.default_branch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_client_new() {
        let client = GitHubClient::new("test_token".into(), "https://api.github.com".into());
        // Verify we can create a client without panicking
        // and it stores the right API URL
        let rt = tokio::runtime::Runtime::new().unwrap();
        let rl = rt.block_on(client.get_rate_limit());
        assert_eq!(rl.limit, 0); // default
        assert_eq!(rl.remaining, 0);
    }
}
