//! Command history — records recently-used plugin commands across sessions.
//!
//! Entries are stored as a JSON array in `~/.config/larkline/history.json`,
//! newest-first. At most [`MAX_ENTRIES`] entries are kept; (plugin, command)
//! pairs are deduplicated so each command appears at most once.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::config_path;

/// Maximum number of history entries to persist.
const MAX_ENTRIES: usize = 50;

/// A single recorded command execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Plugin group name (e.g., `"GitHub"`).
    pub plugin: String,
    /// Command name within the plugin (e.g., `"Workflow Runs"`).
    pub command: String,
    /// Unix timestamp of the execution (seconds since epoch).
    pub ts: u64,
}

/// Returns the path to the history file.
pub fn history_path() -> PathBuf {
    config_path()
        .parent()
        .map_or_else(|| PathBuf::from("history.json"), |p| p.join("history.json"))
}

/// Load all history entries from disk. Returns an empty vec on any error.
fn load() -> Vec<HistoryEntry> {
    let Ok(bytes) = std::fs::read(history_path()) else {
        return vec![];
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

/// Save entries to disk. Silently ignores errors (history is best-effort).
fn save(entries: &[HistoryEntry]) {
    if let Ok(json) = serde_json::to_string(entries) {
        let _ = std::fs::write(history_path(), json);
    }
}

/// Record a command execution.
///
/// The entry is moved to the front (dedup by plugin+command), and the list
/// is capped at [`MAX_ENTRIES`].
pub fn record(plugin: &str, command: &str) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    let new_entry = HistoryEntry {
        plugin: plugin.to_string(),
        command: command.to_string(),
        ts,
    };

    let mut entries = load();
    // Remove any existing entry for this (plugin, command) pair.
    entries.retain(|e| !(e.plugin == plugin && e.command == command));
    // Prepend the new entry.
    entries.insert(0, new_entry);
    // Cap at MAX_ENTRIES.
    entries.truncate(MAX_ENTRIES);
    save(&entries);
}

/// Return the most recent execution timestamp per plugin group key.
///
/// Used by the "Recently Used" sort mode to order groups in the unified list.
pub fn timestamps_by_group() -> std::collections::HashMap<String, u64> {
    let mut map = std::collections::HashMap::new();
    for entry in load() {
        let ts = map.entry(entry.plugin).or_insert(0u64);
        if entry.ts > *ts {
            *ts = entry.ts;
        }
    }
    map
}
