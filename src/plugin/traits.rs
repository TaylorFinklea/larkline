//! Core plugin abstractions.
//!
//! These types form the stable contract between the Larkline host and all plugin backends.
//! The JSON schema for script plugins mirrors `PluginOutput` exactly.
// Phase 2: types used throughout; suppress dead_code until all modules are wired up in Task 6.
#![allow(dead_code)]

use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

/// Metadata about a plugin, loaded from its `manifest.toml`.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// Display name shown in the plugin list.
    pub name: String,
    /// One-line description shown beneath the name.
    pub description: String,
    /// Semver version string.
    pub version: String,
    /// Author name or handle.
    pub author: String,
    /// Emoji or single character shown as the plugin icon.
    pub icon: String,
    /// Optional category for grouping (e.g., "dev", "system", "home").
    pub category: Option<String>,
    /// Optional direct-launch keybinding (e.g., "g p").
    pub keybinding: Option<String>,
    /// Maximum time to wait for the plugin to complete.
    pub timeout: Duration,
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// Structured output produced by a plugin execution.
///
/// This is the deserialized form of the JSON a script plugin writes to stdout.
/// If stdout is not valid JSON, `raw_text` is populated instead.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginOutput {
    /// Heading displayed at the top of the output pane.
    pub title: String,
    /// Navigable list of result items. Empty if the plugin returned raw text.
    #[serde(default)]
    pub items: Vec<OutputItem>,
    /// Raw text fallback — populated when stdout is not valid JSON.
    #[serde(skip)]
    pub raw_text: Option<String>,
}

/// A single item in a plugin's output list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputItem {
    /// Primary text displayed in the list row.
    pub label: String,
    /// Secondary text shown dimmed beside or below the label.
    #[serde(default)]
    pub detail: Option<String>,
    /// Emoji or character shown before the label.
    #[serde(default)]
    pub icon: Option<String>,
    /// URL associated with this item (enables the "open" action).
    #[serde(default)]
    pub url: Option<String>,
    /// Actions the user can invoke on this item.
    #[serde(default)]
    pub actions: Vec<ItemAction>,
}

/// An action that can be invoked on an [`OutputItem`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemAction {
    /// Optional action ID.
    #[serde(default)]
    pub id: Option<String>,
    /// Human-readable label shown in the action list.
    pub label: String,
    /// The kind of command to execute.
    #[serde(rename = "command")]
    pub kind: ActionKind,
    /// Arguments for the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Whether to prompt the user for confirmation before executing.
    #[serde(default)]
    pub confirm: bool,
}

/// The kind of action to execute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionKind {
    /// Open a URL in the system browser.
    Open,
    /// Copy a string to the clipboard.
    Clipboard,
    /// Run a shell command.
    Shell,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during plugin execution.
#[derive(Debug, Error)]
pub enum PluginError {
    /// The plugin exceeded its configured timeout.
    #[error("plugin timed out after {0:?}")]
    Timeout(Duration),
    /// The plugin process failed to start or returned a non-zero exit code.
    #[error("plugin execution failed: {0}")]
    ExecutionFailed(String),
    /// The plugin produced output that could not be parsed.
    #[error("invalid plugin output: {0}")]
    InvalidOutput(String),
    /// The requested action is not supported by this plugin.
    #[error("action not supported: {action_id}")]
    ActionNotSupported {
        /// The ID of the unsupported action.
        action_id: String,
    },
}

// ---------------------------------------------------------------------------
// Plugin trait
// ---------------------------------------------------------------------------

/// The core plugin abstraction. All plugin backends implement this trait.
///
/// Object-safe via `async_trait` so the engine can hold `dyn Plugin`.
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    /// Returns metadata about this plugin.
    fn metadata(&self) -> &PluginMetadata;

    /// Execute the plugin's main action and return structured output.
    async fn execute(&self) -> Result<PluginOutput, PluginError>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_output_deserializes_minimal_json() {
        let json = r#"{"title": "Test Output"}"#;
        let output: PluginOutput = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(output.title, "Test Output");
        assert!(output.items.is_empty());
    }

    #[test]
    fn plugin_output_deserializes_with_items() {
        let json = r#"{
            "title": "Results",
            "items": [
                {
                    "label": "Item One",
                    "detail": "some detail",
                    "url": "https://example.com",
                    "actions": []
                }
            ]
        }"#;
        let output: PluginOutput = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(output.items.len(), 1);
        assert_eq!(output.items[0].label, "Item One");
        assert_eq!(output.items[0].url.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn output_item_allows_missing_optional_fields() {
        let json = r#"{"label": "minimal"}"#;
        let item: OutputItem = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(item.label, "minimal");
        assert!(item.detail.is_none());
        assert!(item.icon.is_none());
        assert!(item.url.is_none());
        assert!(item.actions.is_empty());
    }

    #[test]
    fn hello_world_json_deserializes_correctly() {
        let json = r#"{
            "title": "Hello from Larkline!",
            "items": [{
                "label": "Hello, World!",
                "detail": "This is the simplest possible plugin",
                "icon": "👋",
                "actions": [{
                    "id": "copy",
                    "label": "Copy greeting",
                    "command": "clipboard",
                    "args": ["Hello, World!"]
                }]
            }]
        }"#;
        let output: PluginOutput = serde_json::from_str(json).expect("parse failed");
        assert_eq!(output.items[0].actions[0].kind, ActionKind::Clipboard);
        assert_eq!(output.items[0].actions[0].args, vec!["Hello, World!"]);
    }

    #[test]
    fn plugin_trait_is_object_safe() {
        struct MockPlugin(PluginMetadata);

        #[async_trait::async_trait]
        impl Plugin for MockPlugin {
            fn metadata(&self) -> &PluginMetadata {
                &self.0
            }
            async fn execute(&self) -> Result<PluginOutput, PluginError> {
                Ok(PluginOutput {
                    title: "mock".to_string(),
                    ..Default::default()
                })
            }
        }

        fn accepts_dyn(_p: Box<dyn Plugin>) {}

        let meta = PluginMetadata {
            name: "test".into(),
            description: "test".into(),
            version: "0.1.0".into(),
            author: "test".into(),
            icon: "T".into(),
            category: None,
            keybinding: None,
            timeout: std::time::Duration::from_secs(5),
        };
        accepts_dyn(Box::new(MockPlugin(meta)));
    }
}
