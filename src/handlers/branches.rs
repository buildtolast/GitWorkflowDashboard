use std::sync::Arc;

use actix_web::{web, HttpResponse};
use chrono::Utc;

use crate::error::DashboardError;
use crate::github::client::GitHubClient;
use crate::models::*;

/// GET /api/branches/stale — fetch stale/merged branches across all monitored repos.
pub async fn get_stale_branches(
    client: web::Data<Arc<GitHubClient>>,
    repo_list: web::Data<SharedRepoList>,
) -> Result<HttpResponse, DashboardError> {
    let repos = repo_list.read().await;
    let repo_refs: Vec<_> = repos.clone();
    drop(repos);

    // Fetch branch details for all repos concurrently
    let mut futs: Vec<std::pin::Pin<Box<dyn std::future::Future<Output = Vec<BranchDetail>>>>> =
        Vec::new();
    for repo_ref in &repo_refs {
        let client = client.clone();
        let owner = repo_ref.owner.clone();
        let repo = repo_ref.repo.clone();
        futs.push(Box::pin(async move {
            match client.get_branch_details(&owner, &repo).await {
                Ok(branches) => branches,
                Err(e) => {
                    tracing::warn!(
                        repo = %format!("{}/{}", owner, repo),
                        error = %e,
                        "Failed to fetch branch details"
                    );
                    Vec::new()
                }
            }
        }));
    }

    let all_results = futures_util::future::join_all(futs).await;
    let mut all_branches: Vec<BranchDetail> = all_results.into_iter().flatten().collect();

    // Filter: only branches older than 30 days
    all_branches.retain(|b| b.age_days > 30);

    // Sort by age descending (oldest first)
    all_branches.sort_by(|a, b| b.age_days.cmp(&a.age_days));

    // Generate cleanup suggestions
    let suggestions = generate_cleanup_suggestions(&all_branches, &repo_refs);

    let total = all_branches.len();

    Ok(HttpResponse::Ok().json(StaleBranchesResponse {
        total,
        branches: all_branches,
        cleanup_suggestions: suggestions,
    }))
}

/// DELETE /api/branches/{owner}/{repo}/{branch} — delete a branch.
pub async fn delete_branch(
    client: web::Data<Arc<GitHubClient>>,
    path: web::Path<(String, String, String)>,
) -> Result<HttpResponse, DashboardError> {
    let (owner, repo, branch) = path.into_inner();

    // Validate owner/repo format
    let repo_ref = format!("{}/{}", owner, repo);
    if let Err(e) = crate::config::validate_repo_ref(&repo_ref) {
        return Ok(HttpResponse::BadRequest().json(ActionResponse {
            success: false,
            message: e,
        }));
    }

    // Validate branch name
    if let Err(e) = crate::config::validate_branch_name(&branch) {
        return Ok(HttpResponse::BadRequest().json(ActionResponse {
            success: false,
            message: e,
        }));
    }

    client.delete_branch(&owner, &repo, &branch).await?;

    Ok(HttpResponse::Ok().json(ActionResponse {
        success: true,
        message: format!("Branch '{}' deleted from {}/{}", branch, owner, repo),
    }))
}

/// Generate cleanup suggestions based on branch analysis.
fn generate_cleanup_suggestions(
    branches: &[BranchDetail],
    repos: &[crate::config::RepoRef],
) -> Vec<CleanupSuggestion> {
    let mut suggestions = Vec::new();
    let now = Utc::now();
    let _ = now; // suppress unused warning

    // Count merged branches that are safe to delete
    let safe_to_delete: Vec<_> = branches.iter().filter(|b| b.safe_to_delete).collect();
    if !safe_to_delete.is_empty() {
        suggestions.push(CleanupSuggestion {
            category: "branches".into(),
            title: format!(
                "{} merged branch{} can be safely deleted",
                safe_to_delete.len(),
                if safe_to_delete.len() == 1 { "" } else { "es" }
            ),
            description: format!(
                "These branches have been merged into the default branch and are older than 30 days. \
                 Deleting them reduces clutter and makes branch navigation easier."
            ),
            impact: "low-risk".into(),
        });
    }

    // Count very old branches (6+ months)
    let very_old: Vec<_> = branches.iter().filter(|b| b.age_days > 180).collect();
    if !very_old.is_empty() {
        suggestions.push(CleanupSuggestion {
            category: "branches".into(),
            title: format!(
                "{} branch{} haven't been updated in 6+ months",
                very_old.len(),
                if very_old.len() == 1 { "" } else { "es" }
            ),
            description: "Stale branches may indicate abandoned work. Consider reviewing \
                          these branches and archiving or deleting them if no longer needed."
                .into(),
            impact: "medium".into(),
        });
    }

    // Check for unprotected default branches
    let unprotected_defaults: Vec<_> = branches
        .iter()
        .filter(|b| b.is_default && !b.protected)
        .collect();
    if !unprotected_defaults.is_empty() {
        let repo_names: Vec<_> = unprotected_defaults.iter().map(|b| b.repo.as_str()).collect();
        suggestions.push(CleanupSuggestion {
            category: "security".into(),
            title: "Consider enabling branch protection on default branches".into(),
            description: format!(
                "The following repos have unprotected default branches: {}. \
                 Branch protection prevents accidental force-pushes and requires review before merging.",
                repo_names.join(", ")
            ),
            impact: "high".into(),
        });
    }

    // Suggest auto-delete for repos with many merged branches
    let repos_with_many_merged: std::collections::HashMap<&str, usize> = safe_to_delete
        .iter()
        .fold(std::collections::HashMap::new(), |mut acc, b| {
            *acc.entry(b.repo.as_str()).or_insert(0) += 1;
            acc
        });
    let heavy_repos: Vec<_> = repos_with_many_merged
        .iter()
        .filter(|(_, count)| **count >= 5)
        .collect();
    if !heavy_repos.is_empty() {
        suggestions.push(CleanupSuggestion {
            category: "automation".into(),
            title: "Enable auto-delete head branches in repo settings".into(),
            description: "Some repos have many merged branches accumulating. \
                          Enable 'Automatically delete head branches' in each repo's \
                          Settings > General to auto-clean merged PR branches."
                .into(),
            impact: "low-risk".into(),
        });
    }

    // General suggestion about workflow concurrency
    if repos.len() > 3 {
        suggestions.push(CleanupSuggestion {
            category: "cost".into(),
            title: "Review workflow concurrency settings".into(),
            description: "With multiple repos, consider adding `concurrency` groups to \
                          workflow files to cancel redundant runs on the same branch. \
                          This reduces GitHub Actions minutes and speeds up feedback."
                .into(),
            impact: "medium".into(),
        });
    }

    suggestions
}
