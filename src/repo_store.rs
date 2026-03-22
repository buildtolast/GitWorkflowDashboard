//! Legacy persistent repo storage — JSON file format.
//!
//! Default path: `~/.gwd/repos.json`
//! Override via `--data-dir` or `GWD_DATA_DIR` environment variable.
//!
//! **Note:** This module is now a fallback. Primary persistence uses SQLite
//! via the `database` module. The `resolve_data_dir()` function is still used
//! by the database module. `load_repos()` is the fallback if SQLite fails.

use std::path::{Path, PathBuf};

use crate::config::RepoRef;

/// Filename for the persisted repo list.
const REPOS_FILENAME: &str = "repos.json";

/// Resolve the data directory, creating it if needed.
/// Returns None if the directory can't be determined or created.
pub fn resolve_data_dir(custom: &Option<String>) -> Option<PathBuf> {
    let dir = if let Some(d) = custom {
        PathBuf::from(d)
    } else {
        dirs::home_dir()?.join(".gwd")
    };

    if !dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(path = %dir.display(), error = %e, "Failed to create data directory");
            return None;
        }
        tracing::info!(path = %dir.display(), "Created data directory");
    }

    Some(dir)
}

/// Load persisted repos from the JSON file.
/// Returns an empty Vec if the file doesn't exist or can't be parsed.
pub fn load_repos(data_dir: &Path) -> Vec<RepoRef> {
    let path = data_dir.join(REPOS_FILENAME);
    if !path.exists() {
        return vec![];
    }

    match std::fs::read_to_string(&path) {
        Ok(contents) => match serde_json::from_str::<Vec<RepoRef>>(&contents) {
            Ok(repos) => {
                tracing::info!(count = repos.len(), path = %path.display(), "Loaded persisted repos");
                repos
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "Failed to parse repos file");
                vec![]
            }
        },
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "Failed to read repos file");
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_missing_file() {
        let tmp = TempDir::new().unwrap();
        let loaded = load_repos(tmp.path());
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_load_invalid_json() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join(REPOS_FILENAME), "not json").unwrap();
        let loaded = load_repos(tmp.path());
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_resolve_data_dir_custom() {
        let tmp = TempDir::new().unwrap();
        let custom = Some(tmp.path().join("custom").to_string_lossy().to_string());
        let dir = resolve_data_dir(&custom);
        assert!(dir.is_some());
        assert!(dir.unwrap().exists());
    }
}
