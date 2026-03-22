//! Repository CRUD operations for the database.

use rusqlite::params;

use crate::config::RepoRef;
use crate::error::DashboardError;

use super::Database;

impl Database {
    /// Load all repos from the database.
    pub fn load_repos(&self) -> Vec<RepoRef> {
        let conn = match self.conn() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to acquire lock for load_repos");
                return vec![];
            }
        };
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
        let conn = match self.conn() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to acquire lock for save_repos");
                return;
            }
        };
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
    pub fn get_repo_id(&self, owner: &str, name: &str) -> Result<Option<i64>, DashboardError> {
        let conn = self.conn()?;
        Ok(conn.query_row(
                "SELECT id FROM repos WHERE owner = ?1 AND name = ?2",
                params![owner, name],
                |row| row.get(0),
            )
            .ok())
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

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
    fn test_get_repo_id() {
        let (_tmp, db) = make_test_db();
        db.save_repos(&[make_repo_ref("owner", "repo")]);

        let id = db.get_repo_id("owner", "repo").unwrap();
        assert!(id.is_some());

        let missing = db.get_repo_id("no", "such").unwrap();
        assert!(missing.is_none());
    }
}
