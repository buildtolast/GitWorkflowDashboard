//! Workflow run persistence — upsert and prune operations.

use rusqlite::params;

use crate::error::DashboardError;

use super::Database;

/// Maximum number of runs to keep per repo (prune old ones).
const MAX_RUNS_PER_REPO: u32 = 5000;

impl Database {
    /// Upsert a batch of workflow runs for a given repo in a single transaction.
    pub fn upsert_runs(&self, repo_owner: &str, repo_name: &str, runs: &[crate::models::WorkflowRun]) -> Result<(), DashboardError> {
        let conn = self.conn()?;

        // Auto-insert the repo if needed
        let repo_id: i64 = match conn.query_row(
            "SELECT id FROM repos WHERE owner = ?1 AND name = ?2",
            params![repo_owner, repo_name],
            |row| row.get(0),
        ) {
            Ok(id) => id,
            Err(_) => {
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO repos (owner, name) VALUES (?1, ?2)",
                    params![repo_owner, repo_name],
                );
                match conn.query_row(
                    "SELECT id FROM repos WHERE owner = ?1 AND name = ?2",
                    params![repo_owner, repo_name],
                    |row| row.get(0),
                ) {
                    Ok(id) => id,
                    Err(_) => return Ok(()),
                }
            }
        };

        let tx = match conn.unchecked_transaction() {
            Ok(tx) => tx,
            Err(e) => {
                tracing::error!(error = %e, "Failed to begin upsert transaction");
                return Ok(());
            }
        };

        for run in runs {
            let actor = run.actor.as_ref().map(|a| a.login.as_str());
            let started_at = run.run_started_at.map(|t| t.to_rfc3339());
            let conclusion_str = run.conclusion.as_deref();

            // Calculate duration if both start and complete times exist
            let duration_secs: Option<i64> = if let (Some(start), Some(conclusion)) =
                (run.run_started_at, run.conclusion.as_deref())
            {
                // Only calculate for completed runs
                if conclusion != "null" {
                    run.updated_at.signed_duration_since(start).num_seconds().into()
                } else {
                    None
                }
            } else {
                None
            };

            let _ = tx.execute(
                "INSERT INTO workflow_runs (id, repo_id, workflow_name, workflow_id, status, conclusion, actor, event, branch, head_sha, run_number, started_at, completed_at, duration_secs, created_at, updated_at, html_url)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
                 ON CONFLICT(id) DO UPDATE SET
                    status = excluded.status,
                    conclusion = excluded.conclusion,
                    updated_at = excluded.updated_at,
                    completed_at = excluded.completed_at,
                    duration_secs = COALESCE(excluded.duration_secs, workflow_runs.duration_secs)",
                params![
                    run.id.0 as i64,
                    repo_id,
                    run.name.as_deref(),
                    run.workflow_id.0 as i64,
                    run.status.as_deref(),
                    conclusion_str,
                    actor,
                    run.event,
                    run.head_branch.as_deref(),
                    run.head_sha,
                    run.run_number as i64,
                    started_at,
                    if run.status.as_deref() == Some("completed") { Some(run.updated_at.to_rfc3339()) } else { None::<String> },
                    duration_secs,
                    run.created_at.to_rfc3339(),
                    run.updated_at.to_rfc3339(),
                    run.html_url,
                ],
            );
        }

        if let Err(e) = tx.commit() {
            tracing::error!(error = %e, "Failed to commit upsert transaction");
        }

        Ok(())
    }

    /// Prune old runs for a repo, keeping only the most recent MAX_RUNS_PER_REPO.
    pub fn prune_old_runs(&self, repo_owner: &str, repo_name: &str) -> Result<(), DashboardError> {
        if let Some(repo_id) = self.get_repo_id(repo_owner, repo_name)? {
            let conn = self.conn()?;
            let _ = conn.execute(
                "DELETE FROM workflow_runs WHERE repo_id = ?1 AND id NOT IN (
                    SELECT id FROM workflow_runs WHERE repo_id = ?1
                    ORDER BY created_at DESC LIMIT ?2
                )",
                params![repo_id, MAX_RUNS_PER_REPO],
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

    #[test]
    fn test_upsert_runs() {
        let (_tmp, db) = make_test_db();
        db.save_repos(&[make_repo_ref("owner", "repo")]);

        let runs = vec![
            make_test_run(1, "CI", Some("success"), "push", Some("octocat")),
            make_test_run(2, "Deploy", Some("failure"), "push", Some("alice")),
        ];

        db.upsert_runs("owner", "repo", &runs).unwrap();

        // Verify via analytics
        let since = chrono::Utc::now() - chrono::Duration::hours(1);
        let stats = db.analytics_success_rate(&since).unwrap();
        assert_eq!(stats.total_runs, 2);
        assert_eq!(stats.successful_runs, 1);
    }

    #[test]
    fn test_upsert_runs_updates_existing() {
        let (_tmp, db) = make_test_db();
        db.save_repos(&[make_repo_ref("owner", "repo")]);

        // Insert in-progress run
        let runs = vec![make_test_run(1, "CI", None, "push", Some("octocat"))];
        db.upsert_runs("owner", "repo", &runs).unwrap();

        // Update to completed
        let runs = vec![make_test_run(1, "CI", Some("success"), "push", Some("octocat"))];
        db.upsert_runs("owner", "repo", &runs).unwrap();

        let since = chrono::Utc::now() - chrono::Duration::hours(1);
        let stats = db.analytics_success_rate(&since).unwrap();
        assert_eq!(stats.total_runs, 1);
        assert_eq!(stats.successful_runs, 1);
    }
}
