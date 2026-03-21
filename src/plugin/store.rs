//! Per-plugin persistent key-value storage.
//!
//! Each plugin gets its own JSON-backed store file under `$XDG_DATA_HOME/larkline/stores/`.
//! Multi-command plugins sharing a `plugin_group` share a single store.

use std::collections::HashMap;
use std::path::PathBuf;

use thiserror::Error;

/// Maximum serialized size of a single plugin's store (1 MB).
const MAX_STORE_BYTES: usize = 1_048_576;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during store operations.
#[derive(Debug, Error)]
pub enum StoreError {
    /// The store would exceed the 1 MB size limit.
    #[error("store size would exceed 1 MB limit")]
    SizeLimitExceeded,
    /// An I/O error occurred while reading or writing the store file.
    #[error("failed to write store: {0}")]
    IoError(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// PluginStore
// ---------------------------------------------------------------------------

/// A JSON-backed key-value store for a single plugin.
#[derive(Debug)]
pub struct PluginStore {
    data: HashMap<String, serde_json::Value>,
    path: PathBuf,
}

impl PluginStore {
    /// Load a store from disk. Returns an empty store if the file is missing or corrupt.
    pub fn load(path: PathBuf) -> Self {
        let data = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => {
                    serde_json::from_str::<HashMap<String, serde_json::Value>>(&contents)
                        .unwrap_or_else(|e| {
                            tracing::warn!(
                                path = %path.display(),
                                error = %e,
                                "corrupt plugin store, starting fresh"
                            );
                            HashMap::new()
                        })
                }
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "failed to read plugin store"
                    );
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        Self { data, path }
    }

    /// Persist the store to disk. Creates parent directories as needed.
    pub fn save(&self) -> Result<(), StoreError> {
        let bytes = serde_json::to_vec_pretty(&self.data)
            .expect("HashMap<String, Value> is always serializable");

        if bytes.len() > MAX_STORE_BYTES {
            return Err(StoreError::SizeLimitExceeded);
        }

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&self.path, bytes)?;
        Ok(())
    }

    /// Get a value by key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.data.get(key)
    }

    /// Set a key-value pair. Returns an error if the resulting store would exceed 1 MB.
    pub fn set(
        &mut self,
        key: String,
        value: serde_json::Value,
    ) -> Result<(), StoreError> {
        let old = self.data.insert(key.clone(), value);

        // Check total serialized size.
        let size = serde_json::to_vec(&self.data)
            .expect("HashMap<String, Value> is always serializable")
            .len();

        if size > MAX_STORE_BYTES {
            // Revert the insertion.
            if let Some(prev) = old {
                self.data.insert(key, prev);
            } else {
                self.data.remove(&key);
            }
            return Err(StoreError::SizeLimitExceeded);
        }

        Ok(())
    }

    /// Delete a key. Returns `true` if the key existed.
    pub fn delete(&mut self, key: &str) -> bool {
        self.data.remove(key).is_some()
    }

    /// Return all keys in sorted order.
    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.data.keys().cloned().collect();
        keys.sort();
        keys
    }
}

// ---------------------------------------------------------------------------
// Path utilities
// ---------------------------------------------------------------------------

/// Compute the store file path for a plugin.
///
/// Multi-command plugins sharing a `plugin_group` share a single store file.
/// The filename is sanitized from the plugin/group name.
#[must_use]
pub fn store_path_for(plugin_name: &str, plugin_group: Option<&str>) -> PathBuf {
    let key = plugin_group.unwrap_or(plugin_name);
    let sanitized = sanitize_name(key);
    data_dir().join("stores").join(format!("{sanitized}.json"))
}

/// Sanitize a plugin name into a safe filename component.
///
/// Lowercase, replace non-`[a-z0-9_-]` with `_`, collapse runs, trim, truncate to 64.
fn sanitize_name(name: &str) -> String {
    let lowered = name.to_lowercase();
    let replaced: String = lowered
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();

    // Collapse runs of underscores.
    let mut collapsed = String::with_capacity(replaced.len());
    let mut prev_underscore = false;
    for c in replaced.chars() {
        if c == '_' {
            if !prev_underscore {
                collapsed.push(c);
            }
            prev_underscore = true;
        } else {
            collapsed.push(c);
            prev_underscore = false;
        }
    }

    // Trim leading/trailing underscores and truncate.
    let trimmed = collapsed.trim_matches('_');
    if trimmed.is_empty() {
        "_unnamed".to_string()
    } else if trimmed.len() > 64 {
        trimmed[..64].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Base data directory for Larkline mutable data (XDG compliant).
fn data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("larkline")
    } else {
        let home = std::env::var("HOME").map_or_else(|_| PathBuf::from("/tmp"), PathBuf::from);
        home.join(".local").join("share").join("larkline")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_returns_empty_for_nonexistent_file() {
        let path = PathBuf::from("/tmp/larkline_test_nonexistent_store.json");
        let store = PluginStore::load(path);
        assert!(store.keys().is_empty());
    }

    #[test]
    fn set_and_get_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.json");
        let mut store = PluginStore::load(path);

        store
            .set("count".to_string(), serde_json::json!(42))
            .unwrap();
        assert_eq!(store.get("count"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn save_and_reload() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.json");

        {
            let mut store = PluginStore::load(path.clone());
            store
                .set("name".to_string(), serde_json::json!("lark"))
                .unwrap();
            store.save().unwrap();
        }

        let store = PluginStore::load(path);
        assert_eq!(store.get("name"), Some(&serde_json::json!("lark")));
    }

    #[test]
    fn delete_removes_key() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.json");
        let mut store = PluginStore::load(path);

        store
            .set("key".to_string(), serde_json::json!(true))
            .unwrap();
        assert!(store.delete("key"));
        assert!(store.get("key").is_none());
        assert!(!store.delete("key"));
    }

    #[test]
    fn keys_returns_sorted_list() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.json");
        let mut store = PluginStore::load(path);

        store.set("c".to_string(), serde_json::json!(3)).unwrap();
        store.set("a".to_string(), serde_json::json!(1)).unwrap();
        store.set("b".to_string(), serde_json::json!(2)).unwrap();

        assert_eq!(store.keys(), vec!["a", "b", "c"]);
    }

    #[test]
    fn set_rejects_when_size_limit_exceeded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.json");
        let mut store = PluginStore::load(path);

        // A string just over 1 MB.
        let big_value = serde_json::json!("x".repeat(MAX_STORE_BYTES + 1));
        let result = store.set("big".to_string(), big_value);
        assert!(result.is_err());
        assert!(store.get("big").is_none()); // Reverted.
    }

    #[test]
    fn load_handles_corrupt_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("corrupt.json");
        std::fs::write(&path, "not valid json{{{").unwrap();

        let store = PluginStore::load(path);
        assert!(store.keys().is_empty());
    }

    #[test]
    fn sanitize_name_basic() {
        assert_eq!(sanitize_name("Git Branches"), "git_branches");
        assert_eq!(sanitize_name("Top Processes!"), "top_processes");
        assert_eq!(sanitize_name("  hello  "), "hello");
    }

    #[test]
    fn sanitize_name_uses_plugin_group() {
        let path = store_path_for("Recent Branches", Some("Git"));
        assert!(path.to_str().unwrap().contains("git.json"));
    }

    #[test]
    fn sanitize_name_collapses_underscores() {
        assert_eq!(sanitize_name("a   b   c"), "a_b_c");
    }

    #[test]
    fn sanitize_name_empty_falls_back() {
        assert_eq!(sanitize_name(""), "_unnamed");
        assert_eq!(sanitize_name("___"), "_unnamed");
    }
}
