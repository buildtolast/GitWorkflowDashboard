//! Analytics queries and cache operations.

use chrono::{DateTime, Utc};
use rusqlite::params;

use crate::error::DashboardError;

use super::Database;

impl Database {
    /// Get success rate statistics for a time range.
    pub fn analytics_success_rate(&self, since: &DateTime<Utc>) -> Result<SuccessRateStats, DashboardError> {
        let since_str = since.to_rfc3339();

        // Overall success rate
        let (total, successes): (u32, u32) = {
            let conn = self.conn()?;
            conn.query_row(
                "SELECT COUNT(*), SUM(CASE WHEN conclusion = 'success' THEN 1 ELSE 0 END)
                 FROM workflow_runs WHERE conclusion IS NOT NULL AND started_at >= ?1",
                params![since_str],
                |row| Ok((row.get(0)?, row.get::<_, u32>(1).unwrap_or(0))),
            )
            .unwrap_or((0, 0))
        };

        let overall_rate = if total > 0 {
            (successes as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        // Per-repo success rates
        let per_repo = self.query_success_rate_grouped(
            "SELECT r.owner || '/' || r.name as name,
                    COUNT(*) as total,
                    SUM(CASE WHEN wr.conclusion = 'success' THEN 1 ELSE 0 END) as successes
             FROM workflow_runs wr
             JOIN repos r ON wr.repo_id = r.id
             WHERE wr.conclusion IS NOT NULL AND wr.started_at >= ?1
             GROUP BY r.id
             ORDER BY (CAST(SUM(CASE WHEN wr.conclusion = 'success' THEN 1 ELSE 0 END) AS REAL) / COUNT(*)) ASC",
            &since_str,
        )?;

        // Per-workflow success rates
        let per_workflow = self.query_success_rate_grouped(
            "SELECT COALESCE(workflow_name, 'Unknown') as name,
                    COUNT(*) as total,
                    SUM(CASE WHEN conclusion = 'success' THEN 1 ELSE 0 END) as successes
             FROM workflow_runs
             WHERE conclusion IS NOT NULL AND started_at >= ?1
             GROUP BY workflow_name
             ORDER BY (CAST(SUM(CASE WHEN conclusion = 'success' THEN 1 ELSE 0 END) AS REAL) / COUNT(*)) ASC",
            &since_str,
        )?;

        Ok(SuccessRateStats {
            overall_rate,
            total_runs: total,
            successful_runs: successes,
            per_repo,
            per_workflow,
        })
    }

    /// Get duration analysis for a time range.
    pub fn analytics_duration(&self, since: &DateTime<Utc>) -> Result<DurationStats, DashboardError> {
        let conn = self.conn()?;
        let since_str = since.to_rfc3339();

        // Overall average and median
        let avg_duration: f64 = conn
            .query_row(
                "SELECT COALESCE(AVG(duration_secs), 0)
                 FROM workflow_runs
                 WHERE duration_secs IS NOT NULL AND duration_secs > 0 AND started_at >= ?1",
                params![since_str],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        let median_duration: f64 = conn
            .query_row(
                "SELECT COALESCE(duration_secs, 0) FROM (
                    SELECT duration_secs, ROW_NUMBER() OVER (ORDER BY duration_secs) as rn,
                           COUNT(*) OVER () as total
                    FROM workflow_runs
                    WHERE duration_secs IS NOT NULL AND duration_secs > 0 AND started_at >= ?1
                ) WHERE rn = (total + 1) / 2",
                params![since_str],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        // Per-workflow averages
        let mut stmt = conn
            .prepare(
                "SELECT COALESCE(workflow_name, 'Unknown'),
                        AVG(duration_secs),
                        COUNT(*)
                 FROM workflow_runs
                 WHERE duration_secs IS NOT NULL AND duration_secs > 0 AND started_at >= ?1
                 GROUP BY workflow_name
                 ORDER BY AVG(duration_secs) DESC",
            )?;

        let per_workflow: Vec<WorkflowDuration> = stmt
            .query_map(params![since_str], |row| {
                Ok(WorkflowDuration {
                    name: row.get(0)?,
                    avg_duration_secs: row.get(1)?,
                    run_count: row.get(2)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Slowest runs
        let mut stmt = conn
            .prepare(
                "SELECT wr.id, COALESCE(wr.workflow_name, 'Unknown'),
                        r.owner || '/' || r.name, wr.duration_secs, wr.started_at
                 FROM workflow_runs wr
                 JOIN repos r ON wr.repo_id = r.id
                 WHERE wr.duration_secs IS NOT NULL AND wr.duration_secs > 0 AND wr.started_at >= ?1
                 ORDER BY wr.duration_secs DESC
                 LIMIT 10",
            )?;

        let slowest_runs: Vec<SlowestRun> = stmt
            .query_map(params![since_str], |row| {
                Ok(SlowestRun {
                    run_id: row.get(0)?,
                    workflow_name: row.get(1)?,
                    repo: row.get(2)?,
                    duration_secs: row.get(3)?,
                    started_at: row.get(4)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(DurationStats {
            avg_duration_secs: avg_duration,
            median_duration_secs: median_duration,
            per_workflow,
            slowest_runs,
        })
    }

    /// Get trigger analysis for a time range.
    pub fn analytics_triggers(&self, since: &DateTime<Utc>) -> Result<Vec<TriggerStat>, DashboardError> {
        let conn = self.conn()?;
        let since_str = since.to_rfc3339();

        let mut stmt = conn
            .prepare(
                "SELECT event,
                        COUNT(*) as total,
                        SUM(CASE WHEN conclusion = 'success' THEN 1 ELSE 0 END) as successes
                 FROM workflow_runs
                 WHERE conclusion IS NOT NULL AND started_at >= ?1
                 GROUP BY event
                 ORDER BY total DESC",
            )?;

        Ok(stmt.query_map(params![since_str], |row| {
            let total: u32 = row.get(1)?;
            let successes: u32 = row.get(2)?;
            Ok(TriggerStat {
                event: row.get(0)?,
                total_runs: total,
                successful_runs: successes,
                success_rate: if total > 0 {
                    (successes as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
            })
        })?
        .filter_map(|r| r.ok())
        .collect())
    }

    /// Get actor leaderboard for a time range.
    pub fn analytics_actors(&self, since: &DateTime<Utc>) -> Result<Vec<ActorStat>, DashboardError> {
        let conn = self.conn()?;
        let since_str = since.to_rfc3339();

        let mut stmt = conn
            .prepare(
                "SELECT actor,
                        COUNT(*) as total,
                        SUM(CASE WHEN conclusion = 'success' THEN 1 ELSE 0 END) as successes,
                        SUM(CASE WHEN conclusion = 'failure' THEN 1 ELSE 0 END) as failures
                 FROM workflow_runs
                 WHERE actor IS NOT NULL AND conclusion IS NOT NULL AND started_at >= ?1
                 GROUP BY actor
                 ORDER BY total DESC
                 LIMIT 20",
            )?;

        Ok(stmt.query_map(params![since_str], |row| {
            let total: u32 = row.get(1)?;
            let successes: u32 = row.get(2)?;
            Ok(ActorStat {
                login: row.get(0)?,
                total_runs: total,
                successful_runs: successes,
                failed_runs: row.get(3)?,
                success_rate: if total > 0 {
                    (successes as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
            })
        })?
        .filter_map(|r| r.ok())
        .collect())
    }

    /// Get failure analysis for a time range.
    pub fn analytics_failures(&self, since: &DateTime<Utc>) -> Result<FailureStats, DashboardError> {
        let conn = self.conn()?;
        let since_str = since.to_rfc3339();

        // Most failing workflows
        let mut stmt = conn
            .prepare(
                "SELECT COALESCE(workflow_name, 'Unknown'),
                        COUNT(*) as failures,
                        r.owner || '/' || r.name as repo
                 FROM workflow_runs wr
                 JOIN repos r ON wr.repo_id = r.id
                 WHERE wr.conclusion = 'failure' AND wr.started_at >= ?1
                 GROUP BY wr.workflow_name, wr.repo_id
                 ORDER BY failures DESC
                 LIMIT 10",
            )?;

        let most_failing: Vec<FailingWorkflow> = stmt
            .query_map(params![since_str], |row| {
                Ok(FailingWorkflow {
                    workflow_name: row.get(0)?,
                    failure_count: row.get(1)?,
                    repo: row.get(2)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Recent failures
        let mut stmt = conn
            .prepare(
                "SELECT wr.id, COALESCE(wr.workflow_name, 'Unknown'),
                        r.owner || '/' || r.name, wr.started_at, wr.branch
                 FROM workflow_runs wr
                 JOIN repos r ON wr.repo_id = r.id
                 WHERE wr.conclusion = 'failure' AND wr.started_at >= ?1
                 ORDER BY wr.started_at DESC
                 LIMIT 20",
            )?;

        let recent_failures: Vec<RecentFailure> = stmt
            .query_map(params![since_str], |row| {
                Ok(RecentFailure {
                    run_id: row.get(0)?,
                    workflow_name: row.get(1)?,
                    repo: row.get(2)?,
                    started_at: row.get(3)?,
                    branch: row.get(4)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(FailureStats {
            most_failing,
            recent_failures,
        })
    }

    /// Get branch activity for a time range.
    pub fn analytics_branches(&self, since: &DateTime<Utc>) -> Result<Vec<BranchActivity>, DashboardError> {
        let conn = self.conn()?;
        let since_str = since.to_rfc3339();

        let mut stmt = conn
            .prepare(
                "SELECT COALESCE(branch, 'unknown'),
                        COUNT(*) as total,
                        SUM(CASE WHEN conclusion = 'success' THEN 1 ELSE 0 END),
                        SUM(CASE WHEN conclusion = 'failure' THEN 1 ELSE 0 END),
                        SUM(CASE WHEN conclusion = 'cancelled' THEN 1 ELSE 0 END)
                 FROM workflow_runs
                 WHERE conclusion IS NOT NULL AND started_at >= ?1
                 GROUP BY branch
                 ORDER BY total DESC
                 LIMIT 20",
            )?;

        Ok(stmt.query_map(params![since_str], |row| {
            Ok(BranchActivity {
                branch: row.get(0)?,
                total_runs: row.get(1)?,
                successes: row.get(2)?,
                failures: row.get(3)?,
                cancelled: row.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect())
    }

    // ═══════════════════════════════════════════════════════════════
    //   ANALYTICS CACHE
    // ═══════════════════════════════════════════════════════════════

    /// Get a cached analytics value if fresh enough.
    pub fn get_analytics_cache(&self, key: &str, max_age_secs: i64) -> Option<String> {
        let conn = match self.conn() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to acquire lock for get_analytics_cache");
                return None;
            }
        };
        conn.query_row(
                "SELECT value FROM analytics_cache
                 WHERE key = ?1
                 AND (julianday('now') - julianday(computed_at)) * 86400 < ?2",
                params![key, max_age_secs],
                |row| row.get(0),
            )
            .ok()
    }

    /// Set a cached analytics value.
    pub fn set_analytics_cache(&self, key: &str, value: &str) {
        let conn = match self.conn() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to acquire lock for set_analytics_cache");
                return;
            }
        };
        let _ = conn.execute(
            "INSERT OR REPLACE INTO analytics_cache (key, value, computed_at) VALUES (?1, ?2, datetime('now'))",
            params![key, value],
        );
    }

    /// Helper to execute success rate grouped queries.
    fn query_success_rate_grouped(&self, sql: &str, since: &str) -> Result<Vec<NamedSuccessRate>, DashboardError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(sql)?;
        Ok(stmt.query_map(params![since], |row| {
            let total: u32 = row.get(1)?;
            let successes: u32 = row.get(2)?;
            Ok(NamedSuccessRate {
                name: row.get(0)?,
                total_runs: total,
                successful_runs: successes,
                success_rate: if total > 0 {
                    (successes as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
            })
        })?
        .filter_map(|r| r.ok())
        .collect())
    }
}

// ═══════════════════════════════════════════════════════════════
//   ANALYTICS RESPONSE TYPES
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, serde::Serialize)]
pub struct SuccessRateStats {
    pub overall_rate: f64,
    pub total_runs: u32,
    pub successful_runs: u32,
    pub per_repo: Vec<NamedSuccessRate>,
    pub per_workflow: Vec<NamedSuccessRate>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NamedSuccessRate {
    pub name: String,
    pub total_runs: u32,
    pub successful_runs: u32,
    pub success_rate: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DurationStats {
    pub avg_duration_secs: f64,
    pub median_duration_secs: f64,
    pub per_workflow: Vec<WorkflowDuration>,
    pub slowest_runs: Vec<SlowestRun>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkflowDuration {
    pub name: String,
    pub avg_duration_secs: f64,
    pub run_count: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SlowestRun {
    pub run_id: i64,
    pub workflow_name: String,
    pub repo: String,
    pub duration_secs: i64,
    pub started_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TriggerStat {
    pub event: String,
    pub total_runs: u32,
    pub successful_runs: u32,
    pub success_rate: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ActorStat {
    pub login: String,
    pub total_runs: u32,
    pub successful_runs: u32,
    pub failed_runs: u32,
    pub success_rate: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FailureStats {
    pub most_failing: Vec<FailingWorkflow>,
    pub recent_failures: Vec<RecentFailure>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FailingWorkflow {
    pub workflow_name: String,
    pub failure_count: u32,
    pub repo: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RecentFailure {
    pub run_id: i64,
    pub workflow_name: String,
    pub repo: String,
    pub started_at: String,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BranchActivity {
    pub branch: String,
    pub total_runs: u32,
    pub successes: u32,
    pub failures: u32,
    pub cancelled: u32,
}

/// Full analytics summary response.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AnalyticsSummary {
    pub time_range: String,
    pub success_rate: SuccessRateStats,
    pub duration: DurationStats,
    pub triggers: Vec<TriggerStat>,
    pub actors: Vec<ActorStat>,
    pub failures: FailureStats,
    pub branches: Vec<BranchActivity>,
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

    #[test]
    fn test_analytics_triggers() {
        let (_tmp, db) = make_test_db();
        db.save_repos(&[make_repo_ref("owner", "repo")]);

        let runs = vec![
            make_test_run(1, "CI", Some("success"), "push", None),
            make_test_run(2, "CI", Some("success"), "push", None),
            make_test_run(3, "CI", Some("failure"), "pull_request", None),
        ];
        db.upsert_runs("owner", "repo", &runs).unwrap();

        let since = chrono::Utc::now() - chrono::Duration::hours(1);
        let triggers = db.analytics_triggers(&since).unwrap();

        assert_eq!(triggers.len(), 2);
        assert_eq!(triggers[0].event, "push"); // 2 runs, sorted by count DESC
        assert_eq!(triggers[0].total_runs, 2);
    }

    #[test]
    fn test_analytics_actors() {
        let (_tmp, db) = make_test_db();
        db.save_repos(&[make_repo_ref("owner", "repo")]);

        let runs = vec![
            make_test_run(1, "CI", Some("success"), "push", Some("alice")),
            make_test_run(2, "CI", Some("success"), "push", Some("alice")),
            make_test_run(3, "CI", Some("failure"), "push", Some("bob")),
        ];
        db.upsert_runs("owner", "repo", &runs).unwrap();

        let since = chrono::Utc::now() - chrono::Duration::hours(1);
        let actors = db.analytics_actors(&since).unwrap();

        assert_eq!(actors.len(), 2);
        assert_eq!(actors[0].login, "alice");
        assert_eq!(actors[0].total_runs, 2);
    }

    #[test]
    fn test_analytics_failures() {
        let (_tmp, db) = make_test_db();
        db.save_repos(&[make_repo_ref("owner", "repo")]);

        let runs = vec![
            make_test_run(1, "CI", Some("failure"), "push", None),
            make_test_run(2, "Deploy", Some("failure"), "push", None),
            make_test_run(3, "Deploy", Some("failure"), "push", None),
        ];
        db.upsert_runs("owner", "repo", &runs).unwrap();

        let since = chrono::Utc::now() - chrono::Duration::hours(1);
        let failures = db.analytics_failures(&since).unwrap();

        assert_eq!(failures.most_failing.len(), 2);
        assert_eq!(failures.most_failing[0].workflow_name, "Deploy");
        assert_eq!(failures.most_failing[0].failure_count, 2);
        assert_eq!(failures.recent_failures.len(), 3);
    }

    #[test]
    fn test_analytics_branches() {
        let (_tmp, db) = make_test_db();
        db.save_repos(&[make_repo_ref("owner", "repo")]);

        let runs = vec![
            make_test_run(1, "CI", Some("success"), "push", None),
            make_test_run(2, "CI", Some("failure"), "push", None),
        ];
        db.upsert_runs("owner", "repo", &runs).unwrap();

        let since = chrono::Utc::now() - chrono::Duration::hours(1);
        let branches = db.analytics_branches(&since).unwrap();

        assert!(!branches.is_empty());
        assert_eq!(branches[0].branch, "main");
    }

    #[test]
    fn test_analytics_cache() {
        let (_tmp, db) = make_test_db();

        db.set_analytics_cache("test_key", r#"{"value": 42}"#);
        let cached = db.get_analytics_cache("test_key", 60);
        assert!(cached.is_some());
        assert!(cached.unwrap().contains("42"));

        let missing = db.get_analytics_cache("nonexistent", 60);
        assert!(missing.is_none());
    }
}
