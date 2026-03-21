//! SQLite persistence layer — replaces flat JSON for repos + stores historical run data.
//!
//! Database path: `~/.gwd/dashboard.db`
//!
//! Uses WAL mode for concurrent reads during polling.
//! Auto-migrates from `repos.json` on first run (if file exists).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqliteResult};

use crate::config::RepoRef;

/// Database filename.
const DB_FILENAME: &str = "dashboard.db";

/// Legacy JSON repos file (for auto-migration).
const LEGACY_REPOS_FILENAME: &str = "repos.json";

/// Maximum number of runs to keep per repo (prune old ones).
#[allow(dead_code)]
const MAX_RUNS_PER_REPO: u32 = 5000;

/// SQLite database handle for the dashboard.
///
/// Thread-safe: the inner `Connection` is protected by a `Mutex`.
pub struct Database {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl Database {
    /// Open (or create) the database at the given data directory.
    /// Runs migrations and enables WAL mode.
    pub fn open(data_dir: &Path) -> SqliteResult<Self> {
        let path = data_dir.join(DB_FILENAME);
        let conn = Connection::open(&path)?;

        // Enable WAL mode for concurrent reads during polling
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // Reasonable busy timeout for concurrent access
        conn.pragma_update(None, "busy_timeout", 5000)?;
        // Enable foreign keys
        conn.pragma_update(None, "foreign_keys", "ON")?;

        let mut db = Self { conn: Mutex::new(conn), path };
        db.run_migrations()?;
        db.auto_migrate_from_json(data_dir);

        tracing::info!(path = %db.path.display(), "Database opened");
        Ok(db)
    }

    /// Run schema migrations (idempotent — safe to call on every startup).
    fn run_migrations(&mut self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS repos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                added_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(owner, name)
            );

            CREATE TABLE IF NOT EXISTS workflow_runs (
                id INTEGER PRIMARY KEY,
                repo_id INTEGER NOT NULL,
                workflow_name TEXT,
                workflow_id INTEGER,
                status TEXT,
                conclusion TEXT,
                actor TEXT,
                event TEXT,
                branch TEXT,
                head_sha TEXT,
                run_number INTEGER,
                started_at TEXT,
                completed_at TEXT,
                duration_secs INTEGER,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                html_url TEXT,
                FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS analytics_cache (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                computed_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_runs_repo ON workflow_runs(repo_id);
            CREATE INDEX IF NOT EXISTS idx_runs_conclusion ON workflow_runs(conclusion);
            CREATE INDEX IF NOT EXISTS idx_runs_started ON workflow_runs(started_at);
            CREATE INDEX IF NOT EXISTS idx_runs_event ON workflow_runs(event);
            CREATE INDEX IF NOT EXISTS idx_runs_actor ON workflow_runs(actor);
            CREATE INDEX IF NOT EXISTS idx_runs_workflow ON workflow_runs(workflow_id);
            ",
        )?;
        Ok(())
    }

    /// Auto-migrate repos from legacy repos.json if it exists.
    fn auto_migrate_from_json(&self, data_dir: &Path) {
        let json_path = data_dir.join(LEGACY_REPOS_FILENAME);
        if !json_path.exists() {
            return;
        }

        let conn = self.conn.lock().unwrap();

        // Check if we already have repos in the database
        let count: u32 = conn
            .query_row("SELECT COUNT(*) FROM repos", [], |row| row.get(0))
            .unwrap_or(0);

        if count > 0 {
            // Already have repos — don't re-migrate
            return;
        }

        // Read and parse the JSON file
        match std::fs::read_to_string(&json_path) {
            Ok(contents) => match serde_json::from_str::<Vec<RepoRef>>(&contents) {
                Ok(repos) if !repos.is_empty() => {
                    let migrated = repos.len();
                    for repo in &repos {
                        let _ = conn.execute(
                            "INSERT OR IGNORE INTO repos (owner, name) VALUES (?1, ?2)",
                            params![repo.owner, repo.repo],
                        );
                    }
                    tracing::info!(
                        count = migrated,
                        from = %json_path.display(),
                        "Auto-migrated repos from JSON to SQLite"
                    );
                    // Rename old file to .bak so we don't re-migrate
                    let backup = json_path.with_extension("json.bak");
                    if let Err(e) = std::fs::rename(&json_path, &backup) {
                        tracing::warn!(error = %e, "Failed to rename legacy repos.json to .bak");
                    }
                }
                Ok(_) => {} // empty list, nothing to migrate
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to parse legacy repos.json during migration");
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "Failed to read legacy repos.json during migration");
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════
    //   REPO OPERATIONS
    // ═══════════════════════════════════════════════════════════════

    /// Load all repos from the database.
    pub fn load_repos(&self) -> Vec<RepoRef> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn
            .prepare("SELECT owner, name FROM repos ORDER BY id")
        {
            Ok(stmt) => stmt,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to prepare load_repos query");
                return vec![];
            }
        };

        let rows = stmt.query_map([], |row| {
            Ok(RepoRef {
                owner: row.get(0)?,
                repo: row.get(1)?,
            })
        });

        match rows {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load repos from database");
                vec![]
            }
        }
    }

    /// Save repos to the database (full replacement — deletes removed, inserts new).
    pub fn save_repos(&self, repos: &[RepoRef]) {
        // Use a transaction for atomicity
        let conn = self.conn.lock().unwrap();
        let tx = match conn.unchecked_transaction() {
            Ok(tx) => tx,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to start transaction for save_repos");
                return;
            }
        };

        // Get current repo set
        let current: Vec<(i64, String, String)> = {
            let mut stmt = match tx.prepare("SELECT id, owner, name FROM repos") {
                Ok(stmt) => stmt,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to prepare repos query in save_repos");
                    return;
                }
            };
            let current: Vec<(i64, String, String)> = match stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))) {
                Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
                Err(e) => {
                    tracing::error!(error = %e, "Failed to query repos in save_repos");
                    return;
                }
            };
            current
        };

        // Delete repos no longer in the list
        for (id, owner, name) in &current {
            let still_present = repos
                .iter()
                .any(|r| &r.owner == owner && &r.repo == name);
            if !still_present {
                let _ = tx.execute("DELETE FROM repos WHERE id = ?1", params![id]);
            }
        }

        // Insert new repos
        for repo in repos {
            let _ = tx.execute(
                "INSERT OR IGNORE INTO repos (owner, name) VALUES (?1, ?2)",
                params![repo.owner, repo.repo],
            );
        }

        if let Err(e) = tx.commit() {
            tracing::warn!(error = %e, "Failed to commit save_repos transaction");
        } else {
            tracing::debug!(count = repos.len(), "Saved repos to database");
        }
    }

    /// Get the internal repo ID for a given owner/name. Returns None if not found.
    pub fn get_repo_id(&self, owner: &str, name: &str) -> Option<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
                "SELECT id FROM repos WHERE owner = ?1 AND name = ?2",
                params![owner, name],
                |row| row.get(0),
            )
            .ok()
    }

    // ═══════════════════════════════════════════════════════════════
    //   WORKFLOW RUN OPERATIONS
    // ═══════════════════════════════════════════════════════════════

    /// Upsert a batch of workflow runs for a given repo.
    pub fn upsert_runs(&self, repo_owner: &str, repo_name: &str, runs: &[crate::models::WorkflowRun]) {
        let repo_id = match self.get_repo_id(repo_owner, repo_name) {
            Some(id) => id,
            None => {
                // Auto-insert the repo if needed (handles race conditions)
                let conn = self.conn.lock().unwrap();
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO repos (owner, name) VALUES (?1, ?2)",
                    params![repo_owner, repo_name],
                );
                match self.get_repo_id(repo_owner, repo_name) {
                    Some(id) => id,
                    None => return,
                }
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

            let conn = self.conn.lock().unwrap();
            let _ = conn.execute(
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
    }

    /// Prune old runs for a repo, keeping only the most recent MAX_RUNS_PER_REPO.
    #[allow(dead_code)]
    pub fn prune_old_runs(&self, repo_owner: &str, repo_name: &str) {
        if let Some(repo_id) = self.get_repo_id(repo_owner, repo_name) {
            let conn = self.conn.lock().unwrap();
            let _ = conn.execute(
                "DELETE FROM workflow_runs WHERE repo_id = ?1 AND id NOT IN (
                    SELECT id FROM workflow_runs WHERE repo_id = ?1
                    ORDER BY created_at DESC LIMIT ?2
                )",
                params![repo_id, MAX_RUNS_PER_REPO],
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════
    //   ANALYTICS QUERIES
    // ═══════════════════════════════════════════════════════════════

    /// Get success rate statistics for a time range.
    pub fn analytics_success_rate(&self, since: &DateTime<Utc>) -> SqliteResult<SuccessRateStats> {
        let since_str = since.to_rfc3339();

        // Overall success rate
        let (total, successes): (u32, u32) = {
            let conn = self.conn.lock().unwrap();
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
    pub fn analytics_duration(&self, since: &DateTime<Utc>) -> SqliteResult<DurationStats> {
        let conn = self.conn.lock().unwrap();
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
    pub fn analytics_triggers(&self, since: &DateTime<Utc>) -> SqliteResult<Vec<TriggerStat>> {
        let conn = self.conn.lock().unwrap();
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
    pub fn analytics_actors(&self, since: &DateTime<Utc>) -> SqliteResult<Vec<ActorStat>> {
        let conn = self.conn.lock().unwrap();
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
    pub fn analytics_failures(&self, since: &DateTime<Utc>) -> SqliteResult<FailureStats> {
        let conn = self.conn.lock().unwrap();
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
    pub fn analytics_branches(&self, since: &DateTime<Utc>) -> SqliteResult<Vec<BranchActivity>> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO analytics_cache (key, value, computed_at) VALUES (?1, ?2, datetime('now'))",
            params![key, value],
        );
    }

    /// Get the database file path.
    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    // ═══════════════════════════════════════════════════════════════
    //   HELPERS
    // ═══════════════════════════════════════════════════════════════

    /// Helper to execute success rate grouped queries.
    fn query_success_rate_grouped(&self, sql: &str, since: &str) -> SqliteResult<Vec<NamedSuccessRate>> {
        let conn = self.conn.lock().unwrap();
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

/// Shared database handle type alias.
/// Wrapped in Option to handle cases where the data directory is unavailable.
pub type SharedDatabase = std::sync::Arc<Option<Database>>;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_test_db() -> (TempDir, Database) {
        let tmp = TempDir::new().unwrap();
        let db = Database::open(tmp.path()).unwrap();
        (tmp, db)
    }

    fn make_repo_ref(owner: &str, repo: &str) -> RepoRef {
        RepoRef {
            owner: owner.to_string(),
            repo: repo.to_string(),
        }
    }

    fn make_test_run(id: u64, name: &str, conclusion: Option<&str>, event: &str, actor: Option<&str>) -> crate::models::WorkflowRun {
        crate::models::WorkflowRun {
            id: crate::models::RunId(id),
            name: Some(name.to_string()),
            workflow_id: crate::models::WorkflowId(1),
            head_branch: Some("main".to_string()),
            head_sha: "abc123".to_string(),
            status: Some("completed".to_string()),
            conclusion: conclusion.map(|s| s.to_string()),
            event: event.to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            run_started_at: Some(chrono::Utc::now() - chrono::Duration::seconds(120)),
            html_url: format!("https://github.com/owner/repo/actions/runs/{}", id),
            run_number: id,
            run_attempt: Some(1),
            display_title: Some(name.to_string()),
            actor: actor.map(|login| crate::models::Actor {
                login: login.to_string(),
                id: 1,
                avatar_url: None,
            }),
            repo: "owner/repo".to_string(),
        }
    }

    #[test]
    fn test_open_creates_database() {
        let (tmp, db) = make_test_db();
        assert!(tmp.path().join(DB_FILENAME).exists());
        assert_eq!(db.path(), tmp.path().join(DB_FILENAME));
    }

    #[test]
    fn test_save_and_load_repos() {
        let (_tmp, db) = make_test_db();
        let repos = vec![
            make_repo_ref("octocat", "hello-world"),
            make_repo_ref("rust-lang", "rust"),
        ];

        db.save_repos(&repos);
        let loaded = db.load_repos();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].full_name(), "octocat/hello-world");
        assert_eq!(loaded[1].full_name(), "rust-lang/rust");
    }

    #[test]
    fn test_save_repos_removes_deleted() {
        let (_tmp, db) = make_test_db();
        let repos = vec![
            make_repo_ref("a", "one"),
            make_repo_ref("b", "two"),
        ];
        db.save_repos(&repos);
        assert_eq!(db.load_repos().len(), 2);

        // Remove one
        db.save_repos(&[make_repo_ref("a", "one")]);
        let loaded = db.load_repos();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].full_name(), "a/one");
    }

    #[test]
    fn test_load_empty_repos() {
        let (_tmp, db) = make_test_db();
        assert!(db.load_repos().is_empty());
    }

    #[test]
    fn test_upsert_runs() {
        let (_tmp, db) = make_test_db();
        db.save_repos(&[make_repo_ref("owner", "repo")]);

        let runs = vec![
            make_test_run(1, "CI", Some("success"), "push", Some("octocat")),
            make_test_run(2, "Deploy", Some("failure"), "push", Some("alice")),
        ];

        db.upsert_runs("owner", "repo", &runs);

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
        db.upsert_runs("owner", "repo", &runs);

        // Update to completed
        let runs = vec![make_test_run(1, "CI", Some("success"), "push", Some("octocat"))];
        db.upsert_runs("owner", "repo", &runs);

        let since = chrono::Utc::now() - chrono::Duration::hours(1);
        let stats = db.analytics_success_rate(&since).unwrap();
        assert_eq!(stats.total_runs, 1);
        assert_eq!(stats.successful_runs, 1);
    }

    #[test]
    fn test_analytics_triggers() {
        let (_tmp, db) = make_test_db();
        db.save_repos(&[make_repo_ref("owner", "repo")]);

        let runs = vec![
            make_test_run(1, "CI", Some("success"), "push", None),
            make_test_run(2, "CI", Some("success"), "push", None),
            make_test_run(3, "CI", Some("failure"), "pull_request", None),
        ];
        db.upsert_runs("owner", "repo", &runs);

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
        db.upsert_runs("owner", "repo", &runs);

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
        db.upsert_runs("owner", "repo", &runs);

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
        db.upsert_runs("owner", "repo", &runs);

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

    #[test]
    fn test_auto_migrate_from_json() {
        let tmp = TempDir::new().unwrap();

        // Create a legacy repos.json
        let repos = vec![
            make_repo_ref("octocat", "hello-world"),
            make_repo_ref("rust-lang", "rust"),
        ];
        let json = serde_json::to_string_pretty(&repos).unwrap();
        std::fs::write(tmp.path().join(LEGACY_REPOS_FILENAME), json).unwrap();

        // Open database — should auto-migrate
        let db = Database::open(tmp.path()).unwrap();
        let loaded = db.load_repos();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].full_name(), "octocat/hello-world");

        // Legacy file should be renamed to .bak
        assert!(!tmp.path().join(LEGACY_REPOS_FILENAME).exists());
        assert!(tmp.path().join("repos.json.bak").exists());
    }

    #[test]
    fn test_get_repo_id() {
        let (_tmp, db) = make_test_db();
        db.save_repos(&[make_repo_ref("owner", "repo")]);

        let id = db.get_repo_id("owner", "repo");
        assert!(id.is_some());

        let missing = db.get_repo_id("no", "such");
        assert!(missing.is_none());
    }
}
