//! SQLite persistence layer — replaces flat JSON for repos + stores historical run data.
//!
//! Database path: `~/.gwd/dashboard.db`
//!
//! Uses WAL mode for concurrent reads during polling.
//! Auto-migrates from `repos.json` on first run (if file exists).

mod analytics;
mod repos;
mod runs;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, Result as SqliteResult};

use crate::config::RepoRef;
use crate::error::DashboardError;

// Re-export analytics types for external use.
pub use analytics::*;

/// Database filename.
const DB_FILENAME: &str = "dashboard.db";

/// Legacy JSON repos file (for auto-migration).
const LEGACY_REPOS_FILENAME: &str = "repos.json";

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

    /// Acquire the database connection, mapping mutex poison to DashboardError.
    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, DashboardError> {
        self.conn.lock().map_err(|e| {
            tracing::error!("Database mutex poisoned: {}", e);
            DashboardError::Config("Database connection unavailable (mutex poisoned)".into())
        })
    }

    /// Run schema migrations (idempotent — safe to call on every startup).
    fn run_migrations(&mut self) -> SqliteResult<()> {
        // Safe to unwrap here: called only once during open(), before any concurrent access.
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

        let conn = match self.conn() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = %e, "Cannot auto-migrate: database mutex poisoned");
                return;
            }
        };

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
                            rusqlite::params![repo.owner, repo.repo],
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

    /// Get the database file path (test-only).
    #[cfg(test)]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Shared database handle type alias.
/// Wrapped in Option to handle cases where the data directory is unavailable.
pub type SharedDatabase = std::sync::Arc<Option<Database>>;

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use tempfile::TempDir;

    pub fn make_test_db() -> (TempDir, Database) {
        let tmp = TempDir::new().unwrap();
        let db = Database::open(tmp.path()).unwrap();
        (tmp, db)
    }

    pub fn make_repo_ref(owner: &str, repo: &str) -> RepoRef {
        RepoRef {
            owner: owner.to_string(),
            repo: repo.to_string(),
        }
    }

    pub fn make_test_run(id: u64, name: &str, conclusion: Option<&str>, event: &str, actor: Option<&str>) -> crate::models::WorkflowRun {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_helpers::*;
    use tempfile::TempDir;

    #[test]
    fn test_open_creates_database() {
        let (tmp, db) = make_test_db();
        assert!(tmp.path().join(DB_FILENAME).exists());
        assert_eq!(db.path(), tmp.path().join(DB_FILENAME));
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
}
