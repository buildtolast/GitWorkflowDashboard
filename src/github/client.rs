use std::sync::Arc;
use tokio::sync::RwLock;

use chrono::{DateTime, Utc};
use crate::error::DashboardError;
use crate::models::*;

use super::ETagCache;

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

    /// List all branches for a repository (paginated, for trigger dialog).
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

        Ok(all_branches)
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
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
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

    /// Get the default branch name for a repository.
    #[allow(dead_code)]
    pub async fn get_default_branch(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<String, DashboardError> {
        let url = format!("{}/repos/{}/{}", self.api_url, owner, repo);
        let body = self.get_with_etag(&url).await?;
        let info: GitHubRepoInfo = serde_json::from_str(&body)?;
        Ok(info.default_branch)
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

    /// Check if a branch has been merged into the default branch.
    /// Uses the compare API — if the branch is "identical" or "behind", it's merged.
    #[allow(dead_code)]
    pub async fn check_branch_merged(
        &self,
        owner: &str,
        repo: &str,
        default_branch: &str,
        branch: &str,
    ) -> Result<bool, DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/compare/{}...{}",
            self.api_url,
            owner,
            repo,
            urlencoding::encode(default_branch),
            urlencoding::encode(branch)
        );
        match self.get_with_etag(&url).await {
            Ok(body) => {
                let compare: GitHubCompareResponse = serde_json::from_str(&body)?;
                // "identical" = same as default, "behind" = default has all branch commits
                Ok(compare.status == "identical" || compare.status == "behind")
            }
            Err(DashboardError::RepoNotFound(_)) => {
                // Compare 404 can happen for unrelated histories
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }

    /// Get detailed branch information for a repository, enriched with merge and age data.
    #[allow(dead_code)]
    ///
    /// Optimized flow:
    /// 1. Get default branch name (1 API call)
    /// 2. List all branches with pagination (1-N API calls)
    /// 3. Fetch commit dates concurrently in batches of 10
    /// 4. Only check merge status for branches older than 30 days (skip young ones entirely)
    pub async fn get_branch_details(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<BranchDetail>, DashboardError> {
        // 1. Get default branch
        let default_branch = self.get_default_branch(owner, repo).await?;

        // 2. List all branches (with commit SHAs)
        let url_base = format!("{}/repos/{}/{}/branches", self.api_url, owner, repo);
        let mut all_branches: Vec<GitHubBranchDetail> = Vec::new();
        let mut page = 1u32;
        let per_page = 100;

        loop {
            let url = format!("{}?per_page={}&page={}", url_base, per_page, page);
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

        let full_name = format!("{}/{}", owner, repo);
        let now = Utc::now();

        // 3. Fetch commit dates concurrently in batches of 10
        let batch_size = 10;
        let mut dated_branches: Vec<(GitHubBranchDetail, Option<DateTime<Utc>>, i64)> = Vec::new();
        for chunk in all_branches.chunks(batch_size) {
            let futs: Vec<_> = chunk.iter().map(|branch| {
                let client = self.clone();
                let owner = owner.to_string();
                let repo = repo.to_string();
                let sha = branch.commit.sha.clone();
                async move {
                    client.get_commit_date(&owner, &repo, &sha).await.unwrap_or(None)
                }
            }).collect();
            let dates = futures_util::future::join_all(futs).await;
            for (branch, commit_date) in chunk.iter().zip(dates) {
                let age_days = commit_date
                    .map(|d: DateTime<Utc>| (now - d).num_days())
                    .unwrap_or(0);
                dated_branches.push((branch.clone(), commit_date, age_days));
            }
        }

        // 4. Only check merge status for old, non-default, non-protected branches (concurrently)
        let mut details: Vec<BranchDetail> = Vec::new();
        let stale_candidates: Vec<_> = dated_branches.iter().enumerate().filter(|(_, (b, _, age))| {
            *age > 30 && b.name != default_branch && !b.protected
        }).map(|(i, _)| i).collect();

        // Batch merge checks concurrently (10 at a time)
        let mut merge_results: std::collections::HashMap<usize, bool> = std::collections::HashMap::new();
        for chunk in stale_candidates.chunks(batch_size) {
            let futs: Vec<_> = chunk.iter().map(|&idx| {
                let (ref branch, _, _) = dated_branches[idx];
                let client = self.clone();
                let owner = owner.to_string();
                let repo = repo.to_string();
                let default = default_branch.clone();
                let name = branch.name.clone();
                async move {
                    (idx, client.check_branch_merged(&owner, &repo, &default, &name).await.unwrap_or(false))
                }
            }).collect();
            let results = futures_util::future::join_all(futs).await;
            for (idx, merged) in results {
                merge_results.insert(idx, merged);
            }
        }

        // 5. Build final details
        for (i, (branch, commit_date, age_days)) in dated_branches.into_iter().enumerate() {
            let is_default = branch.name == default_branch;
            let is_merged = merge_results.get(&i).copied().unwrap_or(false);
            let safe_to_delete = is_merged && !branch.protected && !is_default && age_days > 30;

            details.push(BranchDetail {
                name: branch.name,
                repo: full_name.clone(),
                protected: branch.protected,
                last_commit_sha: branch.commit.sha,
                last_commit_date: commit_date,
                is_merged,
                is_default,
                age_days,
                safe_to_delete,
            });
        }

        Ok(details)
    }

    /// Delete a branch from a repository.
    /// Requires `repo` scope on PAT.
    #[allow(dead_code)]
    pub async fn delete_branch(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<(), DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/git/refs/heads/{}",
            self.api_url, owner, repo, branch
        );
        let response = self
            .client
            .delete(&url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;

        self.update_rate_limit(&response).await;

        let status = response.status();
        if status.is_success() || status == reqwest::StatusCode::NO_CONTENT {
            Ok(())
        } else {
            let text = response.text().await.unwrap_or_default();
            Err(DashboardError::GitHubApi(format!(
                "Delete branch failed ({}): {}",
                status, text
            )))
        }
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
        job_id: u64,
    ) -> Result<JobLogResponse, DashboardError> {
        let url = format!(
            "{}/repos/{}/{}/actions/jobs/{}/logs",
            self.api_url, owner, repo, job_id
        );

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
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
}

/// Strip ANSI escape codes from log text.
/// Handles CSI sequences (ESC [ ... letter) and also strips \r.
fn strip_ansi(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until we find a terminating ASCII letter
            for c2 in chars.by_ref() {
                if c2.is_ascii_alphabetic() {
                    break;
                }
            }
        } else if c == '\r' {
            // Strip carriage returns
        } else {
            result.push(c);
        }
    }
    result
}

/// Parse job log text into per-step segments using ##[group]/##[endgroup] markers.
/// Also strips timestamp prefixes (e.g., "2024-01-01T12:00:00.0000000Z ") from each line.
fn parse_step_logs(log_text: &str) -> Vec<JobLogStep> {
    let mut steps: Vec<JobLogStep> = Vec::new();
    let mut current_name = String::new();
    let mut current_lines: Vec<String> = Vec::new();
    let mut in_group = false;

    for line in log_text.lines() {
        // Strip timestamp prefix: "2024-01-01T12:00:00.0000000Z " (28-char ISO + space)
        let content = if line.len() > 28 && line.as_bytes()[4] == b'-' && line.as_bytes()[27] == b'Z' {
            line[29..].trim_start()
        } else {
            line
        };

        // Check for ##[group] marker
        if content.starts_with("##[group]") {
            // Save any pending lines
            if in_group && !current_lines.is_empty() {
                steps.push(JobLogStep {
                    name: current_name.clone(),
                    log: current_lines.join("\n"),
                });
                current_lines.clear();
            } else if !in_group && !current_lines.is_empty() {
                steps.push(JobLogStep {
                    name: "Preamble".into(),
                    log: current_lines.join("\n"),
                });
                current_lines.clear();
            }
            current_name = content[9..].to_string();
            in_group = true;
            continue;
        }

        // Check for ##[endgroup] marker
        if content.starts_with("##[endgroup]") {
            if in_group {
                steps.push(JobLogStep {
                    name: current_name.clone(),
                    log: current_lines.join("\n"),
                });
                current_lines.clear();
                in_group = false;
            }
            continue;
        }

        // Strip other ## directives but keep content
        let clean = if content.starts_with("##[debug]") {
            &content[9..]
        } else if content.starts_with("##[warning]") {
            &content[11..]
        } else if content.starts_with("##[error]") {
            &content[9..]
        } else if content.starts_with("##[command]") {
            content[11..].trim_start()
        } else {
            content
        };

        current_lines.push(clean.to_string());
    }

    // Flush remaining lines
    if !current_lines.is_empty() {
        let name = if in_group {
            current_name
        } else if steps.is_empty() {
            "Full Log".into()
        } else {
            "Post".into()
        };
        steps.push(JobLogStep {
            name,
            log: current_lines.join("\n"),
        });
    }

    // If no steps at all, return a single empty entry
    if steps.is_empty() {
        steps.push(JobLogStep {
            name: "Full Log".into(),
            log: String::new(),
        });
    }

    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_basic() {
        let input = "\x1b[32mHello\x1b[0m World";
        assert_eq!(strip_ansi(input), "Hello World");
    }

    #[test]
    fn test_strip_ansi_no_codes() {
        let input = "Just plain text";
        assert_eq!(strip_ansi(input), "Just plain text");
    }

    #[test]
    fn test_strip_ansi_carriage_return() {
        let input = "line1\r\nline2\r";
        assert_eq!(strip_ansi(input), "line1\nline2");
    }

    #[test]
    fn test_parse_step_logs_with_groups() {
        let log = "2024-01-01T12:00:00.0000000Z ##[group]Set up job\n\
                    2024-01-01T12:00:01.0000000Z Preparing environment\n\
                    2024-01-01T12:00:02.0000000Z ##[endgroup]\n\
                    2024-01-01T12:00:03.0000000Z ##[group]Run tests\n\
                    2024-01-01T12:00:04.0000000Z test passed\n\
                    2024-01-01T12:00:05.0000000Z ##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].name, "Set up job");
        assert_eq!(steps[0].log, "Preparing environment");
        assert_eq!(steps[1].name, "Run tests");
        assert_eq!(steps[1].log, "test passed");
    }

    #[test]
    fn test_parse_step_logs_no_groups() {
        let log = "2024-01-01T12:00:00.0000000Z line 1\n\
                    2024-01-01T12:00:01.0000000Z line 2";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].name, "Full Log");
        assert!(steps[0].log.contains("line 1"));
        assert!(steps[0].log.contains("line 2"));
    }

    #[test]
    fn test_parse_step_logs_with_preamble() {
        let log = "2024-01-01T12:00:00.0000000Z preamble line\n\
                    2024-01-01T12:00:01.0000000Z ##[group]Step 1\n\
                    2024-01-01T12:00:02.0000000Z step content\n\
                    2024-01-01T12:00:03.0000000Z ##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].name, "Preamble");
        assert_eq!(steps[0].log, "preamble line");
        assert_eq!(steps[1].name, "Step 1");
        assert_eq!(steps[1].log, "step content");
    }

    #[test]
    fn test_parse_step_logs_strips_directives() {
        let log = "2024-01-01T12:00:00.0000000Z ##[group]Test\n\
                    2024-01-01T12:00:01.0000000Z ##[command]echo hello\n\
                    2024-01-01T12:00:02.0000000Z ##[warning]something\n\
                    2024-01-01T12:00:03.0000000Z ##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].name, "Test");
        assert!(steps[0].log.contains("echo hello"));
        assert!(steps[0].log.contains("something"));
    }

    #[test]
    fn test_parse_step_logs_empty() {
        let steps = parse_step_logs("");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].name, "Full Log");
    }

    #[test]
    fn test_strip_ansi_multiple_codes() {
        let input = "\x1b[1m\x1b[31mERROR\x1b[0m: \x1b[33mwarning\x1b[0m";
        assert_eq!(strip_ansi(input), "ERROR: warning");
    }

    #[test]
    fn test_strip_ansi_complex_csi() {
        // 256-color and RGB color codes
        let input = "\x1b[38;5;196mRed\x1b[0m \x1b[48;2;255;128;0mOrange\x1b[0m";
        assert_eq!(strip_ansi(input), "Red Orange");
    }

    #[test]
    fn test_strip_ansi_empty() {
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn test_strip_ansi_only_codes() {
        let input = "\x1b[32m\x1b[0m\x1b[1m\x1b[0m";
        assert_eq!(strip_ansi(input), "");
    }

    #[test]
    fn test_strip_ansi_preserves_newlines() {
        let input = "\x1b[32mline1\x1b[0m\nline2\n\x1b[31mline3\x1b[0m";
        assert_eq!(strip_ansi(input), "line1\nline2\nline3");
    }

    #[test]
    fn test_strip_ansi_mixed_cr_lf() {
        let input = "a\r\nb\rc\n";
        assert_eq!(strip_ansi(input), "a\nbc\n");
    }

    #[test]
    fn test_parse_step_logs_post_section() {
        // Lines after the last endgroup get a "Post" name
        let log = "2024-01-01T12:00:00.0000000Z ##[group]Build\n\
                    2024-01-01T12:00:01.0000000Z building...\n\
                    2024-01-01T12:00:02.0000000Z ##[endgroup]\n\
                    2024-01-01T12:00:03.0000000Z cleanup line";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].name, "Build");
        assert_eq!(steps[1].name, "Post");
        assert!(steps[1].log.contains("cleanup line"));
    }

    #[test]
    fn test_parse_step_logs_nested_groups() {
        // Consecutive groups without endgroup — second group closes first
        let log = "2024-01-01T12:00:00.0000000Z ##[group]Step A\n\
                    2024-01-01T12:00:01.0000000Z a content\n\
                    2024-01-01T12:00:02.0000000Z ##[group]Step B\n\
                    2024-01-01T12:00:03.0000000Z b content\n\
                    2024-01-01T12:00:04.0000000Z ##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].name, "Step A");
        assert!(steps[0].log.contains("a content"));
        assert_eq!(steps[1].name, "Step B");
        assert!(steps[1].log.contains("b content"));
    }

    #[test]
    fn test_parse_step_logs_debug_directive() {
        let log = "2024-01-01T12:00:00.0000000Z ##[group]Test\n\
                    2024-01-01T12:00:01.0000000Z ##[debug]debug message\n\
                    2024-01-01T12:00:02.0000000Z ##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 1);
        assert!(steps[0].log.contains("debug message"));
    }

    #[test]
    fn test_parse_step_logs_error_directive() {
        let log = "2024-01-01T12:00:00.0000000Z ##[group]Build\n\
                    2024-01-01T12:00:01.0000000Z ##[error]compilation failed\n\
                    2024-01-01T12:00:02.0000000Z ##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 1);
        assert!(steps[0].log.contains("compilation failed"));
    }

    #[test]
    fn test_parse_step_logs_no_timestamp() {
        // Lines without timestamp prefix should still be parsed
        let log = "##[group]Direct Step\nsome output\n##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].name, "Direct Step");
        assert!(steps[0].log.contains("some output"));
    }

    #[test]
    fn test_parse_step_logs_multiline_content() {
        let log = "2024-01-01T12:00:00.0000000Z ##[group]Multi\n\
                    2024-01-01T12:00:01.0000000Z line 1\n\
                    2024-01-01T12:00:02.0000000Z line 2\n\
                    2024-01-01T12:00:03.0000000Z line 3\n\
                    2024-01-01T12:00:04.0000000Z ##[endgroup]";
        let steps = parse_step_logs(log);
        assert_eq!(steps.len(), 1);
        let lines: Vec<&str> = steps[0].log.lines().collect();
        assert_eq!(lines.len(), 3);
    }

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
