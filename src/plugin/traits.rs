//! Core plugin abstractions.
//!
//! These types form the stable contract between the Larkline host and all plugin backends.
//! The JSON schema for script plugins mirrors `PluginOutput` exactly.
// Phase 2: types used throughout; suppress dead_code until all modules are wired up in Task 6.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

/// Metadata about a plugin, loaded from its `manifest.toml`.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
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
    /// Nerd Font glyph alternative (used when `icon_set = "nerd"` in config).
    pub icon_nerd: Option<String>,
    /// Optional category for grouping (e.g., "dev", "system", "home").
    pub category: Option<String>,
    /// Optional direct-launch keybinding (e.g., "g p").
    pub keybinding: Option<String>,
    /// Maximum time to wait for the plugin to complete.
    pub timeout: Duration,
    /// Whether this plugin uses streaming (newline-delimited JSON) output.
    pub streaming: bool,
    /// Absolute path to the entry script (used by the engine for streaming dispatch).
    pub entry_path: Option<PathBuf>,
    /// Whether this plugin should be executed in the background on startup.
    /// Defaults to `true`. Set `prefetch = false` in the manifest to opt out.
    pub prefetch: bool,
    /// Parent plugin name when this command belongs to a multi-command plugin.
    /// `None` for single-command (legacy) plugins.
    pub plugin_group: Option<String>,
    /// Quick-launch key shown as a badge in the unified list (e.g., `"gb"`).
    pub quickkey: Option<String>,
    /// Whether to use stale-while-revalidate caching. Default `true`.
    /// Set `cache = false` in the manifest to always execute fresh.
    pub cache: bool,
    /// Advisory list of secret env var names this plugin needs (e.g., `["GITHUB_TOKEN"]`).
    pub secrets: Vec<String>,
    /// Settings declared in the manifest. Larkline renders these as a persistent form
    /// accessible from the power menu; submitted values are written to the plugin's store.
    pub settings_spec: Vec<FormField>,
    /// Show a compact summary widget at the top of the unified list.
    pub widget: bool,
    /// Widget auto-refresh interval in seconds (0 = no auto-refresh, default 60).
    pub widget_refresh_secs: u64,
    /// Whether this plugin supports mini app mode (full-screen split panes).
    pub mini_app: bool,
    /// Whether this command (or every command in a multi-command plugin)
    /// is exposed to the in-app AI agent as a callable tool. Default `false`
    /// — agent-callable plugins explicitly opt in via the manifest. The
    /// agent's tool registry (see `crate::agent::registry`) builds a
    /// `ToolDefinition` per callable command at startup.
    pub agent_callable: bool,
    /// Whether this command mutates state (deletes files, sends mail,
    /// archives messages, etc.). Drives the agent's dry-run plan preview:
    /// destructive tools render with a `[!]` marker and the entire plan
    /// requires user approval before any tool runs. Default `false`.
    pub destructive: bool,
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
    /// Raw text content. For script plugins, populated when stdout is not valid JSON.
    /// Plugins can also set this explicitly to provide markdown or plain text content.
    #[serde(default)]
    pub raw_text: Option<String>,
    /// Column definitions for table rendering. Empty = use list mode.
    #[serde(default)]
    pub columns: Vec<ColumnDef>,
    /// Optional form specification. When present, the TUI renders an interactive
    /// form instead of items. On submission, the plugin re-executes with values.
    #[serde(default)]
    pub form: Option<FormSpec>,
    /// Output format hint: `"markdown"`, `"plain"`, or `None` (auto-detect).
    /// When `"markdown"`, `raw_text` is rendered as markdown with syntax highlighting.
    #[serde(default)]
    pub output_format: Option<String>,
    /// Optional mini app layout tree. When present, the TUI renders split panes
    /// instead of a single output view.
    #[serde(default)]
    pub layout: Option<MiniAppLayout>,
}

// ---------------------------------------------------------------------------
// Mini app layout types
// ---------------------------------------------------------------------------

/// Unique identifier for a pane within a mini app layout.
pub type PaneId = String;

/// Layout specification for mini app mode.
///
/// A recursive tree: leaf nodes are panes that render content, split nodes
/// divide space between children (like neovim splits).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum MiniAppLayout {
    /// A leaf pane that renders content.
    Pane {
        /// Unique identifier for this pane.
        id: PaneId,
        /// Initial content to display.
        #[serde(default)]
        content: PaneContent,
    },
    /// A split that divides space between children.
    Split {
        /// Horizontal = side-by-side (left|right), Vertical = stacked (top|bottom).
        direction: SplitDirection,
        /// Children in order, each with a proportional size.
        children: Vec<LayoutChild>,
    },
}

/// A child node in a split layout, with a proportional size.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutChild {
    /// Percentage of parent space (children should sum to 100).
    pub size: u16,
    /// The nested layout node.
    pub layout: MiniAppLayout,
}

/// Direction of a split.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitDirection {
    /// Side-by-side (left | right).
    Horizontal,
    /// Stacked (top / bottom).
    Vertical,
}

/// Content rendered within a single pane. Reuses the same primitives as `PluginOutput`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaneContent {
    /// Title shown in the pane's border.
    #[serde(default)]
    pub title: String,
    /// Navigable list of items.
    #[serde(default)]
    pub items: Vec<OutputItem>,
    /// Raw text or markdown content.
    #[serde(default)]
    pub raw_text: Option<String>,
    /// Column definitions for table rendering.
    #[serde(default)]
    pub columns: Vec<ColumnDef>,
    /// Optional interactive form.
    #[serde(default)]
    pub form: Option<FormSpec>,
    /// Output format hint.
    #[serde(default)]
    pub output_format: Option<String>,
}

/// A single item in a plugin's output list.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// Plugin-defined default text for `y` (copy). When set, `y` copies this instead of `label`.
    /// Use for items where the label is decorative but the useful value is something else
    /// (e.g., PID for a process list, raw branch name for git branches).
    #[serde(default)]
    pub copy_text: Option<String>,
    /// Optional rich body text shown in Telescope's preview pane (markdown by convention).
    /// The TUI does not render this field. Empty/missing → previewer shows a placeholder.
    /// Plugins should pre-truncate at ~5KB to keep the JSON payload small.
    #[serde(default)]
    pub preview: Option<String>,
    /// Optional action invoked when the user retries an error item (`r` in TUI, `<C-r>` in
    /// Telescope). Plugins set this on items whose backing call failed transiently — typically
    /// the same action that produced the error, so retrying re-fires it.
    #[serde(default)]
    pub retry_action: Option<ItemAction>,
    /// Optional documentation or troubleshooting URL opened by the help affordance (`?` in TUI,
    /// `<C-?>` in Telescope). Set on error items where a known fix exists upstream.
    #[serde(default)]
    pub help_url: Option<String>,
    /// Arbitrary key-value pairs for table column resolution and future extensibility.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
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
    #[serde(rename = "command", alias = "kind")]
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
    /// Chain: call the plugin's `on_action` callback with context.
    /// `args[0]` = callback ID, `args[1..]` = context passed to `on_action`.
    Chain,
    /// Update a specific pane in mini app mode.
    /// `args[0]` = target pane ID, `args[1]` = callback ID, `args[2..]` = context.
    #[serde(rename = "update_pane")]
    UpdatePane,
    /// Open a file in the parent Neovim instance (requires `$NVIM` env var, nvim ≥ 0.9).
    /// `args[0]` = file path, `args[1]` = split kind (optional: `edit|split|vsplit|tabedit`, default `edit`).
    /// Falls back to `Open` behaviour when not running under Neovim.
    #[serde(rename = "nvim_edit")]
    NvimEdit,
}

// ---------------------------------------------------------------------------
// Table output types
// ---------------------------------------------------------------------------

/// Definition of a column for table rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    /// Column header text.
    pub header: String,
    /// Key to resolve the cell value: `"label"`, `"detail"`, `"icon"`, `"url"`,
    /// or any key in [`OutputItem::metadata`].
    pub key: String,
    /// Text alignment within the column.
    #[serde(default)]
    pub align: ColumnAlign,
}

/// Text alignment for table columns.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColumnAlign {
    /// Left-aligned (default).
    #[default]
    Left,
    /// Right-aligned.
    Right,
    /// Center-aligned.
    Center,
}

// ---------------------------------------------------------------------------
// Form types
// ---------------------------------------------------------------------------

/// Specification for an interactive form that plugins can return.
///
/// When present in [`PluginOutput`], the TUI renders a form instead of items.
/// On submission, the plugin is re-executed with the collected values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormSpec {
    /// The fields in this form, rendered in order.
    pub fields: Vec<FormField>,
    /// Custom label for the submit button. Defaults to "Submit".
    #[serde(default)]
    pub submit_label: Option<String>,
}

/// A single field in a plugin form.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    /// Unique identifier used as the key in the form values map.
    pub id: String,
    /// Human-readable label displayed beside the input.
    pub label: String,
    /// The type of input control.
    #[serde(rename = "type")]
    pub field_type: FieldType,
    /// Whether this field must have a non-empty value to submit.
    #[serde(default)]
    pub required: bool,
    /// Pre-filled default value.
    #[serde(default)]
    pub default_value: Option<String>,
    /// Placeholder text shown when the field is empty.
    #[serde(default)]
    pub placeholder: Option<String>,
}

/// The type of form input control.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FieldType {
    /// Single-line text input.
    Text,
    /// Selection from a list of options.
    Select {
        /// Available options for selection.
        options: Vec<String>,
    },
    /// Boolean toggle (true/false).
    Toggle,
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

    /// Execute the plugin with form values from a previously submitted form.
    ///
    /// The default implementation ignores form values and delegates to [`execute()`].
    /// Backends that support forms override this to inject values into their context.
    async fn execute_with_form(
        &self,
        _form_values: std::collections::HashMap<String, String>,
    ) -> Result<PluginOutput, PluginError> {
        self.execute().await
    }

    /// Execute the plugin's `on_action` callback for action chaining.
    ///
    /// Called when a `Chain` or `UpdatePane` action is triggered. The plugin
    /// re-executes with the callback ID and context, returning updated output.
    /// The default implementation returns [`PluginError::ActionNotSupported`].
    async fn execute_action(
        &self,
        callback_id: &str,
        _context: &str,
    ) -> Result<PluginOutput, PluginError> {
        Err(PluginError::ActionNotSupported {
            action_id: callback_id.to_string(),
        })
    }
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
        assert!(item.copy_text.is_none());
        assert!(item.actions.is_empty());
    }

    #[test]
    fn copy_text_deserializes_when_present() {
        let json = r#"{"label": "Chrome", "copy_text": "12345"}"#;
        let item: OutputItem = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(item.label, "Chrome");
        assert_eq!(item.copy_text.as_deref(), Some("12345"));
    }

    #[test]
    fn preview_serializes_when_set() {
        let item = OutputItem {
            label: "Issue #42".to_string(),
            preview: Some("Body of the issue\n\nMore details here.".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&item).expect("serialization failed");
        let round: OutputItem = serde_json::from_str(&json).expect("round-trip failed");
        assert_eq!(
            round.preview.as_deref(),
            Some("Body of the issue\n\nMore details here.")
        );
    }

    #[test]
    fn preview_absent_deserializes() {
        let json = r#"{"label": "no preview"}"#;
        let item: OutputItem = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(item.label, "no preview");
        assert!(item.preview.is_none());
    }

    #[test]
    fn retry_action_round_trips() {
        let item = OutputItem {
            label: "Auth failed".to_string(),
            icon: Some("!".to_string()),
            retry_action: Some(ItemAction {
                id: Some("retry".to_string()),
                label: "Retry".to_string(),
                kind: ActionKind::Chain,
                args: vec!["fetch_issues".to_string()],
                confirm: false,
            }),
            ..Default::default()
        };
        let json = serde_json::to_string(&item).expect("serialization failed");
        let round: OutputItem = serde_json::from_str(&json).expect("round-trip failed");
        let retry = round.retry_action.expect("retry_action lost in round-trip");
        assert_eq!(retry.kind, ActionKind::Chain);
        assert_eq!(retry.args, vec!["fetch_issues"]);
    }

    #[test]
    fn help_url_round_trips() {
        let item = OutputItem {
            label: "Missing CLI".to_string(),
            help_url: Some("https://cli.github.com/manual/installation".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&item).expect("serialization failed");
        let round: OutputItem = serde_json::from_str(&json).expect("round-trip failed");
        assert_eq!(
            round.help_url.as_deref(),
            Some("https://cli.github.com/manual/installation")
        );
    }

    #[test]
    fn error_fields_absent_default_to_none() {
        let json = r#"{"label": "plain item"}"#;
        let item: OutputItem = serde_json::from_str(json).expect("deserialization failed");
        assert!(item.retry_action.is_none());
        assert!(item.help_url.is_none());
    }

    #[test]
    fn action_kind_deserializes_nvim_edit() {
        let json = r#"{"label": "Open in Neovim", "kind": "nvim_edit", "args": ["/path/to/file", "vsplit"]}"#;
        let action: ItemAction = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(action.kind, ActionKind::NvimEdit);
        assert_eq!(action.args, vec!["/path/to/file", "vsplit"]);
    }

    #[test]
    fn action_kind_nvim_edit_optional_split() {
        let json = r#"{"label": "Open", "kind": "nvim_edit", "args": ["/path"]}"#;
        let action: ItemAction = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(action.kind, ActionKind::NvimEdit);
        assert_eq!(action.args, vec!["/path"]);
    }

    #[test]
    fn form_spec_deserializes_all_field_types() {
        let json = r#"{
            "title": "Create Note",
            "form": {
                "fields": [
                    {"id": "title", "label": "Title", "type": {"kind": "text"}, "required": true, "placeholder": "Enter title"},
                    {"id": "category", "label": "Category", "type": {"kind": "select", "options": ["work", "personal"]}, "default_value": "work"},
                    {"id": "urgent", "label": "Urgent", "type": {"kind": "toggle"}}
                ],
                "submit_label": "Create"
            }
        }"#;
        let output: PluginOutput = serde_json::from_str(json).expect("parse failed");
        let form = output.form.expect("form should be present");
        assert_eq!(form.fields.len(), 3);
        assert_eq!(form.submit_label.as_deref(), Some("Create"));
        assert!(form.fields[0].required);
        assert!(matches!(form.fields[0].field_type, FieldType::Text));
        assert!(matches!(
            form.fields[1].field_type,
            FieldType::Select { .. }
        ));
        assert!(matches!(form.fields[2].field_type, FieldType::Toggle));
    }

    #[test]
    fn plugin_output_without_form_is_backward_compatible() {
        let json = r#"{"title": "No Form", "items": [{"label": "item"}]}"#;
        let output: PluginOutput = serde_json::from_str(json).expect("parse failed");
        assert!(output.form.is_none());
        assert_eq!(output.items.len(), 1);
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
            icon_nerd: None,
            category: None,
            keybinding: None,
            timeout: std::time::Duration::from_secs(5),
            streaming: false,
            entry_path: None,
            prefetch: true,
            plugin_group: None,
            quickkey: None,
            cache: true,
            secrets: vec![],
            settings_spec: vec![],
            widget: false,
            widget_refresh_secs: 0,
            mini_app: false,
            agent_callable: false,
            destructive: false,
        };
        accepts_dyn(Box::new(MockPlugin(meta)));
    }

    #[test]
    fn mini_app_layout_deserializes_single_pane() {
        let json = r#"{"kind": "pane", "id": "main", "content": {"title": "Hello"}}"#;
        let layout: MiniAppLayout = serde_json::from_str(json).expect("parse failed");
        match layout {
            MiniAppLayout::Pane { id, content } => {
                assert_eq!(id, "main");
                assert_eq!(content.title, "Hello");
            }
            MiniAppLayout::Split { .. } => panic!("expected Pane"),
        }
    }

    #[test]
    fn mini_app_layout_deserializes_horizontal_split() {
        let json = r#"{
            "kind": "split",
            "direction": "horizontal",
            "children": [
                {"size": 30, "layout": {"kind": "pane", "id": "left", "content": {"title": "List"}}},
                {"size": 70, "layout": {"kind": "pane", "id": "right", "content": {"title": "Detail"}}}
            ]
        }"#;
        let layout: MiniAppLayout = serde_json::from_str(json).expect("parse failed");
        match layout {
            MiniAppLayout::Split {
                direction,
                children,
            } => {
                assert!(matches!(direction, SplitDirection::Horizontal));
                assert_eq!(children.len(), 2);
                assert_eq!(children[0].size, 30);
                assert_eq!(children[1].size, 70);
            }
            MiniAppLayout::Pane { .. } => panic!("expected Split"),
        }
    }

    #[test]
    fn mini_app_layout_deserializes_nested_splits() {
        let json = r#"{
            "kind": "split",
            "direction": "horizontal",
            "children": [
                {"size": 30, "layout": {"kind": "pane", "id": "nav", "content": {"title": "Nav"}}},
                {"size": 70, "layout": {
                    "kind": "split",
                    "direction": "vertical",
                    "children": [
                        {"size": 60, "layout": {"kind": "pane", "id": "detail", "content": {"title": "Detail"}}},
                        {"size": 40, "layout": {"kind": "pane", "id": "actions", "content": {"title": "Actions"}}}
                    ]
                }}
            ]
        }"#;
        let layout: MiniAppLayout = serde_json::from_str(json).expect("parse failed");
        match layout {
            MiniAppLayout::Split { children, .. } => {
                assert_eq!(children.len(), 2);
                match &children[1].layout {
                    MiniAppLayout::Split {
                        direction,
                        children: inner,
                    } => {
                        assert!(matches!(direction, SplitDirection::Vertical));
                        assert_eq!(inner.len(), 2);
                    }
                    MiniAppLayout::Pane { .. } => panic!("expected nested Split"),
                }
            }
            MiniAppLayout::Pane { .. } => panic!("expected Split"),
        }
    }

    #[test]
    fn plugin_output_with_layout_field() {
        let json = r#"{
            "title": "Dashboard",
            "layout": {
                "kind": "pane",
                "id": "main",
                "content": {"title": "Main", "items": [{"label": "Item 1"}]}
            }
        }"#;
        let output: PluginOutput = serde_json::from_str(json).expect("parse failed");
        assert_eq!(output.title, "Dashboard");
        assert!(output.layout.is_some());
        match output.layout.unwrap() {
            MiniAppLayout::Pane { id, content } => {
                assert_eq!(id, "main");
                assert_eq!(content.items.len(), 1);
            }
            MiniAppLayout::Split { .. } => panic!("expected Pane"),
        }
    }

    #[test]
    fn plugin_output_without_layout_is_backward_compatible() {
        let json = r#"{"title": "Old Plugin", "items": [{"label": "hello"}]}"#;
        let output: PluginOutput = serde_json::from_str(json).expect("parse failed");
        assert!(output.layout.is_none());
        assert_eq!(output.items.len(), 1);
    }
}
