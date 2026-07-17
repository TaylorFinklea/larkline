//! Core application state and event loop.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as NucleoConfig, Matcher, Utf32Str};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use crate::action::Action;
use crate::config::{Config, KeybindingsConfig, ResolvedKeybindings, Theme};
use crate::input;
use crate::plugin::engine::{EngineEvent, ExecutionSource, ExecutionStamp, PluginEngine};
use crate::plugin::registry;
use crate::plugin::traits::{
    ActionKind, ItemAction, MiniAppLayout, OutputItem, PaneContent, PaneId, PluginError,
    PluginOutput,
};
use crate::plugin::{Plugin, PluginMetadata};
use crate::tui::ui;

// ---------------------------------------------------------------------------
// Output mode
// ---------------------------------------------------------------------------

/// How plugin output is displayed in the output pane.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum OutputMode {
    /// Render structured items as a selectable list.
    #[default]
    List,
    /// Render raw text (or items formatted as plain lines).
    RawText,
    /// Render items as a table with column headers (when `columns` is non-empty).
    Table,
    /// Render `raw_text` as markdown with syntax highlighting.
    Markdown,
}

// ---------------------------------------------------------------------------
// State types
// ---------------------------------------------------------------------------

/// The current UI mode — describes *which pane is active*.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Mode {
    /// Unified launcher view — plugin sections + items, filterable by query.
    #[default]
    Unified,
    /// Viewing a plugin's output in the detail pane (table/raw-text fallback).
    ViewOutput,
    /// Plugin management screen — enable/disable, settings, secrets.
    PluginManager,
    /// Mini app mode — full-screen split-pane layout controlled by a plugin.
    MiniApp,
}

/// A row in the unified launcher list.
#[derive(Debug, Clone)]
pub enum UnifiedRow {
    /// A selectable command row.
    Command {
        /// Index into `AppState::plugins`.
        plugin_index: usize,
        name: String,
        description: String,
        icon: String,
        /// Quick-launch key badge (e.g., `"gb"`).
        quickkey: Option<String>,
        /// Parent plugin/group name shown inline in dimmed text.
        /// `Some` for multi-command plugins, `None` for single-command (name == group).
        group_name: Option<String>,
        /// Nucleo match positions into `name` for character highlighting.
        match_positions: Vec<usize>,
    },
}

impl UnifiedRow {
    /// All rows are selectable commands; kept for call-site compatibility.
    #[allow(clippy::unused_self)]
    pub fn is_selectable(&self) -> bool {
        true
    }
}

/// Cached execution result for a plugin (used by prefetch).
#[derive(Debug, Clone)]
pub enum CachedResult {
    /// Plugin is currently executing in the background.
    Loading(#[allow(dead_code)] std::time::Instant),
    /// Plugin completed successfully.
    Ready(PluginOutput),
    /// Plugin failed.
    Error(String),
    /// Stale output shown while a background re-execution is in progress.
    Revalidating(PluginOutput),
}

#[derive(Debug)]
struct CacheExecution {
    stamp: ExecutionStamp,
    was_revalidating: bool,
    streamed_output: Option<PluginOutput>,
}

enum CacheEvent {
    Open {
        plugin_index: usize,
        cache_enabled: bool,
    },
    Started {
        plugin_index: usize,
        stamp: ExecutionStamp,
        source: ExecutionSource,
    },
    Partial {
        plugin_index: usize,
        stamp: ExecutionStamp,
        source: ExecutionSource,
        title: Option<String>,
        items: Vec<OutputItem>,
    },
    Finished {
        plugin_index: usize,
        stamp: ExecutionStamp,
        source: ExecutionSource,
        result: Box<Result<PluginOutput, PluginError>>,
        cache_enabled: bool,
    },
    Invalidate {
        plugin_index: usize,
    },
    Clear,
}

enum CacheOpen {
    ShowAndRevalidate(PluginOutput),
    ShowRevalidating(PluginOutput),
    Loading,
    Error(String),
    Miss,
}

enum CacheTransition {
    None,
    Open(CacheOpen),
    Started {
        was_revalidating: bool,
    },
    Partial {
        output: PluginOutput,
        replace: bool,
    },
    Finished {
        was_revalidating: bool,
        result: Result<PluginOutput, String>,
    },
}

/// Sort order for the unified launcher list.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum SortMode {
    /// Alphabetical order (default).
    #[default]
    Alpha,
    /// Most recently used first; never-used commands sort to the end alphabetically.
    Recent,
}

impl SortMode {
    /// Returns the display label shown in the status bar / power menu.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Alpha => "Alpha",
            Self::Recent => "Recent",
        }
    }

    /// Cycles to the next mode.
    pub fn next(&self) -> Self {
        match self {
            Self::Alpha => Self::Recent,
            Self::Recent => Self::Alpha,
        }
    }
}

/// Vim-style input mode — describes *how keys are interpreted*.
///
/// Orthogonal to [`Mode`]: Normal + Browse = navigation; Insert + Browse = quickkeys/search.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum VimMode {
    /// Navigation keys (j/k/q) are active. Default on startup.
    #[default]
    Normal,
    /// Quickkeys and search input are active; j/k/q are NOT navigation.
    Insert,
    /// Command input mode — accumulates a `:command` string.
    Command,
}

/// Central application state.
///
/// The TUI layer reads this struct to render; it never writes to it.
/// State transitions happen here in [`App`].
#[derive(Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct AppState {
    /// All known plugins (loaded from the registry).
    pub plugins: Vec<PluginMetadata>,
    /// The current fuzzy search query.
    pub query: String,
    /// Current UI mode.
    pub mode: Mode,
    /// Whether the application should exit on the next tick.
    pub should_quit: bool,
    /// Output from the last executed plugin.
    pub plugin_output: Option<PluginOutput>,
    /// Error message from the last executed plugin.
    pub plugin_error: Option<String>,
    /// Whether a plugin is currently executing.
    pub is_loading: bool,
    /// Spinner animation tick counter.
    pub spinner_tick: u8,
    /// Index of the selected item within plugin output (for item navigation).
    pub output_selected: usize,
    /// Whether to show emoji icons next to plugin names.
    pub show_icons: bool,
    /// Plugin names pinned to the top (from config), in config order.
    pub favorites: Vec<String>,
    /// Warnings to show in the status bar (cleared on first keypress).
    pub warnings: Vec<String>,
    /// When plugin execution started (for elapsed-time display).
    pub loading_started: Option<std::time::Instant>,
    /// How plugin output is displayed in the output pane.
    pub output_mode: OutputMode,
    /// Vim-style input mode (Normal / Insert / Command).
    pub vim_mode: VimMode,
    /// Accumulated input buffer for Command mode (the text after `:`).
    pub command_input: String,
    /// Pending shell action awaiting user confirmation (Y/N).
    pub pending_confirmation: Option<PendingConfirmation>,
    /// Cache of execution results keyed by plugin index.
    pub result_cache: std::collections::HashMap<usize, CachedResult>,
    /// Flat list of rows for the unified launcher view.
    pub unified_rows: Vec<UnifiedRow>,
    /// Index into `unified_rows` for the currently highlighted selectable row.
    pub unified_selected: usize,
    /// Maximum items to show per section in the output pane (0 = unlimited). From config.
    #[allow(dead_code)]
    pub max_items_per_section: usize,
    /// Flash message shown in status bar after an action completes.
    pub status_message: Option<(String, std::time::Instant)>,
    /// Plugin index currently displayed in the [`Mode::ViewOutput`] pane, if any.
    pub viewing_plugin_index: Option<usize>,
    /// Plugin index of the highlighted command in Unified mode (for preview pane).
    pub preview_plugin_index: Option<usize>,
    /// Copy-menu overlay state (shown over `ViewOutput` pane).
    pub copy_menu: Option<CopyMenuState>,
    /// Active search query within the output pane.
    pub output_query: String,
    /// Whether output search mode is active (user pressed `/` in `ViewOutput`).
    pub output_searching: bool,
    /// Indices into `plugin_output.items` that match `output_query`.
    /// When empty and `output_query` is empty, all items are shown.
    pub output_filtered_indices: Vec<usize>,
    /// Active form state (shown in the output pane when a plugin returns a form).
    pub form_state: Option<FormState>,
    /// Scroll offset for Markdown and `RawText` output modes (line-based).
    pub scroll_offset: usize,
    /// Whether to show plugin descriptions in the unified list.
    pub show_descriptions: bool,
    /// Action palette overlay (searchable action list for the selected item).
    pub action_palette: Option<ActionPaletteState>,
    /// Power menu overlay (which-key style, Space key).
    pub power_menu: Option<PowerMenuState>,
    /// Cached rendered markdown `Text` — avoids re-parsing every frame.
    pub markdown_cache: Option<ratatui::text::Text<'static>>,
    /// Content-addressed cache of ANSI-parsed raw text — avoids re-parsing
    /// every frame (see [`crate::tui::ansi_cache`]).
    pub ansi_cache: crate::tui::ansi_cache::AnsiTextCache,
    /// Pending `g` key — waiting for second `g` to trigger `GoToFirst`.
    pub pending_g: bool,
    /// Whether the sidebar is hidden in `ViewOutput` mode.
    pub sidebar_hidden: bool,
    /// User-locked layout profile override. `None` = auto-detect from
    /// terminal width every frame. Set via `:layout phone|narrow|medium
    /// |wide` and cleared via `:layout auto`.
    pub layout_profile_override: Option<crate::tui::profile::LayoutProfile>,
    /// Sidebar width percentage in browse mode (20-80).
    pub sidebar_ratio: u16,
    /// History stack for back-navigation through `ViewOutput` states.
    pub navigation_history: Vec<NavigationEntry>,
    /// Current sort order for the unified launcher list.
    pub sort_mode: SortMode,
    /// Last time each widget plugin was refreshed (keyed by `plugin_index`).
    pub widget_last_refresh: std::collections::HashMap<usize, std::time::Instant>,
    /// Whether the widget dashboard row is visible.
    pub widgets_visible: bool,
    /// Whether focus is on the widget row (vs the command list).
    pub widget_focused: bool,
    /// Index of the currently selected widget card.
    pub widget_selected: usize,
    /// Plugin indices of active widget commands (built at startup, refreshed on R).
    pub widget_indices: Vec<usize>,
    /// Last time each glance-strip status plugin was refreshed (by `plugin_index`).
    pub status_last_refresh: std::collections::HashMap<usize, std::time::Instant>,
    /// Whether the glance strip (compact status chips) is visible.
    pub status_visible: bool,
    /// Whether focus is on the glance strip (vs the command list / widgets).
    pub status_focused: bool,
    /// Index of the currently focused status chip.
    pub status_selected: usize,
    /// Plugin indices of active glance-strip status commands.
    pub status_indices: Vec<usize>,
    /// Plugin indices actually shown in the glance strip: the `status_indices`
    /// plus any degraded widgets demoted from their card. Rebuilt from
    /// `status_indices` + `widget_indices` + the result cache. This is what the
    /// strip renders and focuses over (`status_selected` indexes it).
    pub glance_indices: Vec<usize>,
    /// Theme preset picker overlay (None = closed).
    pub theme_picker: Option<ThemePickerState>,
    /// Plugin manager state (shown in `Mode::PluginManager`).
    pub plugin_manager: Option<PluginManagerState>,
    /// Widget picker overlay (shown when adding/removing widgets).
    pub widget_picker: Option<WidgetPickerState>,
    /// Update hint: `Some("0.5.0")` when a newer version is available.
    pub update_hint: Option<String>,
    /// How larkline was installed (for upgrade instructions).
    pub install_method: crate::update::InstallMethod,
    /// Mini app state (active when `mode == Mode::MiniApp`).
    #[allow(dead_code)]
    pub mini_app: Option<MiniAppState>,
}

/// State for mini app mode — full-screen split-pane layout.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MiniAppState {
    /// Plugin index that owns this mini app.
    pub plugin_index: usize,
    /// The layout tree (may be mutated by user splits).
    pub layout: MiniAppLayout,
    /// Per-pane runtime state, keyed by pane ID.
    pub panes: std::collections::HashMap<PaneId, PaneState>,
    /// ID of the currently focused pane.
    pub focused_pane: PaneId,
    /// Ordered list of pane IDs for focus cycling (depth-first leaf order).
    pub pane_order: Vec<PaneId>,
}

/// Runtime state for a single pane in mini app mode.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct PaneState {
    /// The content currently displayed in this pane.
    pub content: PaneContent,
    /// How the pane's content is rendered.
    pub output_mode: OutputMode,
    /// Selected item index within the pane's items list.
    pub selected: usize,
    /// Scroll offset for markdown/raw text modes.
    pub scroll_offset: usize,
    /// Whether this pane is loading.
    pub is_loading: bool,
}

/// A shell action awaiting user confirmation before execution.
#[derive(Debug, Clone)]
pub struct PendingConfirmation {
    /// Human-readable description of the action.
    pub description: String,
    /// Command to run.
    pub command: String,
    /// Arguments to pass.
    pub args: Vec<String>,
}

/// Overlay state for the copy-to-clipboard menu.
#[derive(Debug, Clone)]
pub struct CopyMenuState {
    /// Available entries: `(label, value)` pairs.
    pub entries: Vec<(String, String)>,
    /// Index of the highlighted menu entry.
    pub selected: usize,
}

/// Overlay state for the action palette (searchable action list for an item).
#[derive(Debug, Clone)]
pub struct ActionPaletteState {
    /// All actions available for the selected item (plugin-defined + built-in).
    pub actions: Vec<crate::plugin::traits::ItemAction>,
    /// Index of the highlighted action in the filtered list.
    pub selected: usize,
    /// Search query to filter actions by label.
    pub query: String,
    /// Indices into `actions` that match the current query.
    pub filtered_indices: Vec<usize>,
}

impl ActionPaletteState {
    /// Rebuild filtered indices based on the current query.
    fn rebuild_filter(&mut self) {
        if self.query.is_empty() {
            self.filtered_indices = (0..self.actions.len()).collect();
        } else {
            let q = self.query.to_lowercase();
            self.filtered_indices = self
                .actions
                .iter()
                .enumerate()
                .filter(|(_, a)| a.label.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect();
        }
        let max = self.filtered_indices.len().saturating_sub(1);
        if self.selected > max {
            self.selected = max;
        }
    }
}

/// A single entry in the power menu.
#[derive(Debug, Clone)]
pub struct PowerMenuItem {
    /// Shortcut key to execute this action (e.g., 'y', 'q', '/').
    pub key: char,
    /// Display string for the key hint (e.g., "⏎" for Enter, "SPC" for Space).
    pub key_hint: String,
    /// Human-readable label shown next to the key.
    pub label: String,
    /// The action to dispatch when this item is selected.
    pub action: Action,
}

/// A category group in the power menu.
#[derive(Debug, Clone)]
pub struct PowerMenuCategory {
    /// Category heading (e.g., "Actions", "Display", "App").
    pub name: String,
    /// Items in this category.
    pub items: Vec<PowerMenuItem>,
}

/// Overlay state for the power menu (which-key style).
#[derive(Debug, Clone)]
pub struct PowerMenuState {
    /// Categorized action groups.
    pub categories: Vec<PowerMenuCategory>,
}

/// Which dashboard surface a picker entry manages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerItemKind {
    /// A widget card (top dashboard row).
    Widget,
    /// A glance-strip status chip.
    Status,
}

/// A widget- or status-eligible command for the dashboard picker overlay.
#[derive(Debug, Clone)]
pub struct WidgetPickerEntry {
    /// Display name: "Plugin: Command" or just "Command".
    pub label: String,
    /// Icon from the manifest.
    pub icon: String,
    /// Key used in `disabled_widgets` / `disabled_status` — `"GroupKey:CommandName"`.
    pub key: String,
    /// Whether this item is currently enabled (not in the disabled list).
    pub enabled: bool,
    /// Whether this entry manages a widget card or a status chip.
    pub kind: PickerItemKind,
}

/// Overlay state for the widget picker.
#[derive(Debug, Clone)]
pub struct WidgetPickerState {
    /// All widget-eligible commands.
    pub entries: Vec<WidgetPickerEntry>,
    /// Currently highlighted entry (index into `filtered_indices` if filtering, else `entries`).
    pub selected: usize,
    /// Active search query for filtering widget entries.
    pub query: String,
    /// Indices into `entries` matching the current query. Empty = show all.
    pub filtered_indices: Vec<usize>,
}

impl WidgetPickerState {
    /// Rebuild `filtered_indices` based on the current `query`.
    pub fn rebuild_filter(&mut self) {
        if self.query.is_empty() {
            self.filtered_indices.clear();
        } else {
            let q = self.query.to_lowercase();
            self.filtered_indices = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.label.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect();
        }
    }

    /// Get the visible entries (filtered or all).
    pub fn visible_entries(&self) -> Vec<(usize, &WidgetPickerEntry)> {
        if self.filtered_indices.is_empty() && self.query.is_empty() {
            self.entries.iter().enumerate().collect()
        } else {
            self.filtered_indices
                .iter()
                .filter_map(|&i| self.entries.get(i).map(|e| (i, e)))
                .collect()
        }
    }
}

/// Overlay state for the theme preset picker.
#[derive(Debug, Clone)]
pub struct ThemePickerState {
    /// Index into [`crate::config::PRESET_NAMES`] for the highlighted preset.
    pub selected: usize,
    /// Theme snapshot to restore if the user presses Esc.
    pub original_theme: Theme,
    /// Preset name that was active when the picker opened (for display and revert).
    pub original_preset: Option<String>,
}

/// Mutable state for an active form being filled by the user.
#[derive(Debug, Clone)]
pub struct FormState {
    /// Per-field editing state, parallel to the `FormSpec::fields` vector.
    pub fields: Vec<FormFieldState>,
    /// Index of the currently focused field.
    pub focused: usize,
    /// Plugin index this form belongs to (for re-execution on submit).
    pub plugin_index: usize,
    /// Submit button label (used by the TUI renderer).
    #[allow(dead_code)]
    pub submit_label: String,
    /// When `true`, this form is a settings form — on submit, values are saved to
    /// the plugin's store instead of being passed back for re-execution.
    pub is_settings: bool,
}

/// Editing state for a single form field.
#[derive(Debug, Clone)]
pub struct FormFieldState {
    /// The field specification (id, label, type, etc.).
    pub spec: crate::plugin::traits::FormField,
    /// Current text value.
    pub value: String,
    /// Cursor position within the value string (for text fields).
    pub cursor: usize,
    /// Currently selected option index (for Select fields).
    pub selected_option: usize,
    /// Whether the toggle is on (for Toggle fields).
    pub toggled: bool,
}

/// Source of a plugin secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretSource {
    DotEnv,
    EnvVar,
    Keychain,
    NotSet,
}

/// A row in the plugin manager tree view.
#[derive(Debug, Clone)]
pub enum PluginManagerRow {
    PluginHeader {
        group_key: String,
        name: String,
        icon: String,
        category: String,
        version: String,
        enabled: bool,
        expanded: bool,
        command_count: usize,
    },
    Command {
        group_key: String,
        name: String,
        quickkey: Option<String>,
        enabled: bool,
    },
    Setting {
        #[allow(dead_code)]
        group_key: String,
        #[allow(dead_code)]
        id: String,
        label: String,
        value: String,
    },
    Secret {
        key: String,
        source: SecretSource,
    },
}

/// State for the plugin manager screen.
#[derive(Debug, Clone)]
pub struct PluginManagerState {
    pub rows: Vec<PluginManagerRow>,
    pub selected: usize,
    pub expanded: std::collections::HashSet<String>,
}

/// Maximum entries in the navigation history stack.
const MAX_NAV_HISTORY: usize = 10;

const SLOW_TUI_FRAME_THRESHOLD: std::time::Duration = std::time::Duration::from_millis(16);

#[derive(Debug)]
struct FrameTiming {
    total: std::time::Duration,
    draw: std::time::Duration,
    input_wait: std::time::Duration,
    input_drain: std::time::Duration,
    engine_drain: std::time::Duration,
    input_events: usize,
    engine_events: usize,
}

impl FrameTiming {
    fn active(&self) -> std::time::Duration {
        self.total.saturating_sub(self.input_wait)
    }

    fn is_slow(&self) -> bool {
        self.active() >= SLOW_TUI_FRAME_THRESHOLD
    }

    fn log(&self) {
        let frame_ms = self.active().as_millis();
        if self.is_slow() {
            tracing::warn!(
                frame_ms,
                draw_ms = self.draw.as_millis(),
                input_wait_ms = self.input_wait.as_millis(),
                input_drain_ms = self.input_drain.as_millis(),
                engine_drain_ms = self.engine_drain.as_millis(),
                input_events = self.input_events,
                engine_events = self.engine_events,
                "slow tui frame"
            );
        } else {
            tracing::trace!(
                frame_ms,
                draw_ms = self.draw.as_millis(),
                input_wait_ms = self.input_wait.as_millis(),
                input_drain_ms = self.input_drain.as_millis(),
                engine_drain_ms = self.engine_drain.as_millis(),
                input_events = self.input_events,
                engine_events = self.engine_events,
                "tui frame"
            );
        }
    }
}

fn log_first_paint(startup: std::time::Duration, draw: std::time::Duration, plugin_count: usize) {
    tracing::info!(
        startup_ms = startup.as_millis(),
        draw_ms = draw.as_millis(),
        plugin_count,
        "first paint"
    );
}

/// Snapshot of `ViewOutput` state saved when navigating to another plugin.
#[derive(Debug, Clone)]
pub struct NavigationEntry {
    /// Plugin index that was being viewed.
    pub plugin_index: usize,
    /// The plugin's output at the time of navigation.
    pub plugin_output: Option<PluginOutput>,
    /// Error message, if any.
    pub plugin_error: Option<String>,
    /// Selected item index within the output.
    pub output_selected: usize,
    /// How the output was being displayed.
    pub output_mode: OutputMode,
    /// Scroll offset for Markdown/RawText modes.
    pub scroll_offset: usize,
}

// ---------------------------------------------------------------------------
// App runner
// ---------------------------------------------------------------------------

/// The main application runner.
/// Completion of a command that ran on the background channel.
#[derive(Debug)]
pub(crate) enum BgCommandEvent {
    /// A shell action finished (or failed to spawn).
    ShellDone {
        /// The command that ran (for the title / flash message).
        command: String,
        /// `viewing_plugin_index` at dispatch time — a completion arriving
        /// after the user navigated away degrades to a flash message instead
        /// of stealing the current view.
        dispatched_view: Option<usize>,
        /// The child's captured output, or the spawn error.
        result: std::io::Result<std::process::Output>,
    },
    /// An nvim remote-send finished.
    NvimDone {
        /// The file path that was opened.
        path: String,
        /// Outcome — `NotUnderNvim` falls back to the system handler.
        result: Result<(), NvimOpenError>,
    },
    /// A background plugin-registry rescan finished (`RefreshPlugins`).
    RefreshScanDone {
        /// The scan inputs for the engine rebuild, or the scan error.
        result: Result<Box<RefreshScan>, String>,
    },
    /// The plugin-manager snapshot (scan + keychain presence) is ready.
    PmSnapshotReady {
        /// Gathered inputs for pure row rebuilding on later keypresses.
        snapshot: Box<crate::plugin_manager_state::PmSnapshot>,
    },
}

/// Everything a `RefreshPlugins` engine rebuild needs, gathered off the UI
/// task: the registry scan, the plugin-manager config it was filtered
/// against, and secrets with keychain fallbacks resolved (each keychain
/// lookup shells out to `security`).
#[derive(Debug)]
pub(crate) struct RefreshScan {
    discovered: Vec<crate::plugin::registry::DiscoveredPlugin>,
    pm_config: crate::config::PluginManagerConfig,
    secrets: std::collections::HashMap<String, String>,
}

/// Gather refresh inputs (registry scan + config + secret resolution).
/// Blocking — run via `spawn_blocking` on the async path.
fn scan_for_refresh(
    plugin_dirs: &[PathBuf],
    icon_set: &crate::config::IconSet,
) -> Result<RefreshScan, String> {
    let mut discovered = registry::scan(plugin_dirs).map_err(|e| e.to_string())?;
    // Resolve icons based on configured icon set.
    if *icon_set == crate::config::IconSet::Nerd {
        for d in &mut discovered {
            if let Some(ref nerd) = d.metadata.icon_nerd {
                if !nerd.is_empty() {
                    d.metadata.icon = nerd.clone();
                }
            }
        }
    }
    // Filter out disabled plugins/commands.
    let pm_config = crate::config::load_plugin_manager_config();
    let discovered: Vec<_> = discovered
        .into_iter()
        .filter(|d| {
            let gk = d
                .metadata
                .plugin_group
                .as_deref()
                .unwrap_or(&d.metadata.name);
            if pm_config.is_plugin_disabled(gk) {
                return false;
            }
            !pm_config.is_command_disabled(gk, &d.metadata.name)
        })
        .collect();
    // Reload secrets (with keychain fallback — `security` subprocess per key).
    let mut secrets = crate::config::load_secrets();
    let mut declared_keys: Vec<&str> = discovered
        .iter()
        .flat_map(|d| d.metadata.secrets.iter().map(String::as_str))
        .collect();
    declared_keys.extend(crate::config::AI_SECRET_KEYS);
    crate::config::resolve_keychain_secrets(&mut secrets, &declared_keys);
    Ok(RefreshScan {
        discovered,
        pm_config,
        secrets,
    })
}

pub struct App {
    pub(crate) state: AppState,
    pub(crate) theme: Theme,
    pub(crate) keybindings: ResolvedKeybindings,
    pub(crate) engine: PluginEngine,
    pub(crate) rx: mpsc::Receiver<EngineEvent>,
    registry_generation: u64,
    latest_plugin_executions: std::collections::HashMap<(usize, ExecutionSource), ExecutionStamp>,
    latest_cache_executions: std::collections::HashMap<usize, ExecutionStamp>,
    cache_executions: std::collections::HashMap<(usize, ExecutionSource), CacheExecution>,
    latest_action_executions: std::collections::HashMap<usize, ExecutionStamp>,
    /// Single-flight guard: the latest still-running prefetch per plugin.
    /// Inserted at dispatch, cleared when its `PluginFinished` arrives, so
    /// the due-scan skips plugins slower than their refresh interval instead
    /// of piling up executions. (mkv.11 subsumes this into the canonical
    /// in-flight registry keyed by `command_id`.)
    prefetch_in_flight: std::collections::HashMap<usize, ExecutionStamp>,
    /// Background-command channel: shell actions and nvim remote-sends run
    /// off the event-loop task and report back as [`BgCommandEvent`]s, so a
    /// slow or interactive child never freezes the UI.
    bg_tx: mpsc::Sender<BgCommandEvent>,
    bg_rx: mpsc::Receiver<BgCommandEvent>,
    /// A `RefreshPlugins` scan is running in the background — a second `R`
    /// while it runs is dropped instead of piling up engine rebuilds.
    refresh_in_flight: bool,
    /// Cached plugin-manager inputs (scan + keychain presence), gathered in
    /// the background at manager open so expand/toggle keypresses rebuild
    /// rows without a registry scan or `security` subprocess.
    pub(crate) pm_snapshot: Option<crate::plugin_manager_state::PmSnapshot>,
    /// Plugin directories for re-scanning on refresh.
    pub(crate) plugin_dirs: Vec<PathBuf>,
    /// Raw keybindings config for re-resolving after refresh.
    pub(crate) keybindings_config: KeybindingsConfig,
    /// Icon set preference for resolving Nerd Font vs emoji icons.
    pub(crate) icon_set: crate::config::IconSet,
    /// Secrets loaded from `~/.config/larkline/.env`.
    pub(crate) secrets: std::collections::HashMap<String, String>,
    /// Currently active theme preset name (e.g. `"nord"`). `None` = default.
    pub(crate) current_preset: Option<String>,
    /// Plugin manager enable/disable config.
    pub(crate) pm_config: crate::config::PluginManagerConfig,
}

impl App {
    /// Create a new `App` with the given set of plugins and config.
    pub fn new(
        plugins: Vec<Arc<dyn Plugin>>,
        config: &Config,
        warnings: Vec<String>,
        secrets: std::collections::HashMap<String, String>,
    ) -> Self {
        let plugin_count = plugins.len();
        let (tx, rx) = mpsc::channel(plugin_count.max(1) * 3);
        let (bg_tx, bg_rx) = mpsc::channel(16);
        let metadata: Vec<PluginMetadata> = plugins.iter().map(|p| p.metadata().clone()).collect();
        let registry_generation = 0;
        let engine = PluginEngine::new(plugins, tx, secrets.clone(), registry_generation);
        // Resolve theme; fall back to defaults on invalid colors.
        let theme = config.theme.resolve().unwrap_or_else(|e| {
            tracing::warn!(error = %e, "invalid theme color, falling back to defaults");
            Theme::default_theme()
        });
        // Resolve keybindings (uses plugin metadata for launch map).
        let keybindings = config.keybindings.resolve(&metadata);

        let mut app = Self {
            state: AppState {
                plugins: metadata,
                show_icons: config.ui.show_icons,
                show_descriptions: config.ui.show_descriptions,
                sidebar_ratio: config.ui.sidebar_ratio.clamp(20, 80),
                favorites: config.favorites.pinned.clone(),
                warnings,
                max_items_per_section: config.ui.max_items_per_section,
                result_cache: std::collections::HashMap::new(),
                unified_rows: Vec::new(),
                unified_selected: 0,
                status_message: None,
                ..Default::default()
            },
            theme,
            keybindings,
            engine,
            rx,
            registry_generation,
            latest_plugin_executions: std::collections::HashMap::new(),
            latest_cache_executions: std::collections::HashMap::new(),
            cache_executions: std::collections::HashMap::new(),
            latest_action_executions: std::collections::HashMap::new(),
            prefetch_in_flight: std::collections::HashMap::new(),
            bg_tx,
            bg_rx,
            refresh_in_flight: false,
            pm_snapshot: None,
            plugin_dirs: config.general.plugin_dirs.clone(),
            keybindings_config: config.keybindings.clone(),
            icon_set: config.ui.icon_set.clone(),
            secrets,
            current_preset: config.theme.preset.clone(),
            pm_config: crate::config::load_plugin_manager_config(),
        };
        app.rebuild_unified_list();
        crate::widgets::rebuild_widget_indices(&mut app.state, &app.pm_config);
        crate::widgets::rebuild_status_indices(&mut app.state, &app.pm_config);
        crate::widgets::rebuild_glance_indices(&mut app.state);

        // Apply default_plugin pre-selection: find the first Command row with the named plugin.
        if let Some(ref name) = config.general.default_plugin {
            let row_pos = app
                .state
                .unified_rows
                .iter()
                .enumerate()
                .find(|(_, r)| {
                    matches!(r, UnifiedRow::Command { plugin_index, .. }
                        if app.state.plugins[*plugin_index].name == *name)
                })
                .map(|(i, _)| i);
            if let Some(pos) = row_pos {
                app.state.unified_selected = pos;
                crate::widgets::sync_preview_index(&mut app.state);
            } else {
                tracing::warn!(
                    plugin_name = %name,
                    "default_plugin not found in loaded plugins"
                );
            }
        }

        app
    }

    fn track_plugin_execution(
        &mut self,
        plugin_index: usize,
        source: ExecutionSource,
        stamp: ExecutionStamp,
    ) {
        if source == ExecutionSource::Prefetch {
            self.prefetch_in_flight.insert(plugin_index, stamp);
        }
        self.latest_plugin_executions
            .insert((plugin_index, source), stamp);
        self.latest_cache_executions.insert(plugin_index, stamp);
    }

    #[allow(clippy::too_many_lines)]
    fn apply_cache_event(&mut self, event: CacheEvent) -> CacheTransition {
        match event {
            CacheEvent::Open {
                plugin_index,
                cache_enabled,
            } => {
                let current = self.state.result_cache.get(&plugin_index).cloned();
                let open = match current {
                    Some(CachedResult::Ready(output)) if cache_enabled => {
                        self.state
                            .result_cache
                            .insert(plugin_index, CachedResult::Revalidating(output.clone()));
                        CacheOpen::ShowAndRevalidate(output)
                    }
                    Some(CachedResult::Ready(_)) => {
                        self.state.result_cache.remove(&plugin_index);
                        CacheOpen::Miss
                    }
                    Some(CachedResult::Revalidating(output)) => CacheOpen::ShowRevalidating(output),
                    Some(CachedResult::Loading(_)) => CacheOpen::Loading,
                    Some(CachedResult::Error(error)) => CacheOpen::Error(error),
                    None => CacheOpen::Miss,
                };
                CacheTransition::Open(open)
            }
            CacheEvent::Started {
                plugin_index,
                stamp,
                source,
            } => {
                let was_revalidating = matches!(
                    self.state.result_cache.get(&plugin_index),
                    Some(CachedResult::Ready(_) | CachedResult::Revalidating(_))
                );
                let owns_cache = self.latest_cache_executions.get(&plugin_index) == Some(&stamp);
                if owns_cache {
                    let next = match self.state.result_cache.remove(&plugin_index) {
                        Some(CachedResult::Ready(output) | CachedResult::Revalidating(output)) => {
                            CachedResult::Revalidating(output)
                        }
                        _ => CachedResult::Loading(std::time::Instant::now()),
                    };
                    self.state.result_cache.insert(plugin_index, next);
                }
                self.cache_executions.insert(
                    (plugin_index, source),
                    CacheExecution {
                        stamp,
                        was_revalidating,
                        streamed_output: None,
                    },
                );
                CacheTransition::Started { was_revalidating }
            }
            CacheEvent::Partial {
                plugin_index,
                stamp,
                source,
                title,
                items,
            } => {
                let key = (plugin_index, source);
                let execution =
                    self.cache_executions
                        .entry(key)
                        .or_insert_with(|| CacheExecution {
                            stamp,
                            was_revalidating: matches!(
                                self.state.result_cache.get(&plugin_index),
                                Some(CachedResult::Revalidating(_))
                            ),
                            streamed_output: None,
                        });
                if execution.stamp != stamp {
                    return CacheTransition::None;
                }

                let replace = title.is_some() || execution.streamed_output.is_none();
                if let Some(title) = title {
                    execution.streamed_output = Some(PluginOutput {
                        title,
                        items,
                        ..Default::default()
                    });
                } else {
                    execution
                        .streamed_output
                        .get_or_insert_with(PluginOutput::default)
                        .items
                        .extend(items);
                }
                let output = execution.streamed_output.clone().unwrap_or_default();
                if self.latest_cache_executions.get(&plugin_index) == Some(&stamp) {
                    self.state
                        .result_cache
                        .insert(plugin_index, CachedResult::Ready(output.clone()));
                }
                CacheTransition::Partial { output, replace }
            }
            CacheEvent::Finished {
                plugin_index,
                stamp,
                source,
                result,
                cache_enabled,
            } => {
                let key = (plugin_index, source);
                let execution = self
                    .cache_executions
                    .get(&key)
                    .is_some_and(|execution| execution.stamp == stamp)
                    .then(|| self.cache_executions.remove(&key))
                    .flatten();
                let was_revalidating = execution.as_ref().is_some_and(|e| e.was_revalidating)
                    || matches!(
                        self.state.result_cache.get(&plugin_index),
                        Some(CachedResult::Revalidating(_))
                    );
                let result = match *result {
                    Ok(output) => Ok(execution
                        .and_then(|execution| execution.streamed_output)
                        .unwrap_or(output)),
                    Err(error) => Err(error.to_string()),
                };

                if self.latest_cache_executions.get(&plugin_index) == Some(&stamp) {
                    match &result {
                        Ok(output) if cache_enabled => {
                            self.state
                                .result_cache
                                .insert(plugin_index, CachedResult::Ready(output.clone()));
                        }
                        Ok(_) => {
                            self.state.result_cache.remove(&plugin_index);
                        }
                        Err(error) => {
                            self.state
                                .result_cache
                                .insert(plugin_index, CachedResult::Error(error.clone()));
                        }
                    }
                }

                CacheTransition::Finished {
                    was_revalidating,
                    result,
                }
            }
            CacheEvent::Invalidate { plugin_index } => {
                self.state.result_cache.remove(&plugin_index);
                self.latest_cache_executions.remove(&plugin_index);
                self.cache_executions
                    .retain(|(index, _), _| *index != plugin_index);
                CacheTransition::None
            }
            CacheEvent::Clear => {
                self.state.result_cache.clear();
                self.latest_cache_executions.clear();
                self.cache_executions.clear();
                CacheTransition::None
            }
        }
    }

    pub(crate) fn invalidate_plugin_cache(&mut self, plugin_index: usize) {
        self.apply_cache_event(CacheEvent::Invalidate { plugin_index });
    }

    fn clear_plugin_cache(&mut self) {
        self.apply_cache_event(CacheEvent::Clear);
    }

    pub(crate) fn dispatch_plugin(&mut self, plugin_index: usize) {
        let stamp = self.engine.execute(plugin_index);
        self.track_plugin_execution(plugin_index, ExecutionSource::UserSelected, stamp);
    }

    fn dispatch_prefetch(&mut self, plugin_index: usize) {
        let stamp = self.engine.refresh(plugin_index);
        self.track_plugin_execution(plugin_index, ExecutionSource::Prefetch, stamp);
    }

    /// Widget indices due for auto-refresh at `now`: refresh interval elapsed
    /// (or never refreshed) and no prefetch already in flight.
    fn due_widget_refreshes(&self, now: std::time::Instant) -> Vec<usize> {
        self.state
            .widget_indices
            .iter()
            .copied()
            .filter(|pidx| {
                let Some(meta) = self.state.plugins.get(*pidx) else {
                    return false;
                };
                // Same absent-key fix as the status strip: unwrap_or(now)
                // made widgets never auto-refresh on their interval (only
                // at startup / manual R). is_none_or(true) treats a
                // never-refreshed index as due.
                meta.widget_refresh_secs > 0
                    && !self.prefetch_in_flight.contains_key(pidx)
                    && self
                        .state
                        .widget_last_refresh
                        .get(pidx)
                        .copied()
                        .is_none_or(|t| now.duration_since(t).as_secs() >= meta.widget_refresh_secs)
            })
            .collect()
    }

    /// Glance-strip status indices due for auto-refresh at `now`.
    fn due_status_refreshes(&self, now: std::time::Instant) -> Vec<usize> {
        self.state
            .status_indices
            .iter()
            .copied()
            .filter(|pidx| {
                // Bounds-safe: status_indices may briefly lag state.plugins
                // (e.g. between a plugin-list rebuild and index rebuild).
                let Some(meta) = self.state.plugins.get(*pidx) else {
                    return false;
                };
                let interval = if meta.status_refresh_secs > 0 {
                    meta.status_refresh_secs
                } else {
                    30
                };
                // An absent last-refresh key means "never refreshed" → due
                // now. (unwrap_or(now) would make duration 0 < interval, so
                // the chip would never auto-refresh — froze the countdown.)
                !self.prefetch_in_flight.contains_key(pidx)
                    && self
                        .state
                        .status_last_refresh
                        .get(pidx)
                        .copied()
                        .is_none_or(|t| now.duration_since(t).as_secs() >= interval)
            })
            .collect()
    }

    fn dispatch_all_prefetch(&mut self) {
        let now = std::time::Instant::now();
        for (plugin_index, stamp) in self.engine.execute_all() {
            self.track_plugin_execution(plugin_index, ExecutionSource::Prefetch, stamp);
            // Seed refresh timestamps for what was actually dispatched, so
            // the first due-scan doesn't immediately re-dispatch everything
            // startup just ran (startup double-dispatch).
            if let Some(meta) = self.state.plugins.get(plugin_index) {
                if meta.widget {
                    self.state.widget_last_refresh.insert(plugin_index, now);
                }
                if meta.status {
                    self.state.status_last_refresh.insert(plugin_index, now);
                }
            }
        }
    }

    pub(crate) fn dispatch_plugin_with_form(
        &mut self,
        plugin_index: usize,
        form_values: std::collections::HashMap<String, String>,
    ) {
        let stamp = self.engine.execute_with_form(plugin_index, form_values);
        self.track_plugin_execution(plugin_index, ExecutionSource::UserSelected, stamp);
    }

    fn dispatch_plugin_action(
        &mut self,
        plugin_index: usize,
        callback_id: String,
        context: String,
    ) {
        let stamp = self
            .engine
            .execute_action(plugin_index, callback_id, context);
        self.latest_action_executions.insert(plugin_index, stamp);
    }

    fn invalidate_viewing_execution(&mut self) {
        if let Some(plugin_index) = self.state.viewing_plugin_index {
            self.latest_plugin_executions
                .remove(&(plugin_index, ExecutionSource::UserSelected));
            self.latest_action_executions.remove(&plugin_index);
        }
        self.state.is_loading = false;
        self.state.loading_started = None;
    }

    fn is_current_engine_event(&self, event: &EngineEvent) -> bool {
        let stamp = match event {
            EngineEvent::PluginStarted { stamp, .. }
            | EngineEvent::PluginFinished { stamp, .. }
            | EngineEvent::PartialOutput { stamp, .. }
            | EngineEvent::ActionResult { stamp, .. } => stamp,
        };
        if stamp.registry_generation != self.registry_generation {
            return false;
        }

        match event {
            EngineEvent::PluginStarted {
                plugin_index,
                stamp,
                source,
            }
            | EngineEvent::PluginFinished {
                plugin_index,
                stamp,
                source,
                ..
            }
            | EngineEvent::PartialOutput {
                plugin_index,
                stamp,
                source,
                ..
            } => self.latest_plugin_executions.get(&(*plugin_index, *source)) == Some(stamp),
            EngineEvent::ActionResult {
                plugin_index,
                stamp,
                ..
            } => self.latest_action_executions.get(plugin_index) == Some(stamp),
        }
    }

    /// Set an initial search query (from `--query` CLI flag).
    pub fn set_initial_query(&mut self, query: &str) {
        self.state.query = query.to_string();
        self.state.vim_mode = VimMode::Insert;
        self.rebuild_unified_list();
    }

    /// Create an `App` with stub plugins for testing.
    #[cfg(test)]
    pub fn with_stubs() -> Self {
        Self::new(
            stub_plugins(),
            &Config::default(),
            Vec::new(),
            std::collections::HashMap::new(),
        )
    }

    /// Create an `App` with stub plugins and a favorites list for testing.
    #[cfg(test)]
    pub fn with_stubs_and_favorites(pinned: Vec<String>) -> Self {
        use crate::config::FavoritesConfig;
        let config = Config {
            favorites: FavoritesConfig { pinned },
            ..Config::default()
        };
        Self::new(
            stub_plugins(),
            &config,
            Vec::new(),
            std::collections::HashMap::new(),
        )
    }

    /// Create an `App` with stub plugins and a `default_plugin` setting for testing.
    #[cfg(test)]
    pub fn with_stubs_and_default(default_plugin: &str) -> Self {
        let mut config = Config::default();
        config.general.default_plugin = Some(default_plugin.to_string());
        Self::new(
            stub_plugins(),
            &config,
            Vec::new(),
            std::collections::HashMap::new(),
        )
    }

    /// Run the main event loop until the user quits.
    // The event loop uses crossterm's sync poll + tokio::spawn for plugins.
    // No direct .await calls here, but `run` must be async so main can await it.
    #[allow(clippy::unused_async, clippy::too_many_lines)]
    pub async fn run(
        mut self,
        terminal: &mut DefaultTerminal,
        startup_started: std::time::Instant,
    ) -> Result<()> {
        // Kick off background prefetch for all eligible plugins.
        self.dispatch_all_prefetch();

        // Detect install method and check for updates in the background.
        self.state.install_method = crate::update::detect_install_method();
        self.state.update_hint = crate::update::cached_update_available();

        let (update_tx, update_rx) = tokio::sync::oneshot::channel::<Option<String>>();
        let needs_check = self.state.update_hint.is_none()
            && !crate::update::load_cache().is_some_and(|c| crate::update::cache_is_fresh(&c));
        if needs_check {
            tokio::spawn(async move {
                let _ = update_tx.send(crate::update::check_for_update().await);
            });
        } else {
            drop(update_tx);
        }
        let mut update_rx = Some(update_rx);
        let mut first_paint_pending = true;

        while !self.state.should_quit {
            let frame_started = std::time::Instant::now();
            self.refresh_markdown_cache();
            self.refresh_ansi_cache();
            let draw_started = std::time::Instant::now();
            terminal.draw(|frame| ui::render(frame, &self.state, &self.theme))?;
            let draw = draw_started.elapsed();
            if first_paint_pending {
                log_first_paint(startup_started.elapsed(), draw, self.state.plugins.len());
                first_paint_pending = false;
            }

            // Block up to 16ms waiting for input, then drain every queued event
            // before re-rendering. Draining matters on terminals that emit
            // non-Key events (resize, paste, focus) interleaved with keys —
            // otherwise each non-Key event wastes a frame and the next press
            // appears to "not register" until another key is pressed.
            let input_wait_started = std::time::Instant::now();
            let input_ready = event::poll(std::time::Duration::from_millis(16))?;
            let input_wait = input_wait_started.elapsed();
            let input_drain_started = std::time::Instant::now();
            let mut input_events = 0;
            if input_ready {
                loop {
                    let event = event::read()?;
                    input_events += 1;
                    match event {
                        Event::Key(key)
                            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                        {
                            if let Some(action) = input::handle_key(
                                key,
                                &self.state.mode,
                                &self.state.vim_mode,
                                &self.keybindings,
                                self.state.pending_confirmation.is_some(),
                                self.state.copy_menu.is_some(),
                                self.state.output_searching,
                                self.state.form_state.is_some(),
                                self.state.action_palette.is_some(),
                                self.state.theme_picker.is_some(),
                                self.state.widget_picker.is_some(),
                                self.state
                                    .power_menu
                                    .as_ref()
                                    .map(|m| m.categories.as_slice()),
                                self.state.pending_g,
                                self.state.widget_focused,
                                self.state.status_focused,
                            ) {
                                if !matches!(action, Action::PendingG) {
                                    self.state.pending_g = false;
                                }
                                self.handle_action(action);
                            } else {
                                self.state.pending_g = false;
                            }
                        }
                        _ => {}
                    }
                    if !event::poll(std::time::Duration::ZERO)? {
                        break;
                    }
                }
            }
            let input_drain = input_drain_started.elapsed();

            // Drain engine events (non-blocking).
            let engine_drain_started = std::time::Instant::now();
            let mut engine_events = 0;
            while let Ok(event) = self.rx.try_recv() {
                engine_events += 1;
                self.handle_engine_event(event);
            }
            let engine_drain = engine_drain_started.elapsed();

            // Drain background-command completions (non-blocking).
            while let Ok(event) = self.bg_rx.try_recv() {
                self.handle_bg_command_event(event);
            }

            // Advance spinner.
            if self.state.is_loading {
                self.state.spinner_tick = self.state.spinner_tick.wrapping_add(1);
            }

            // Expire flash messages after 2 seconds.
            if let Some((_, ref started)) = self.state.status_message {
                if started.elapsed().as_secs_f32() >= 2.0 {
                    self.state.status_message = None;
                }
            }

            // Auto-refresh widget plugins on their configured interval.
            // Skip entirely when the dashboard isn't visible: widgets toggled off,
            // a non-Unified mode active, or no enabled widgets exist. Iterate the
            // pre-computed widget_indices instead of rescanning every plugin.
            if self.state.mode == Mode::Unified
                && self.state.widgets_visible
                && !self.state.widget_indices.is_empty()
            {
                let now = std::time::Instant::now();
                let due = self.due_widget_refreshes(now);
                if !due.is_empty() {
                    for pidx in &due {
                        self.state.widget_last_refresh.insert(*pidx, now);
                        // Background refresh: updates the card's cache without
                        // hijacking the foreground view (execute() = UserSelected
                        // would force the main pane to the refreshed widget).
                        self.dispatch_prefetch(*pidx);
                    }
                    self.rebuild_unified_list();
                }
            }

            // Auto-refresh glance-strip status plugins on their interval. Not
            // gated to Unified mode — the chip should stay fresh wherever the
            // strip is shown (e.g. a caffeinate countdown while browsing).
            if self.state.status_visible && !self.state.status_indices.is_empty() {
                let now = std::time::Instant::now();
                let due = self.due_status_refreshes(now);
                for pidx in &due {
                    self.state.status_last_refresh.insert(*pidx, now);
                    // Background refresh — must not steal the foreground view.
                    self.dispatch_prefetch(*pidx);
                }
            }

            // Check for background update result (non-blocking).
            if let Some(ref mut rx) = update_rx {
                if let Ok(result) = rx.try_recv() {
                    if let Some(version) = result {
                        self.state.update_hint = Some(version);
                    }
                    update_rx = None;
                }
            }

            FrameTiming {
                total: frame_started.elapsed(),
                draw,
                input_wait,
                input_drain,
                engine_drain,
                input_events,
                engine_events,
            }
            .log();
        }

        Ok(())
    }

    /// Process a single engine event, updating app state.
    ///
    /// Extracted from the run loop so it can be called from tests.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn handle_engine_event(&mut self, event: EngineEvent) {
        if !self.is_current_engine_event(&event) {
            return;
        }
        // Only the viewed plugin's events can change the viewed document —
        // scope the markdown-cache invalidation so an unrelated background
        // widget/status refresh doesn't force a full re-parse next frame.
        // (Arms that replace `plugin_output` for other reasons carry their
        // own targeted `markdown_cache = None`.)
        let event_plugin_index = match &event {
            EngineEvent::PluginStarted { plugin_index, .. }
            | EngineEvent::PluginFinished { plugin_index, .. }
            | EngineEvent::PartialOutput { plugin_index, .. }
            | EngineEvent::ActionResult { plugin_index, .. } => *plugin_index,
        };
        if self.state.viewing_plugin_index == Some(event_plugin_index) {
            self.state.markdown_cache = None;
        }
        match event {
            EngineEvent::PluginStarted {
                plugin_index,
                stamp,
                source,
                ..
            } => {
                let CacheTransition::Started { was_revalidating } =
                    self.apply_cache_event(CacheEvent::Started {
                        plugin_index,
                        stamp,
                        source,
                    })
                else {
                    return;
                };
                if source == ExecutionSource::UserSelected && !was_revalidating {
                    self.state.is_loading = true;
                    self.state.loading_started = Some(std::time::Instant::now());
                    self.state.plugin_output = None;
                    self.state.plugin_error = None;
                }
            }
            EngineEvent::PartialOutput {
                plugin_index,
                stamp,
                title,
                items,
                source,
                ..
            } => {
                let CacheTransition::Partial { output, replace } =
                    self.apply_cache_event(CacheEvent::Partial {
                        plugin_index,
                        stamp,
                        source,
                        title,
                        items,
                    })
                else {
                    return;
                };
                if source == ExecutionSource::UserSelected
                    && self.state.viewing_plugin_index == Some(plugin_index)
                {
                    self.state.plugin_output = Some(output.clone());
                    self.state.plugin_error = None;
                    if replace {
                        self.state.mode = Mode::ViewOutput;
                        self.state.output_selected = 0;
                        self.state.output_mode = crate::app_output::output_mode_for(&output);
                    }
                    crate::app_output::rebuild_output_filter(&mut self.state);
                }
            }
            EngineEvent::PluginFinished {
                plugin_index,
                stamp,
                result,
                source,
                ..
            } => {
                // Free the single-flight guard only for the execution that
                // owns it — a stale finish must not mark a newer in-flight
                // prefetch as done.
                if source == ExecutionSource::Prefetch
                    && self.prefetch_in_flight.get(&plugin_index) == Some(&stamp)
                {
                    self.prefetch_in_flight.remove(&plugin_index);
                }
                let cache_enabled = self.state.plugins.get(plugin_index).is_none_or(|p| p.cache);
                let CacheTransition::Finished {
                    was_revalidating,
                    result,
                } = self.apply_cache_event(CacheEvent::Finished {
                    plugin_index,
                    stamp,
                    source,
                    result: Box::new(result),
                    cache_enabled,
                })
                else {
                    return;
                };

                match source {
                    ExecutionSource::Prefetch => {
                        // Refresh widget summaries when prefetch results arrive.
                        if self
                            .state
                            .plugins
                            .get(plugin_index)
                            .is_some_and(|m| m.widget)
                        {
                            self.rebuild_unified_list();
                        }
                        // A result that just changed health (degraded ↔ healthy)
                        // moves a widget between its card and a strip warning chip.
                        crate::widgets::rebuild_glance_indices(&mut self.state);
                    }
                    ExecutionSource::UserSelected => {
                        self.state.is_loading = false;
                        self.state.loading_started = None;

                        match result {
                            Ok(output) => {
                                // Record to command history on successful execution.
                                if let Some(meta) = self.state.plugins.get(plugin_index) {
                                    let plugin_name =
                                        meta.plugin_group.as_deref().unwrap_or(&meta.name);
                                    crate::history::record(plugin_name, &meta.name);
                                }

                                if was_revalidating {
                                    // Seamlessly update the pane if the user is still viewing it.
                                    if self.state.viewing_plugin_index == Some(plugin_index) {
                                        self.state.output_mode =
                                            crate::app_output::output_mode_for(&output);
                                        self.state.plugin_output = Some(output);
                                        crate::app_output::rebuild_output_filter(&mut self.state);
                                        crate::app_output::check_form_init(
                                            &mut self.state,
                                            plugin_index,
                                        );
                                    }
                                } else {
                                    // Fresh load: don't overwrite streaming output.
                                    if self.state.plugin_output.is_none() {
                                        self.state.plugin_output = Some(output);
                                        crate::app_output::rebuild_output_filter(&mut self.state);
                                        crate::app_output::check_form_init(
                                            &mut self.state,
                                            plugin_index,
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                if !was_revalidating {
                                    self.state.plugin_error = Some(e);
                                }
                            }
                        }

                        if !was_revalidating {
                            // Check if this plugin declares mini_app and returned a layout.
                            let has_layout = self
                                .state
                                .plugin_output
                                .as_ref()
                                .is_some_and(|o| o.layout.is_some());
                            let is_mini_app = self
                                .state
                                .plugins
                                .get(plugin_index)
                                .is_some_and(|p| p.mini_app);

                            if is_mini_app && has_layout {
                                let layout = self
                                    .state
                                    .plugin_output
                                    .as_ref()
                                    .and_then(|o| o.layout.clone())
                                    .expect("checked above");
                                self.state.mini_app = Some(crate::mini_app::build_mini_app_state(
                                    plugin_index,
                                    layout,
                                ));
                                self.state.mode = Mode::MiniApp;
                            } else {
                                if self.state.mode != Mode::ViewOutput {
                                    self.state.mode = Mode::ViewOutput;
                                }
                                self.state.output_selected = 0;
                                // Detect Markdown/Table/RawText/List from the
                                // output shape (single source of truth) so a
                                // markdown response renders instead of showing
                                // literal `**bold**` in RawText/List mode.
                                self.state.output_mode = self
                                    .state
                                    .plugin_output
                                    .as_ref()
                                    .map_or(OutputMode::List, crate::app_output::output_mode_for);
                                self.state.markdown_cache = None;
                            }
                        }
                    }
                }
            }

            EngineEvent::ActionResult {
                plugin_index,
                result,
                ..
            } => {
                self.state.is_loading = false;
                match result {
                    Ok(output) => {
                        if self.state.mode == Mode::MiniApp {
                            // In mini app mode: if the result has a layout, rebuild the entire
                            // mini app state. Otherwise use the title as a pane ID hint to
                            // update a single pane's content.
                            if let Some(ref layout) = output.layout {
                                self.state.mini_app = Some(crate::mini_app::build_mini_app_state(
                                    plugin_index,
                                    layout.clone(),
                                ));
                            } else if let Some(ref mut mini) = self.state.mini_app {
                                // Use the output title as target pane ID.
                                let target = &output.title;
                                if let Some(pane) = mini.panes.get_mut(target) {
                                    pane.content = crate::plugin::traits::PaneContent {
                                        title: output.title.clone(),
                                        items: output.items,
                                        raw_text: output.raw_text,
                                        columns: output.columns,
                                        form: output.form,
                                        output_format: output.output_format,
                                    };
                                    pane.selected = 0;
                                    pane.scroll_offset = 0;
                                }
                            }
                            self.state.status_message =
                                Some(("Action completed".to_string(), std::time::Instant::now()));
                        } else if self.state.viewing_plugin_index == Some(plugin_index) {
                            // ViewOutput mode: replace entire output.
                            self.state.output_selected = 0;
                            self.state.scroll_offset = 0;
                            // Detect Markdown/Table/RawText/List from the
                            // output shape — single source of truth so
                            // markdown responses render (not literal `**`).
                            self.state.output_mode = crate::app_output::output_mode_for(&output);
                            self.state.markdown_cache = None;
                            self.state.plugin_output = Some(output);
                            // Reset any active output-search filter: its indices
                            // point into the OLD items list, so leaving them would
                            // hide/misselect rows in the replaced output.
                            crate::app_output::reset_output_search(&mut self.state);
                            self.state.status_message =
                                Some(("Action completed".to_string(), std::time::Instant::now()));
                        }
                    }
                    Err(e) => {
                        self.state.status_message =
                            Some((format!("Action failed: {e}"), std::time::Instant::now()));
                    }
                }
            }
        }
    }

    /// Apply an [`Action`] to the application state.
    #[allow(clippy::too_many_lines)]
    pub fn handle_action(&mut self, action: Action) {
        // Dismiss any config warnings on the first keypress.
        self.state.warnings.clear();

        // Auto-dismiss power menu when any other action fires from it.
        if self.state.power_menu.is_some()
            && !matches!(action, Action::PowerMenuOpen | Action::PowerMenuDismiss)
        {
            self.state.power_menu = None;
        }

        match action {
            Action::Quit => self.state.should_quit = true,

            Action::MoveUp => {
                // Widget focused: k stays in widgets (no-op at top).
                // But overlays take priority.
                if self.state.widget_focused
                    && self.state.action_palette.is_none()
                    && self.state.copy_menu.is_none()
                    && self.state.theme_picker.is_none()
                {
                    return;
                }
                // Glance strip focused: k steps up/out to the command list
                // (the strip is the bottom-most row). Otherwise j/k would
                // silently scroll the hidden list while the chip stays lit.
                if self.state.status_focused
                    && self.state.action_palette.is_none()
                    && self.state.copy_menu.is_none()
                    && self.state.theme_picker.is_none()
                {
                    self.state.status_focused = false;
                    return;
                }
                // Action palette is its own list — navigate within it, not
                // the rows behind it. Must come before mode-based branches
                // since the palette renders over ViewOutput.
                if let Some(ref mut palette) = self.state.action_palette {
                    palette.selected = palette.selected.saturating_sub(1);
                    return;
                }
                if let Some(ref mut pm) = self.state.plugin_manager {
                    if self.state.mode == Mode::PluginManager {
                        pm.selected = pm.selected.saturating_sub(1);
                        return;
                    }
                }
                let picker_idx = if let Some(ref mut p) = self.state.theme_picker {
                    if p.selected > 0 {
                        p.selected -= 1;
                    }
                    Some(p.selected)
                } else {
                    None
                };
                if let Some(idx) = picker_idx {
                    let (id, _) = crate::config::PRESET_NAMES[idx];
                    self.theme = crate::config::ThemeConfig::resolve_preset(id);
                } else if let Some(ref mut menu) = self.state.copy_menu {
                    menu.selected = menu.selected.saturating_sub(1);
                } else if self.state.mode == Mode::MiniApp {
                    if let Some(ref mut mini) = self.state.mini_app {
                        if let Some(pane) = mini.panes.get_mut(&mini.focused_pane) {
                            // Markdown/raw-text panes scroll the per-pane offset
                            // the renderer actually reads; list panes move the
                            // selection. Keyed on the PANE's mode, not the global
                            // output_mode (never set in MiniApp).
                            if matches!(
                                pane.output_mode,
                                OutputMode::Markdown | OutputMode::RawText
                            ) {
                                pane.scroll_offset = pane.scroll_offset.saturating_sub(1);
                            } else if pane.selected > 0 {
                                pane.selected -= 1;
                            }
                        }
                    }
                } else if self.state.mode == Mode::ViewOutput
                    && matches!(
                        self.state.output_mode,
                        OutputMode::Markdown | OutputMode::RawText
                    )
                {
                    self.state.scroll_offset = self.state.scroll_offset.saturating_sub(1);
                } else if self.state.mode == Mode::ViewOutput {
                    if self.state.output_selected > 0 {
                        self.state.output_selected -= 1;
                    }
                } else {
                    // Move to previous selectable row in unified list.
                    let current = self.state.unified_selected;
                    if let Some(prev) = self.state.unified_rows[..current]
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, r)| r.is_selectable())
                        .map(|(i, _)| i)
                    {
                        self.state.unified_selected = prev;
                        crate::widgets::sync_preview_index(&mut self.state);
                    }
                }
            }

            Action::MoveDown => {
                // Widget focused: j goes back to command list.
                // But overlays (palette, copy menu, picker) take priority.
                if self.state.widget_focused
                    && self.state.action_palette.is_none()
                    && self.state.copy_menu.is_none()
                    && self.state.theme_picker.is_none()
                {
                    self.state.widget_focused = false;
                    return;
                }
                // Glance strip focused: j stays in the strip (it's the
                // bottom-most row — nothing below). Captures the key so it
                // doesn't scroll the hidden list.
                if self.state.status_focused
                    && self.state.action_palette.is_none()
                    && self.state.copy_menu.is_none()
                    && self.state.theme_picker.is_none()
                {
                    return;
                }
                // Action palette is its own list — navigate within it, not
                // the rows behind it. Must come before mode-based branches
                // since the palette renders over ViewOutput.
                if let Some(ref mut palette) = self.state.action_palette {
                    let max = palette.filtered_indices.len().saturating_sub(1);
                    if palette.selected < max {
                        palette.selected += 1;
                    }
                    return;
                }
                if let Some(ref mut pm) = self.state.plugin_manager {
                    if self.state.mode == Mode::PluginManager {
                        let max = pm.rows.len().saturating_sub(1);
                        if pm.selected < max {
                            pm.selected += 1;
                        }
                        return;
                    }
                }
                let picker_idx = if let Some(ref mut p) = self.state.theme_picker {
                    let max = crate::config::PRESET_NAMES.len() - 1;
                    if p.selected < max {
                        p.selected += 1;
                    }
                    Some(p.selected)
                } else {
                    None
                };
                if let Some(idx) = picker_idx {
                    let (id, _) = crate::config::PRESET_NAMES[idx];
                    self.theme = crate::config::ThemeConfig::resolve_preset(id);
                } else if let Some(ref mut menu) = self.state.copy_menu {
                    let max = menu.entries.len().saturating_sub(1);
                    if menu.selected < max {
                        menu.selected += 1;
                    }
                } else if self.state.mode == Mode::MiniApp {
                    if let Some(ref mut mini) = self.state.mini_app {
                        if let Some(pane) = mini.panes.get_mut(&mini.focused_pane) {
                            if matches!(
                                pane.output_mode,
                                OutputMode::Markdown | OutputMode::RawText
                            ) {
                                pane.scroll_offset += 1;
                            } else {
                                let max = pane.content.items.len().saturating_sub(1);
                                if pane.selected < max {
                                    pane.selected += 1;
                                }
                            }
                        }
                    }
                } else if self.state.mode == Mode::ViewOutput
                    && matches!(
                        self.state.output_mode,
                        OutputMode::Markdown | OutputMode::RawText
                    )
                {
                    self.state.scroll_offset += 1;
                } else if self.state.mode == Mode::ViewOutput {
                    let max =
                        crate::app_output::visible_output_count(&self.state).saturating_sub(1);
                    if self.state.output_selected < max {
                        self.state.output_selected += 1;
                    }
                } else {
                    // Move to next selectable row in unified list.
                    let current = self.state.unified_selected;
                    if let Some(next) = self
                        .state
                        .unified_rows
                        .iter()
                        .enumerate()
                        .skip(current + 1)
                        .find(|(_, r)| r.is_selectable())
                        .map(|(i, _)| i)
                    {
                        self.state.unified_selected = next;
                        crate::widgets::sync_preview_index(&mut self.state);
                    }
                }
            }

            Action::Search(c) => {
                // '/' is the trigger key but we don't add it to the query.
                if c != '/' {
                    self.state.query.push(c);
                }
                self.rebuild_unified_list();
            }

            Action::BackspaceSearch => {
                self.state.query.pop();
                self.rebuild_unified_list();
            }

            Action::WidgetCardOpen => {
                // Enter on a focused widget card: drill into full output.
                if self.state.widget_focused {
                    if let Some(&plugin_index) =
                        self.state.widget_indices.get(self.state.widget_selected)
                    {
                        self.state.widget_focused = false;
                        self.open_plugin_in_view_output(plugin_index);
                    }
                }
            }

            Action::StatusFocus => {
                if !self.state.glance_indices.is_empty() {
                    self.state.status_focused = true;
                    self.state.widget_focused = false;
                    self.state.vim_mode = VimMode::Normal;
                    if self.state.status_selected >= self.state.glance_indices.len() {
                        self.state.status_selected = 0;
                    }
                }
            }
            Action::StatusMoveLeft => {
                if self.state.status_focused {
                    if self.state.status_selected == 0 {
                        // Step out of the strip back to the list.
                        self.state.status_focused = false;
                    } else {
                        self.state.status_selected -= 1;
                    }
                }
            }
            Action::StatusMoveRight => {
                if self.state.status_focused {
                    let max = self.state.glance_indices.len().saturating_sub(1);
                    if self.state.status_selected < max {
                        self.state.status_selected += 1;
                    }
                }
            }
            Action::StatusItemOpen => {
                if self.state.status_focused {
                    if let Some(&plugin_index) =
                        self.state.glance_indices.get(self.state.status_selected)
                    {
                        self.state.status_focused = false;
                        self.open_plugin_in_view_output(plugin_index);
                    } else {
                        // Stale/empty selection — step out instead of a dead key.
                        self.state.status_focused = false;
                    }
                }
            }

            Action::Select => {
                // Widget focused: l/Right → next card.
                if self.state.widget_focused {
                    let max = self.state.widget_indices.len().saturating_sub(1);
                    if self.state.widget_selected < max {
                        self.state.widget_selected += 1;
                    }
                    return;
                }
                if self.state.mode == Mode::ViewOutput {
                    // In ViewOutput: 0 actions → URL fallback, 1 → run it,
                    // 2+ → open the action palette.
                    if let Some(item) =
                        crate::app_output::selected_output_item(&self.state).cloned()
                    {
                        match item.actions.len() {
                            0 => {
                                if let Some(ref url) = item.url {
                                    open_url(url);
                                }
                            }
                            1 => {
                                self.execute_item_action(&item.actions[0]);
                            }
                            _ => {
                                self.handle_action(Action::PaletteOpen);
                            }
                        }
                    }
                } else {
                    // In Unified mode, act on the selected command row.
                    let row = self
                        .state
                        .unified_rows
                        .get(self.state.unified_selected)
                        .cloned();
                    if let Some(UnifiedRow::Command { plugin_index, .. }) = row {
                        self.open_plugin_in_view_output(plugin_index);
                    }
                }
            }

            Action::Back => {
                // Widget focused: h/Back moves to previous card.
                if self.state.widget_focused {
                    if self.state.widget_selected > 0 {
                        self.state.widget_selected -= 1;
                    }
                    return;
                }
                // Clear ephemeral overlays.
                self.state.copy_menu = None;
                self.state.form_state = None;
                self.state.action_palette = None;

                // Two-step Esc for output search: first clear query, then go back.
                if !self.state.output_query.is_empty() {
                    self.state.output_query.clear();
                    self.state.output_searching = false;
                    crate::app_output::rebuild_output_filter(&mut self.state);
                    return;
                }
                crate::app_output::reset_output_search(&mut self.state);
                self.invalidate_viewing_execution();

                if let Some(entry) = self.state.navigation_history.pop() {
                    // Restore previous ViewOutput state from history.
                    self.state.viewing_plugin_index = Some(entry.plugin_index);
                    self.state.plugin_output = entry.plugin_output;
                    self.state.plugin_error = entry.plugin_error;
                    self.state.output_selected = entry.output_selected;
                    self.state.output_mode = entry.output_mode;
                    self.state.scroll_offset = entry.scroll_offset;
                    self.state.mode = Mode::ViewOutput;
                } else {
                    // Empty history: return to Unified in Normal mode.
                    // Query is preserved so the filtered list stays visible,
                    // but j/k navigate rather than type until the user re-enters Insert.
                    self.state.mode = Mode::Unified;
                    self.state.vim_mode = VimMode::Normal;
                    self.state.plugin_output = None;
                    self.state.plugin_error = None;
                    self.state.output_selected = 0;
                    self.state.output_mode = OutputMode::List;
                    self.state.viewing_plugin_index = None;
                    self.state.scroll_offset = 0;
                }
            }

            Action::Execute => {
                // Mini app mode: execute action on focused pane's selected item.
                if self.state.mode == Mode::MiniApp {
                    let item = self.state.mini_app.as_ref().and_then(|mini| {
                        let pane = mini.panes.get(&mini.focused_pane)?;
                        pane.content.items.get(pane.selected).cloned()
                    });
                    if let Some(item) = item {
                        match item.actions.len() {
                            0 => {
                                if let Some(ref url) = item.url {
                                    open_url(url);
                                }
                            }
                            1 => self.execute_item_action(&item.actions[0]),
                            _ => self.handle_action(Action::PaletteOpen),
                        }
                    }
                } else if let Some(item) =
                    crate::app_output::selected_output_item(&self.state).cloned()
                {
                    match item.actions.len() {
                        0 => {
                            if let Some(ref url) = item.url {
                                open_url(url);
                            }
                        }
                        1 => {
                            self.execute_item_action(&item.actions[0]);
                        }
                        _ => {
                            self.handle_action(Action::PaletteOpen);
                        }
                    }
                }
            }

            Action::LaunchPlugin(name) => {
                if let Some(plugin_index) = self.state.plugins.iter().position(|p| p.name == name) {
                    self.open_plugin_in_view_output(plugin_index);
                } else {
                    tracing::warn!(plugin_name = %name, "LaunchPlugin: plugin not found");
                }
            }

            Action::ScrollHalfPageDown => {
                if self.state.mode == Mode::ViewOutput
                    && matches!(
                        self.state.output_mode,
                        OutputMode::Markdown | OutputMode::RawText
                    )
                {
                    self.state.scroll_offset += 10;
                } else if self.state.mode == Mode::ViewOutput {
                    let max =
                        crate::app_output::visible_output_count(&self.state).saturating_sub(1);
                    self.state.output_selected = (self.state.output_selected + 10).min(max);
                } else {
                    // Advance unified_selected by up to 10 selectable rows.
                    let current = self.state.unified_selected;
                    let selectable: Vec<usize> = self
                        .state
                        .unified_rows
                        .iter()
                        .enumerate()
                        .filter(|(_, r)| r.is_selectable())
                        .map(|(i, _)| i)
                        .collect();
                    if let Some(pos) = selectable.iter().position(|&i| i >= current) {
                        let next_pos = (pos + 10).min(selectable.len().saturating_sub(1));
                        if let Some(&next_row) = selectable.get(next_pos) {
                            self.state.unified_selected = next_row;
                            crate::widgets::sync_preview_index(&mut self.state);
                        }
                    }
                }
            }

            Action::ScrollHalfPageUp => {
                if self.state.mode == Mode::ViewOutput
                    && matches!(
                        self.state.output_mode,
                        OutputMode::Markdown | OutputMode::RawText
                    )
                {
                    self.state.scroll_offset = self.state.scroll_offset.saturating_sub(10);
                } else if self.state.mode == Mode::ViewOutput {
                    self.state.output_selected = self.state.output_selected.saturating_sub(10);
                } else {
                    // Move unified_selected back by up to 10 selectable rows.
                    let current = self.state.unified_selected;
                    let selectable: Vec<usize> = self
                        .state
                        .unified_rows
                        .iter()
                        .enumerate()
                        .filter(|(_, r)| r.is_selectable())
                        .map(|(i, _)| i)
                        .collect();
                    if let Some(pos) = selectable.iter().position(|&i| i >= current) {
                        let prev_pos = pos.saturating_sub(10);
                        if let Some(&prev_row) = selectable.get(prev_pos) {
                            self.state.unified_selected = prev_row;
                            crate::widgets::sync_preview_index(&mut self.state);
                        }
                    }
                }
            }

            Action::ToggleOutputMode => {
                let has_columns = self
                    .state
                    .plugin_output
                    .as_ref()
                    .is_some_and(|o| !o.columns.is_empty());
                let has_markdown = self.state.plugin_output.as_ref().is_some_and(|o| {
                    o.output_format.as_deref() == Some("markdown") && o.raw_text.is_some()
                });
                self.state.output_mode = match self.state.output_mode {
                    OutputMode::List => OutputMode::RawText,
                    OutputMode::RawText if has_columns => OutputMode::Table,
                    OutputMode::RawText | OutputMode::Table if has_markdown => OutputMode::Markdown,
                    OutputMode::RawText | OutputMode::Table | OutputMode::Markdown => {
                        OutputMode::List
                    }
                };
                self.state.scroll_offset = 0;
                self.state.markdown_cache = None;
            }

            Action::ToggleDescriptions => {
                self.state.show_descriptions = !self.state.show_descriptions;
            }

            Action::PaletteOpen => {
                if self.state.mode == Mode::ViewOutput {
                    if let Some(item) =
                        crate::app_output::selected_output_item(&self.state).cloned()
                    {
                        let mut actions = item.actions.clone();
                        // Add built-in actions.
                        actions.push(ItemAction {
                            id: Some("_copy_label".to_string()),
                            label: "Copy Label".to_string(),
                            kind: ActionKind::Clipboard,
                            args: vec![item.copy_text.as_ref().unwrap_or(&item.label).clone()],
                            confirm: false,
                        });
                        actions.push(ItemAction {
                            id: Some("_copy_json".to_string()),
                            label: "Copy as JSON".to_string(),
                            kind: ActionKind::Clipboard,
                            args: vec![serde_json::to_string(&item).unwrap_or_default()],
                            confirm: false,
                        });
                        if let Some(ref url) = item.url {
                            actions.push(ItemAction {
                                id: Some("_open_url".to_string()),
                                label: "Open URL".to_string(),
                                kind: ActionKind::Open,
                                args: vec![url.clone()],
                                confirm: false,
                            });
                        }
                        let filtered_indices = (0..actions.len()).collect();
                        self.state.action_palette = Some(ActionPaletteState {
                            actions,
                            selected: 0,
                            query: String::new(),
                            filtered_indices,
                        });
                    }
                }
            }

            Action::PaletteSelect => {
                if let Some(palette) = self.state.action_palette.take() {
                    if let Some(&real_idx) = palette.filtered_indices.get(palette.selected) {
                        if let Some(action) = palette.actions.get(real_idx) {
                            self.execute_item_action(action);
                        }
                    }
                }
            }

            Action::PaletteDismiss => {
                self.state.action_palette = None;
            }

            Action::PaletteSearch(c) => {
                if let Some(ref mut palette) = self.state.action_palette {
                    palette.query.push(c);
                    palette.rebuild_filter();
                }
            }

            Action::PaletteBackspace => {
                if let Some(ref mut palette) = self.state.action_palette {
                    palette.query.pop();
                    palette.rebuild_filter();
                }
            }

            Action::Confirm => {
                if let Some(pending) = self.state.pending_confirmation.take() {
                    self.dispatch_shell_action(pending.command, pending.args);
                }
            }

            Action::Cancel => {
                self.state.pending_confirmation = None;
            }

            Action::CopyLabel => {
                if self.state.mode == Mode::ViewOutput {
                    let text = crate::app_output::selected_output_item(&self.state)
                        .map(|item| item.copy_text.as_ref().unwrap_or(&item.label).clone());
                    if let Some(text) = text {
                        copy_and_flash(&text, &mut self.state);
                    }
                }
            }

            Action::CopyMenu => {
                if self.state.mode == Mode::ViewOutput {
                    if let Some(item) =
                        crate::app_output::selected_output_item(&self.state).cloned()
                    {
                        let item = &item;
                        let mut entries = vec![("Label".to_string(), item.label.clone())];
                        if let Some(ref detail) = item.detail {
                            entries.push(("Detail".to_string(), detail.clone()));
                        }
                        if let Some(ref url) = item.url {
                            entries.push(("URL".to_string(), url.clone()));
                        }
                        if let Some(ref copy_text) = item.copy_text {
                            if copy_text != &item.label {
                                entries.push(("Copy Text".to_string(), copy_text.clone()));
                            }
                        }
                        let mut meta_keys: Vec<&String> = item.metadata.keys().collect();
                        meta_keys.sort();
                        for key in meta_keys {
                            if let Some(val) = item.metadata.get(key) {
                                entries.push((key.clone(), val.clone()));
                            }
                        }
                        entries.push((
                            "JSON".to_string(),
                            serde_json::to_string(item).unwrap_or_default(),
                        ));
                        self.state.copy_menu = Some(CopyMenuState {
                            entries,
                            selected: 0,
                        });
                    }
                }
            }

            Action::CopyMenuSelect => {
                if let Some(menu) = self.state.copy_menu.take() {
                    if let Some((_, value)) = menu.entries.get(menu.selected) {
                        copy_and_flash(value, &mut self.state);
                    }
                }
            }

            Action::CopyMenuDismiss => {
                self.state.copy_menu = None;
            }

            Action::OutputEnterSearch => {
                if self.state.mode == Mode::ViewOutput {
                    self.state.output_searching = true;
                }
            }

            Action::OutputSearch(c) => {
                self.state.output_query.push(c);
                crate::app_output::rebuild_output_filter(&mut self.state);
            }

            Action::OutputBackspaceSearch => {
                if self.state.output_query.pop().is_none() {
                    // Empty query + backspace → exit search mode.
                    self.state.output_searching = false;
                }
                crate::app_output::rebuild_output_filter(&mut self.state);
            }

            Action::OutputExitSearch => {
                // First Esc: exit search mode, keep filter visible for j/k navigation.
                self.state.output_searching = false;
            }

            Action::OpenUrl => {
                if self.state.mode == Mode::ViewOutput {
                    // Error items set `help_url` to point at troubleshooting docs; prefer it
                    // over `url` so `o` opens help when on a structured error row.
                    let url = crate::app_output::selected_output_item(&self.state)
                        .and_then(|item| item.help_url.clone().or_else(|| item.url.clone()));
                    if let Some(url) = url {
                        open_url(&url);
                        self.state.status_message =
                            Some((format!("Opened: {url}"), std::time::Instant::now()));
                    } else {
                        self.state.status_message =
                            Some(("No URL on this item".to_string(), std::time::Instant::now()));
                    }
                }
            }

            Action::FormNextField => crate::form_actions::next_field(self),
            Action::FormPrevField => crate::form_actions::prev_field(self),
            Action::FormInput(c) => crate::form_actions::input(self, c),
            Action::FormBackspace => crate::form_actions::backspace(self),
            Action::FormCursorLeft => crate::form_actions::cursor_left(self),
            Action::FormCursorRight => crate::form_actions::cursor_right(self),
            Action::FormSelectNext => crate::form_actions::select_next(self),
            Action::FormSelectPrev => crate::form_actions::select_prev(self),
            Action::FormToggle => crate::form_actions::toggle(self),
            Action::FormSubmit => crate::form_actions::submit(self),
            Action::FormCancel => crate::form_actions::cancel(self),

            Action::EnterInsertMode => {
                self.state.vim_mode = VimMode::Insert;
            }

            Action::EnterNormalMode => {
                self.state.command_input.clear();
                // Esc also steps out of the glance strip.
                self.state.status_focused = false;
                if self.state.vim_mode == VimMode::Normal
                    && self.state.mode == Mode::Unified
                    && !self.state.query.is_empty()
                {
                    // Second Esc in Normal mode: clear search.
                    self.state.query.clear();
                    self.rebuild_unified_list();
                } else {
                    // First Esc: just enter Normal mode, keep query.
                    self.state.vim_mode = VimMode::Normal;
                }
            }

            Action::EnterCommandMode => {
                self.state.vim_mode = VimMode::Command;
                self.state.command_input.clear();
            }

            Action::CommandChar(c) => {
                self.state.command_input.push(c);
            }

            Action::CommandBackspace => {
                self.state.command_input.pop();
            }

            Action::CommandComplete => {
                // Tab-complete the verb against the registry. On a unique
                // match expand to the full name; append a trailing space
                // only for commands that take arguments so the user can
                // type them immediately (no-arg commands are ready to
                // submit). On a partial common prefix, just extend.
                if let Some(completed) = crate::command::complete(&self.state.command_input) {
                    let takes_args = crate::command::COMMANDS
                        .iter()
                        .find(|c| c.name == completed)
                        .is_some_and(|c| c.arg_hint.is_some());
                    self.state.command_input = if takes_args {
                        format!("{completed} ")
                    } else {
                        completed
                    };
                }
            }

            Action::CommandSubmit => {
                let cmd = self.state.command_input.trim().to_string();
                self.state.vim_mode = VimMode::Normal;
                self.state.command_input.clear();
                // Split off the verb so `:layout phone` etc work; verbs
                // without args keep their original branches.
                let (verb, rest) = cmd
                    .split_once(' ')
                    .map_or((cmd.as_str(), ""), |(v, r)| (v, r.trim()));
                match verb {
                    "q" | "quit" => self.state.should_quit = true,
                    "r" | "refresh" => {
                        // Re-use the RefreshPlugins logic by recursing.
                        self.handle_action(Action::RefreshPlugins);
                    }
                    "layout" => {
                        // `:layout auto` clears the override; any other
                        // name sets a fixed profile. Unknown names flash
                        // the available options to the status bar.
                        if rest.is_empty() || rest.eq_ignore_ascii_case("auto") {
                            self.state.layout_profile_override = None;
                            self.state.status_message =
                                Some(("layout: auto".to_string(), std::time::Instant::now()));
                        } else if let Some(p) = crate::tui::profile::LayoutProfile::parse(rest) {
                            self.state.layout_profile_override = Some(p);
                            self.state.status_message =
                                Some((format!("layout: {}", p.label()), std::time::Instant::now()));
                        } else {
                            self.state.status_message = Some((
                                "layout: phone|narrow|medium|wide|auto".to_string(),
                                std::time::Instant::now(),
                            ));
                        }
                    }
                    _ => {
                        // Unknown command — ignore silently for now.
                    }
                }
            }

            Action::PendingG => {
                self.state.pending_g = true;
            }

            Action::GoToFirst => match self.state.mode {
                Mode::PluginManager => {
                    if let Some(ref mut pm) = self.state.plugin_manager {
                        pm.selected = 0;
                    }
                }
                Mode::Unified => {
                    if let Some(first) = self
                        .state
                        .unified_rows
                        .iter()
                        .position(UnifiedRow::is_selectable)
                    {
                        self.state.unified_selected = first;
                        crate::widgets::sync_preview_index(&mut self.state);
                    }
                }
                Mode::ViewOutput => {
                    if matches!(
                        self.state.output_mode,
                        OutputMode::Markdown | OutputMode::RawText
                    ) {
                        self.state.scroll_offset = 0;
                    } else {
                        self.state.output_selected = 0;
                    }
                }
                Mode::MiniApp => {
                    if let Some(ref mut mini) = self.state.mini_app {
                        if let Some(pane) = mini.panes.get_mut(&mini.focused_pane) {
                            if matches!(
                                pane.output_mode,
                                OutputMode::Markdown | OutputMode::RawText
                            ) {
                                pane.scroll_offset = 0;
                            } else {
                                pane.selected = 0;
                            }
                        }
                    }
                }
            },

            Action::GoToLast => {
                match self.state.mode {
                    Mode::PluginManager => {
                        if let Some(ref mut pm) = self.state.plugin_manager {
                            pm.selected = pm.rows.len().saturating_sub(1);
                        }
                    }
                    Mode::Unified => {
                        if let Some(last) = self
                            .state
                            .unified_rows
                            .iter()
                            .enumerate()
                            .rev()
                            .find(|(_, r)| r.is_selectable())
                            .map(|(i, _)| i)
                        {
                            self.state.unified_selected = last;
                            crate::widgets::sync_preview_index(&mut self.state);
                        }
                    }
                    Mode::ViewOutput => {
                        if matches!(
                            self.state.output_mode,
                            OutputMode::Markdown | OutputMode::RawText
                        ) {
                            // Clamp to the last content line. A huge sentinel
                            // offset truncates at the u16 render cast and
                            // ratatui's Paragraph does NOT clamp scroll — the
                            // pane blanks irrecoverably.
                            self.state.scroll_offset = self.viewed_scroll_limit();
                        } else {
                            let max = crate::app_output::visible_output_count(&self.state)
                                .saturating_sub(1);
                            self.state.output_selected = max;
                        }
                    }
                    Mode::MiniApp => {
                        if let Some(ref mut mini) = self.state.mini_app {
                            if let Some(pane) = mini.panes.get_mut(&mini.focused_pane) {
                                if matches!(
                                    pane.output_mode,
                                    OutputMode::Markdown | OutputMode::RawText
                                ) {
                                    // Same clamp as ViewOutput above.
                                    pane.scroll_offset = pane
                                        .content
                                        .raw_text
                                        .as_deref()
                                        .map_or(0, |raw| raw.lines().count())
                                        .saturating_sub(1);
                                } else {
                                    pane.selected = pane.content.items.len().saturating_sub(1);
                                }
                            }
                        }
                    }
                }
            }

            Action::ToggleSidebar => {
                self.state.sidebar_hidden = !self.state.sidebar_hidden;
            }

            Action::PowerMenuOpen => {
                let categories = crate::power_menu::build_power_menu_categories(&self.state);
                self.state.power_menu = Some(PowerMenuState { categories });
            }

            Action::PowerMenuDismiss => {
                self.state.power_menu = None;
            }

            Action::RerunCommand => {
                // If the focused item carries a `retry_action`, dispatch that — preserves
                // chain context that `engine.execute(plugin_index)` would lose. Falls
                // through to the standard rerun when no retry_action is present.
                let retry = crate::app_output::selected_output_item(&self.state)
                    .and_then(|item| item.retry_action.clone());
                if let Some(retry) = retry {
                    self.execute_item_action(&retry);
                } else if let Some(plugin_index) = self.state.viewing_plugin_index {
                    // Clear cache so execution always runs fresh.
                    self.invalidate_plugin_cache(plugin_index);
                    self.state.plugin_output = None;
                    self.state.plugin_error = None;
                    self.state.is_loading = true;
                    self.state.loading_started = Some(std::time::Instant::now());
                    self.state.scroll_offset = 0;
                    self.dispatch_plugin(plugin_index);
                }
            }

            Action::CycleSort => {
                self.state.sort_mode = self.state.sort_mode.next();
                self.rebuild_unified_list();
            }

            Action::OpenSettings => {
                if let Some(plugin_index) = self.state.viewing_plugin_index {
                    if let Some(meta) = self.state.plugins.get(plugin_index) {
                        if meta.settings_spec.is_empty() {
                            return;
                        }
                        // Load current store values to pre-fill the form.
                        let store_path = crate::plugin::store::store_path_for(
                            &meta.name,
                            meta.plugin_group.as_deref(),
                        );
                        let store = crate::plugin::store::PluginStore::load(store_path);

                        let fields: Vec<FormFieldState> = meta
                            .settings_spec
                            .iter()
                            .map(|spec| {
                                // Prefer saved store value, then default_value.
                                let stored = store
                                    .get(&spec.id)
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string);
                                let default = stored
                                    .or_else(|| spec.default_value.clone())
                                    .unwrap_or_default();
                                let selected_option =
                                    if let crate::plugin::traits::FieldType::Select {
                                        ref options,
                                    } = spec.field_type
                                    {
                                        options.iter().position(|o| o == &default).unwrap_or(0)
                                    } else {
                                        0
                                    };
                                let toggled = default == "true";
                                let cursor = default.len();
                                FormFieldState {
                                    spec: spec.clone(),
                                    value: default,
                                    cursor,
                                    selected_option,
                                    toggled,
                                }
                            })
                            .collect();

                        self.state.form_state = Some(FormState {
                            fields,
                            focused: 0,
                            plugin_index,
                            submit_label: "Save Settings".to_string(),
                            is_settings: true,
                        });
                        self.state.power_menu = None;
                    }
                }
            }

            Action::ThemePickerOpen => {
                let presets = crate::config::PRESET_NAMES;
                let current = self.current_preset.as_deref().unwrap_or("default");
                let selected = presets
                    .iter()
                    .position(|(id, _)| *id == current)
                    .unwrap_or(0);
                self.state.theme_picker = Some(ThemePickerState {
                    selected,
                    original_theme: self.theme.clone(),
                    original_preset: self.current_preset.clone(),
                });
                self.state.power_menu = None;
            }

            Action::ThemePickerClose { confirmed } => {
                if let Some(picker) = self.state.theme_picker.take() {
                    if confirmed {
                        let (id, _) = crate::config::PRESET_NAMES[picker.selected];
                        self.current_preset = Some(id.to_string());
                        if let Err(e) = crate::config::save_theme_preset(id) {
                            tracing::warn!(error = %e, "failed to save theme preset");
                        }
                    } else {
                        self.theme = picker.original_theme;
                        self.current_preset = picker.original_preset;
                    }
                }
            }

            Action::PluginManagerOpen => crate::plugin_manager_actions::open(self),
            Action::PluginManagerClose => crate::plugin_manager_actions::close(self),
            Action::PluginManagerToggle => crate::plugin_manager_actions::toggle(self),
            Action::PluginManagerExpand => crate::plugin_manager_actions::expand(self),
            Action::PluginManagerSetSecret => crate::plugin_manager_actions::set_secret(self),
            Action::PluginManagerDeleteSecret => crate::plugin_manager_actions::delete_secret(self),

            Action::WidgetFocusUp => crate::widget_actions::widget_focus_up(self),
            Action::WidgetDisable => crate::widget_actions::widget_disable(self),
            Action::WidgetMoveLeft => crate::widget_actions::widget_move_left(self),
            Action::WidgetMoveRight => crate::widget_actions::widget_move_right(self),
            Action::WidgetToggleVisibility => crate::widget_actions::widget_toggle_visibility(self),
            Action::WidgetPickerOpen => crate::widget_actions::widget_picker_open(self),
            Action::WidgetPickerClose => crate::widget_actions::widget_picker_close(self),
            Action::WidgetPickerUp => crate::widget_actions::widget_picker_up(self),
            Action::WidgetPickerDown => crate::widget_actions::widget_picker_down(self),
            Action::WidgetPickerToggle => crate::widget_actions::widget_picker_toggle(self),
            Action::WidgetPickerSearch(c) => crate::widget_actions::widget_picker_search(self, c),
            Action::WidgetPickerBackspace => crate::widget_actions::widget_picker_backspace(self),

            Action::RunUpgrade => {
                if let Some(ref hint) = self.state.update_hint {
                    let (cmd, args) = match self.state.install_method {
                        crate::update::InstallMethod::Cargo => {
                            ("cargo", vec!["install".to_string(), "larkline".to_string()])
                        }
                        crate::update::InstallMethod::Homebrew
                        | crate::update::InstallMethod::Unknown => {
                            ("brew", vec!["upgrade".to_string(), "larkline".to_string()])
                        }
                    };
                    self.state.pending_confirmation = Some(PendingConfirmation {
                        description: format!("Upgrade to v{hint}?"),
                        command: cmd.to_string(),
                        args,
                    });
                }
            }

            // ----- Mini app actions -----
            Action::MiniAppFocusNext => crate::mini_app::focus_next(&mut self.state),
            Action::MiniAppFocusPrev => crate::mini_app::focus_prev(&mut self.state),
            Action::MiniAppClose => {
                self.invalidate_viewing_execution();
                crate::mini_app::close(&mut self.state);
            }
            Action::MiniAppExpand => crate::mini_app::expand(&mut self.state),
            Action::MiniAppSplitH => crate::mini_app::split_h(&mut self.state),
            Action::MiniAppSplitV => crate::mini_app::split_v(&mut self.state),
            Action::MiniAppClosePane => crate::mini_app::close_focused_pane(&mut self.state),
            Action::MiniAppResizeGrow => crate::mini_app::resize_grow(&mut self.state),
            Action::MiniAppResizeShrink => crate::mini_app::resize_shrink(&mut self.state),

            Action::RefreshPlugins => self.dispatch_plugin_refresh(),

            Action::RunFocusedItemAt(idx) => {
                // Power menu fires this when the user picks a "This item"
                // entry. Look up the focused output item and run its
                // action at the given index. Power menu is auto-dismissed
                // by the surrounding handler (see line ~1124).
                if self.state.mode == Mode::ViewOutput {
                    let action_clone = crate::app_output::selected_output_item(&self.state)
                        .and_then(|item| item.actions.get(idx).cloned());
                    if let Some(action) = action_clone {
                        self.execute_item_action(&action);
                    }
                }
            }
        }
    }

    fn execute_item_action(&mut self, action: &ItemAction) {
        match action.kind {
            ActionKind::Open => {
                if let Some(url) = action.args.first() {
                    open_url(url);
                    self.state.status_message =
                        Some(("Opened in browser".to_string(), std::time::Instant::now()));
                }
            }
            ActionKind::Clipboard => {
                if let Some(text) = action.args.first() {
                    if let Err(e) = copy_to_clipboard(text) {
                        tracing::warn!(error = %e, "clipboard copy failed");
                    } else {
                        self.state.status_message =
                            Some(("Copied to clipboard".to_string(), std::time::Instant::now()));
                    }
                }
            }
            ActionKind::Shell => {
                let cmd = action.args.first().cloned().unwrap_or_default();
                let args: Vec<String> = action.args.iter().skip(1).cloned().collect();
                let description = action.label.clone();

                if action.confirm {
                    // Show Y/N confirmation before running.
                    self.state.pending_confirmation = Some(PendingConfirmation {
                        description,
                        command: cmd,
                        args,
                    });
                } else {
                    // Execute immediately without confirmation.
                    self.dispatch_shell_action(cmd, args);
                }
            }
            ActionKind::Chain => {
                // Chain: call the plugin's on_action callback.
                // args[0] = callback_id, args[1..] = context (joined with space).
                let callback_id = action.args.first().cloned().unwrap_or_default();
                let context = action
                    .args
                    .iter()
                    .skip(1)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
                if let Some(plugin_index) = self.state.viewing_plugin_index {
                    self.state.is_loading = true;
                    self.dispatch_plugin_action(plugin_index, callback_id, context);
                }
            }
            ActionKind::UpdatePane => {
                // UpdatePane: call on_action to update a specific pane (mini app mode).
                // args[0] = pane_id, args[1] = callback_id, args[2..] = context.
                let _pane_id = action.args.first().cloned().unwrap_or_default();
                let callback_id = action.args.get(1).cloned().unwrap_or_default();
                let context = action
                    .args
                    .iter()
                    .skip(2)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
                if let Some(plugin_index) = self.state.viewing_plugin_index {
                    self.state.is_loading = true;
                    self.dispatch_plugin_action(plugin_index, callback_id, context);
                }
            }
            ActionKind::NvimEdit => {
                // args[0] = file path, args[1] = split kind (optional, default "edit").
                let Some(path) = action.args.first() else {
                    return;
                };
                let split = action.args.get(1).map_or("edit", String::as_str);
                self.dispatch_nvim_open(path.clone(), split.to_string());
            }
        }
    }

    /// Open a plugin's cached output in `ViewOutput` mode, or execute it if not cached.
    /// Populate the markdown cache if we're viewing markdown and it's not already cached.
    fn refresh_markdown_cache(&mut self) {
        if self.state.mode == Mode::ViewOutput
            && self.state.output_mode == OutputMode::Markdown
            && self.state.markdown_cache.is_none()
        {
            if let Some(ref raw) = self
                .state
                .plugin_output
                .as_ref()
                .and_then(|o| o.raw_text.clone())
            {
                self.state.markdown_cache =
                    Some(crate::tui::markdown::markdown_to_text(raw, &self.theme));
            }
        }
    }

    /// Last valid scroll offset for the viewed output: the rendered markdown
    /// line count when the cache is populated (accurate — rendering changes
    /// the line count), else the raw text's line count.
    fn viewed_scroll_limit(&self) -> usize {
        let rendered = if self.state.output_mode == OutputMode::Markdown {
            self.state.markdown_cache.as_ref().map(|t| t.lines.len())
        } else {
            None
        };
        rendered
            .or_else(|| {
                self.state
                    .plugin_output
                    .as_ref()
                    .and_then(|o| o.raw_text.as_ref())
                    .map(|raw| raw.lines().count())
            })
            .unwrap_or(0)
            .saturating_sub(1)
    }

    /// Sync the content-addressed ANSI text cache with the raw buffers the
    /// next frame will render, so raw output is parsed once per output
    /// change instead of once per frame. Stale entries are evicted by the
    /// sync itself — no invalidation call sites needed.
    fn refresh_ansi_cache(&mut self) {
        let mut cache = std::mem::take(&mut self.state.ansi_cache);
        cache.sync(&crate::tui::ansi_cache::live_raw_buffers(&self.state));
        self.state.ansi_cache = cache;
    }

    fn open_plugin_in_view_output(&mut self, plugin_index: usize) {
        self.state.markdown_cache = None;
        if self.state.viewing_plugin_index != Some(plugin_index) {
            self.invalidate_viewing_execution();
        }
        // Push current ViewOutput state onto history if already viewing a plugin.
        if self.state.mode == Mode::ViewOutput {
            if let Some(current_idx) = self.state.viewing_plugin_index {
                let entry = NavigationEntry {
                    plugin_index: current_idx,
                    plugin_output: self.state.plugin_output.clone(),
                    plugin_error: self.state.plugin_error.clone(),
                    output_selected: self.state.output_selected,
                    output_mode: self.state.output_mode.clone(),
                    scroll_offset: self.state.scroll_offset,
                };
                self.state.navigation_history.push(entry);
                if self.state.navigation_history.len() > MAX_NAV_HISTORY {
                    self.state.navigation_history.remove(0);
                }
            }
        }

        crate::app_output::reset_output_search(&mut self.state);
        self.state.viewing_plugin_index = Some(plugin_index);
        let cache_enabled = self.state.plugins.get(plugin_index).is_none_or(|p| p.cache);
        let CacheTransition::Open(open) = self.apply_cache_event(CacheEvent::Open {
            plugin_index,
            cache_enabled,
        }) else {
            return;
        };
        match open {
            CacheOpen::ShowAndRevalidate(output) => {
                // Stale-while-revalidate: show cached output immediately, refresh in background.
                self.state.output_mode = crate::app_output::output_mode_for(&output);
                self.state.plugin_output = Some(output);
                self.state.plugin_error = None;
                self.state.is_loading = false;
                self.state.output_selected = 0;
                self.state.scroll_offset = 0;
                self.state.mode = Mode::ViewOutput;
                self.dispatch_plugin(plugin_index);
            }
            CacheOpen::ShowRevalidating(output) => {
                // Already revalidating — show stale data, don't trigger another execution.
                self.state.output_mode = crate::app_output::output_mode_for(&output);
                self.state.plugin_output = Some(output);
                self.state.plugin_error = None;
                self.state.is_loading = false;
                self.state.output_selected = 0;
                self.state.scroll_offset = 0;
                self.state.mode = Mode::ViewOutput;
            }
            CacheOpen::Loading => {
                self.state.plugin_output = None;
                self.state.plugin_error = None;
                self.state.is_loading = true;
                self.state.mode = Mode::ViewOutput;
            }
            CacheOpen::Error(e) => {
                self.state.plugin_output = None;
                self.state.plugin_error = Some(e);
                self.state.is_loading = false;
                self.state.mode = Mode::ViewOutput;
            }
            CacheOpen::Miss => {
                self.state.is_loading = true;
                self.state.plugin_output = None;
                self.state.plugin_error = None;
                self.state.mode = Mode::ViewOutput;
                self.dispatch_plugin(plugin_index);
            }
        }
        crate::app_output::rebuild_output_filter(&mut self.state);
        crate::app_output::check_form_init(&mut self.state, plugin_index);
        self.state.vim_mode = VimMode::Normal;
    }

    /// Rebuild the unified launcher list from plugin metadata.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn rebuild_unified_list(&mut self) {
        use nucleo_matcher::pattern::AtomKind;
        let query = self.state.query.clone();

        let n = self.state.plugins.len();

        // Compute the "group key" for each plugin: plugin_group if set, else the plugin name.
        // This key determines how plugins are bucketed into display groups.
        let group_keys: Vec<String> = (0..n)
            .map(|i| {
                self.state.plugins[i]
                    .plugin_group
                    .as_deref()
                    .unwrap_or(&self.state.plugins[i].name)
                    .to_string()
            })
            .collect();

        // Build ordered plugin indices: favorites first (config order), then alphabetically.
        let favorites = self.state.favorites.clone();
        let mut ordered: Vec<usize> = Vec::new();
        let mut fav_set: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for fav_name in &favorites {
            for (i, key) in group_keys.iter().enumerate() {
                if key == fav_name && !fav_set.contains(&i) {
                    ordered.push(i);
                    fav_set.insert(i);
                }
            }
        }
        let mut rest: Vec<usize> = (0..n).filter(|i| !fav_set.contains(i)).collect();
        match self.state.sort_mode {
            SortMode::Alpha => {
                rest.sort_unstable_by(|&a, &b| group_keys[a].cmp(&group_keys[b]));
            }
            SortMode::Recent => {
                let ts_map = crate::history::timestamps_by_group();
                rest.sort_unstable_by(|&a, &b| {
                    let ta = ts_map.get(&group_keys[a]).copied().unwrap_or(0);
                    let tb = ts_map.get(&group_keys[b]).copied().unwrap_or(0);
                    // Most recent first; ties broken alphabetically.
                    tb.cmp(&ta).then_with(|| group_keys[a].cmp(&group_keys[b]))
                });
            }
        }
        ordered.extend(rest);

        let rows = if query.is_empty() {
            // ── Flat display (empty query) ─────────────────────────────────────────
            ordered
                .iter()
                .map(|&pidx| {
                    let meta = &self.state.plugins[pidx];
                    UnifiedRow::Command {
                        plugin_index: pidx,
                        name: meta.name.clone(),
                        description: meta.description.clone(),
                        icon: meta.icon.clone(),
                        quickkey: meta.quickkey.clone(),
                        group_name: meta.plugin_group.clone(),
                        match_positions: vec![],
                    }
                })
                .collect()
        } else {
            // ── Global search (non-empty query) ───────────────────────────────────
            // Score each command's "name description" haystack; sort descending; emit flat.
            let pattern = Pattern::new(
                &query,
                CaseMatching::Ignore,
                Normalization::Smart,
                AtomKind::Fuzzy,
            );
            let mut matcher = Matcher::new(NucleoConfig::DEFAULT);
            let mut indices_buf: Vec<u32> = Vec::new();
            let mut scored: Vec<(usize, u32, Vec<usize>)> = Vec::new();

            for &pidx in &ordered {
                let meta = &self.state.plugins[pidx];
                // Exact quickkey match → pin to top regardless of fuzzy score.
                if meta
                    .quickkey
                    .as_deref()
                    .is_some_and(|qk| qk.eq_ignore_ascii_case(&query))
                {
                    scored.push((pidx, u32::MAX, vec![]));
                    continue;
                }
                let group = meta.plugin_group.as_deref().unwrap_or(&meta.name);
                let search_text = format!("{} {} {}", meta.name, group, meta.description);
                let mut chars: Vec<char> = search_text.chars().collect();
                let haystack = Utf32Str::new(&search_text, &mut chars);
                indices_buf.clear();
                if let Some(score) = pattern.indices(haystack, &mut matcher, &mut indices_buf) {
                    let name_len = meta.name.chars().count();
                    let match_positions: Vec<usize> = indices_buf
                        .iter()
                        .map(|&i| i as usize)
                        .filter(|&i| i < name_len)
                        .collect();
                    scored.push((pidx, score, match_positions));
                }
            }
            scored.sort_unstable_by_key(|row| std::cmp::Reverse(row.1));

            scored
                .into_iter()
                .map(|(pidx, _, match_positions)| {
                    let meta = &self.state.plugins[pidx];
                    // group_name badge: show the group key so the user knows which plugin this is.
                    let group_name = Some(
                        meta.plugin_group
                            .as_deref()
                            .unwrap_or(&meta.name)
                            .to_string(),
                    );
                    UnifiedRow::Command {
                        plugin_index: pidx,
                        name: meta.name.clone(),
                        description: meta.description.clone(),
                        icon: meta.icon.clone(),
                        quickkey: meta.quickkey.clone(),
                        group_name,
                        match_positions,
                    }
                })
                .collect()
        };

        // Preserve selection by plugin_index — row positions shift when the Recent
        // section is added/removed, so index-based tracking loses the cursor.
        let old_plugin_index = self
            .state
            .unified_rows
            .get(self.state.unified_selected)
            .map(|r| match r {
                UnifiedRow::Command { plugin_index, .. } => *plugin_index,
            });

        // How many selectable rows were before the old selection (position fallback).
        let old_selectable_pos = self
            .state
            .unified_rows
            .iter()
            .take(self.state.unified_selected)
            .filter(|r| r.is_selectable())
            .count();

        self.state.unified_rows = rows;

        let selectable_count = self
            .state
            .unified_rows
            .iter()
            .filter(|r| r.is_selectable())
            .count();
        if selectable_count == 0 {
            self.state.unified_selected = 0;
            return;
        }

        // During search, always start at the top result — no cursor preservation.
        if !query.is_empty() {
            self.state.unified_selected = 0;
            crate::widgets::sync_preview_index(&mut self.state);
            return;
        }

        // Restore by plugin_index: prefer the occurrence outside the Recent section
        // (i.e., the last match), so the cursor stays in the plugin's natural position.
        if let Some(pidx) = old_plugin_index {
            let match_pos = self
                .state
                .unified_rows
                .iter()
                .enumerate()
                .filter_map(|(i, r)| match r {
                    UnifiedRow::Command { plugin_index, .. } if *plugin_index == pidx => Some(i),
                    UnifiedRow::Command { .. } => None,
                })
                .next_back(); // last occurrence = canonical position (not Recent duplicate)
            if let Some(pos) = match_pos {
                self.state.unified_selected = pos;
                crate::widgets::sync_preview_index(&mut self.state);
                return;
            }
        }

        // Fallback: preserve relative selectable position.
        let target = old_selectable_pos.min(selectable_count.saturating_sub(1));
        self.state.unified_selected = self
            .state
            .unified_rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.is_selectable())
            .nth(target)
            .map_or(0, |(i, _)| i);
        crate::widgets::sync_preview_index(&mut self.state);
    }
}

// Action side-effect helpers (open_url, copy_to_clipboard, nvim_open_file)
// live in `crate::actions::side_effects` so the `lark action` CLI dispatcher
// can reuse them without depending on TUI types.
use crate::actions::side_effects::{NvimOpenError, copy_to_clipboard, nvim_open_file, open_url};

impl App {
    /// Run a shell action off the event-loop task; the result arrives as a
    /// [`BgCommandEvent::ShellDone`]. Uses explicit args (no shell
    /// interpolation) for safety. Outside a Tokio runtime (sync tests / CLI
    /// paths) the command runs inline like the old synchronous path.
    fn dispatch_shell_action(&mut self, cmd: String, args: Vec<String>) {
        tracing::info!(command = %cmd, args = ?args, "executing shell action");
        let dispatched_view = self.state.viewing_plugin_index;
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            let result = std::process::Command::new(&cmd).args(&args).output();
            self.apply_shell_result(&cmd, dispatched_view, result);
            return;
        };
        // Immediate feedback while the child runs.
        self.state.status_message = Some((format!("Running {cmd}…"), std::time::Instant::now()));
        let tx = self.bg_tx.clone();
        handle.spawn_blocking(move || {
            let result = std::process::Command::new(&cmd).args(&args).output();
            let _ = tx.blocking_send(BgCommandEvent::ShellDone {
                command: cmd,
                dispatched_view,
                result,
            });
        });
    }

    /// Apply a finished shell action to app state: raw-text pane when the
    /// user is still where they dispatched it, flash message otherwise.
    fn apply_shell_result(
        &mut self,
        command: &str,
        dispatched_view: Option<usize>,
        result: std::io::Result<std::process::Output>,
    ) {
        let navigated_away = self.state.viewing_plugin_index != dispatched_view;
        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = if stderr.is_empty() {
                    stdout.into_owned()
                } else {
                    format!("{stdout}{stderr}")
                };
                let trimmed = combined.trim();
                // Flash instead of replacing the output pane when there's
                // nothing meaningful to show (empty / empty JSON, typical
                // API responses) — or when the user has navigated away and
                // a pane replace would steal their current view.
                if trimmed.is_empty()
                    || trimmed == "[]"
                    || trimmed == "{}"
                    || trimmed.starts_with('[')
                    || navigated_away
                {
                    self.state.status_message = Some((
                        format!("{command} done (exit {})", output.status),
                        std::time::Instant::now(),
                    ));
                } else {
                    self.state.plugin_output = Some(PluginOutput {
                        title: format!("{command} (exit {})", output.status),
                        raw_text: Some(combined),
                        ..Default::default()
                    });
                    self.state.output_mode = OutputMode::RawText;
                }
            }
            Err(e) => {
                if navigated_away {
                    self.state.status_message = Some((
                        format!("shell command failed: {e}"),
                        std::time::Instant::now(),
                    ));
                } else {
                    self.state.plugin_error = Some(format!("shell command failed: {e}"));
                }
            }
        }
    }

    /// Rescan plugin dirs and rebuild the engine, gathering the blocking
    /// inputs (fs scan, config, keychain `security` subprocesses) off the
    /// event-loop task; the rebuild applies when [`BgCommandEvent::RefreshScanDone`]
    /// arrives. Outside a Tokio runtime (sync tests) the whole refresh runs
    /// inline like the old synchronous path.
    fn dispatch_plugin_refresh(&mut self) {
        if self.refresh_in_flight {
            self.state.status_message = Some((
                "Plugin refresh already running…".to_string(),
                std::time::Instant::now(),
            ));
            return;
        }
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            let result = scan_for_refresh(&self.plugin_dirs, &self.icon_set);
            self.apply_plugin_refresh(result);
            return;
        };
        self.refresh_in_flight = true;
        self.state.status_message =
            Some(("Refreshing plugins…".to_string(), std::time::Instant::now()));
        let dirs = self.plugin_dirs.clone();
        let icon_set = self.icon_set.clone();
        let tx = self.bg_tx.clone();
        handle.spawn_blocking(move || {
            let result = scan_for_refresh(&dirs, &icon_set).map(Box::new);
            let _ = tx.blocking_send(BgCommandEvent::RefreshScanDone { result });
        });
    }

    /// Rebuild the engine + app state from gathered refresh inputs.
    fn apply_plugin_refresh(&mut self, result: Result<RefreshScan, String>) {
        self.refresh_in_flight = false;
        match result {
            Ok(scan) => {
                self.pm_config = scan.pm_config;
                self.secrets = scan.secrets;
                let plugins: Vec<Arc<dyn Plugin>> = scan
                    .discovered
                    .into_iter()
                    .map(crate::plugin::build_plugin)
                    .collect();
                let metadata: Vec<PluginMetadata> =
                    plugins.iter().map(|p| p.metadata().clone()).collect();
                let plugin_count = plugins.len();
                let (tx, rx) = mpsc::channel(plugin_count.max(1) * 3);
                self.registry_generation = self.registry_generation.wrapping_add(1);
                self.latest_plugin_executions.clear();
                self.latest_action_executions.clear();
                // Old-generation finishes are rejected by the generation
                // check and would never free their guards — drop them.
                self.prefetch_in_flight.clear();
                self.engine =
                    PluginEngine::new(plugins, tx, self.secrets.clone(), self.registry_generation);
                self.rx = rx;
                self.keybindings = self.keybindings_config.resolve(&metadata);
                self.state.plugins = metadata;
                self.state.mode = Mode::Unified;
                self.state.output_mode = OutputMode::List;
                self.state.plugin_output = None;
                self.state.plugin_error = None;
                self.state.is_loading = false;
                self.state.loading_started = None;
                self.clear_plugin_cache();
                self.state.viewing_plugin_index = None;
                self.state.navigation_history.clear();
                // The plugin list was replaced, so widget/status indices (and
                // their per-index refresh timestamps) point into the OLD list —
                // rebuild them or a stale index panics the render/refresh.
                self.state.widget_last_refresh.clear();
                self.state.status_last_refresh.clear();
                self.dispatch_all_prefetch();
                self.rebuild_unified_list();
                crate::widgets::rebuild_widget_indices(&mut self.state, &self.pm_config);
                crate::widgets::rebuild_status_indices(&mut self.state, &self.pm_config);
                crate::widgets::rebuild_glance_indices(&mut self.state);
            }
            Err(e) => {
                self.state.warnings = vec![format!("Refresh failed: {e}")];
            }
        }
    }

    /// Open a file in the parent nvim off the event-loop task; the result
    /// arrives as a [`BgCommandEvent::NvimDone`].
    fn dispatch_nvim_open(&mut self, path: String, split: String) {
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            let result = nvim_open_file(&path, &split);
            self.apply_nvim_result(&path, result);
            return;
        };
        let tx = self.bg_tx.clone();
        handle.spawn_blocking(move || {
            let result = nvim_open_file(&path, &split);
            let _ = tx.blocking_send(BgCommandEvent::NvimDone { path, result });
        });
    }

    /// Apply a finished nvim remote-send to app state.
    fn apply_nvim_result(&mut self, path: &str, result: Result<(), NvimOpenError>) {
        match result {
            Ok(()) => {
                self.state.status_message =
                    Some((format!("Opened in nvim: {path}"), std::time::Instant::now()));
            }
            Err(NvimOpenError::NotUnderNvim) => {
                // No $NVIM — fall back to open_url so plugins stay useful
                // outside Neovim sessions. (spawn(), never blocks.)
                open_url(path);
                self.state.status_message = Some((
                    "Not running under Neovim; opened via system handler".to_string(),
                    std::time::Instant::now(),
                ));
            }
            Err(NvimOpenError::CommandFailed(e)) => {
                tracing::warn!(error = %e, path = %path, "nvim remote-send failed");
                self.state.status_message =
                    Some((format!("nvim open failed: {e}"), std::time::Instant::now()));
            }
        }
    }

    /// Route one background-command completion to its applier.
    fn handle_bg_command_event(&mut self, event: BgCommandEvent) {
        match event {
            BgCommandEvent::ShellDone {
                command,
                dispatched_view,
                result,
            } => self.apply_shell_result(&command, dispatched_view, result),
            BgCommandEvent::NvimDone { path, result } => self.apply_nvim_result(&path, result),
            BgCommandEvent::RefreshScanDone { result } => {
                self.apply_plugin_refresh(result.map(|scan| *scan));
            }
            BgCommandEvent::PmSnapshotReady { snapshot } => {
                self.pm_snapshot = Some(*snapshot);
                // Rebuild the open manager's rows with real keychain data,
                // preserving cursor + expansion.
                if self.state.mode == Mode::PluginManager {
                    crate::plugin_manager_actions::rebuild_rows(self);
                }
            }
        }
    }

    /// Gather the plugin-manager snapshot off the event-loop task; rows
    /// rebuild when [`BgCommandEvent::PmSnapshotReady`] arrives. Outside a
    /// Tokio runtime the snapshot is gathered inline.
    pub(crate) fn dispatch_pm_snapshot(&mut self) {
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            self.pm_snapshot = Some(crate::plugin_manager_state::scan_snapshot(
                &self.plugin_dirs,
                &self.state.plugins,
            ));
            return;
        };
        let dirs = self.plugin_dirs.clone();
        let active = self.state.plugins.clone();
        let tx = self.bg_tx.clone();
        handle.spawn_blocking(move || {
            let snapshot = crate::plugin_manager_state::scan_snapshot(&dirs, &active);
            let _ = tx.blocking_send(BgCommandEvent::PmSnapshotReady {
                snapshot: Box::new(snapshot),
            });
        });
    }
}

/// Copy a string to the system clipboard and show a flash message on the status bar.
fn copy_and_flash(text: &str, state: &mut AppState) {
    match copy_to_clipboard(text) {
        Ok(()) => {
            let preview = if text.chars().count() > 40 {
                format!("{}…", text.chars().take(40).collect::<String>())
            } else {
                text.to_string()
            };
            state.status_message = Some((format!("Copied: {preview}"), std::time::Instant::now()));
        }
        Err(e) => {
            state.status_message =
                Some((format!("Clipboard error: {e}"), std::time::Instant::now()));
        }
    }
}

// ---------------------------------------------------------------------------
// Stub data (test only — replaced by PluginRegistry + ScriptPlugin in production)
// ---------------------------------------------------------------------------

#[cfg(test)]
fn stub_plugins() -> Vec<Arc<dyn Plugin>> {
    use std::time::Duration;

    use crate::plugin::traits::{PluginError, PluginOutput};

    macro_rules! stub {
        ($name:expr, $desc:expr, $icon:expr, $cat:expr) => {{
            struct StubPlugin(PluginMetadata);
            #[async_trait::async_trait]
            impl Plugin for StubPlugin {
                fn metadata(&self) -> &PluginMetadata {
                    &self.0
                }
                async fn execute(&self) -> Result<PluginOutput, PluginError> {
                    Ok(PluginOutput {
                        title: self.0.name.clone(),
                        ..Default::default()
                    })
                }
            }
            Arc::new(StubPlugin(PluginMetadata {
                name: $name.to_string(),
                description: $desc.to_string(),
                version: "0.1.0".to_string(),
                author: "taylor".to_string(),
                icon: $icon.to_string(),
                icon_nerd: None,
                category: Some($cat.to_string()),
                keybinding: None,
                timeout: Duration::from_secs(10),
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
                status: false,
                status_refresh_secs: 0,
                mini_app: false,
                agent_callable: false,
                destructive: false,
            })) as Arc<dyn Plugin>
        }};
    }

    vec![
        stub!(
            "GitHub PRs",
            "Check open pull requests across your repos",
            "🔀",
            "dev"
        ),
        stub!(
            "System Info",
            "CPU, memory, and disk usage at a glance",
            "💻",
            "system"
        ),
        stub!(
            "Home Assistant",
            "Toggle lights and switches via REST API",
            "🏠",
            "home"
        ),
        stub!(
            "Claude Usage",
            "Monitor Claude Code API token consumption",
            "📊",
            "dev"
        ),
        stub!(
            "RSS Feed",
            "Quick-check curated RSS feed highlights",
            "📰",
            "reading"
        ),
        stub!(
            "Shell Snippets",
            "Run saved shell commands with confirmation",
            "⚡",
            "system"
        ),
        stub!("Weather", "Current conditions and forecast", "🌤", "info"),
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract command names in order from `unified_rows` (Command rows only).
    fn command_names(app: &App) -> Vec<&str> {
        app.state
            .unified_rows
            .iter()
            .map(|r| match r {
                UnifiedRow::Command { name, .. } => name.as_str(),
            })
            .collect()
    }

    fn frame_timing(total_ms: u64, input_wait_ms: u64) -> FrameTiming {
        FrameTiming {
            total: std::time::Duration::from_millis(total_ms),
            draw: std::time::Duration::ZERO,
            input_wait: std::time::Duration::from_millis(input_wait_ms),
            input_drain: std::time::Duration::ZERO,
            engine_drain: std::time::Duration::ZERO,
            input_events: 0,
            engine_events: 0,
        }
    }

    #[test]
    fn frame_timing_is_slow_at_sixteen_milliseconds() {
        assert!(frame_timing(16, 0).is_slow());
    }

    #[test]
    fn frame_timing_is_not_slow_below_sixteen_milliseconds() {
        assert!(!frame_timing(15, 0).is_slow());
    }

    #[test]
    fn frame_timing_excludes_blocking_input_wait() {
        let timing = frame_timing(17, 16);

        assert_eq!(timing.active(), std::time::Duration::from_millis(1));
    }

    #[test]
    fn frame_timing_saturates_when_input_wait_exceeds_total() {
        let timing = frame_timing(5, 10);

        assert_eq!(timing.active(), std::time::Duration::ZERO);
    }

    #[test]
    fn emitted_timing_first_paint_has_exact_info_contract() {
        let event = crate::test_tracing::capture_event(|| {
            log_first_paint(
                std::time::Duration::from_millis(123),
                std::time::Duration::from_millis(4),
                7,
            );
        });

        assert_eq!(
            event,
            crate::test_tracing::CapturedEvent::new(
                tracing::Level::INFO,
                [
                    ("message", "first paint"),
                    ("startup_ms", "123"),
                    ("draw_ms", "4"),
                    ("plugin_count", "7"),
                ],
            )
        );
    }

    #[test]
    fn emitted_timing_slow_frame_has_exact_warn_contract() {
        let timing = FrameTiming {
            total: std::time::Duration::from_millis(20),
            draw: std::time::Duration::from_millis(2),
            input_wait: std::time::Duration::from_millis(3),
            input_drain: std::time::Duration::from_millis(4),
            engine_drain: std::time::Duration::from_millis(5),
            input_events: 6,
            engine_events: 7,
        };

        let event = crate::test_tracing::capture_event(|| timing.log());

        assert_eq!(
            event,
            crate::test_tracing::CapturedEvent::new(
                tracing::Level::WARN,
                [
                    ("message", "slow tui frame"),
                    ("frame_ms", "17"),
                    ("draw_ms", "2"),
                    ("input_wait_ms", "3"),
                    ("input_drain_ms", "4"),
                    ("engine_drain_ms", "5"),
                    ("input_events", "6"),
                    ("engine_events", "7"),
                ],
            )
        );
    }

    #[test]
    fn emitted_timing_normal_frame_has_exact_trace_contract() {
        let timing = FrameTiming {
            total: std::time::Duration::from_millis(20),
            draw: std::time::Duration::from_millis(1),
            input_wait: std::time::Duration::from_millis(5),
            input_drain: std::time::Duration::from_millis(2),
            engine_drain: std::time::Duration::from_millis(3),
            input_events: 4,
            engine_events: 5,
        };

        let event = crate::test_tracing::capture_event(|| timing.log());

        assert_eq!(
            event,
            crate::test_tracing::CapturedEvent::new(
                tracing::Level::TRACE,
                [
                    ("message", "tui frame"),
                    ("frame_ms", "15"),
                    ("draw_ms", "1"),
                    ("input_wait_ms", "5"),
                    ("input_drain_ms", "2"),
                    ("engine_drain_ms", "3"),
                    ("input_events", "4"),
                    ("engine_events", "5"),
                ],
            )
        );
    }

    #[test]
    fn empty_query_shows_all_commands() {
        let app = App::with_stubs();
        // All stubs are standalone (no plugin_group) → one Command row each.
        let names = command_names(&app);
        assert_eq!(names.len(), app.state.plugins.len());
    }

    #[test]
    fn favorites_sort_to_top_with_empty_query() {
        // "Weather" is alphabetically last among stubs, but favorited → should be first command.
        let app = App::with_stubs_and_favorites(vec!["Weather".to_string()]);
        let names = command_names(&app);
        assert!(!names.is_empty());
        assert_eq!(names[0], "Weather");
    }

    #[test]
    fn favorites_config_order_preserved() {
        // Multiple favorites should appear in command order: Weather, then GitHub PRs.
        let app =
            App::with_stubs_and_favorites(vec!["Weather".to_string(), "GitHub PRs".to_string()]);
        let names = command_names(&app);
        assert_eq!(names[0], "Weather");
        assert_eq!(names[1], "GitHub PRs");
    }

    #[test]
    fn non_favorite_commands_sorted_alphabetically() {
        // With no favorites, commands should appear alphabetically.
        let app = App::with_stubs();
        let names = command_names(&app);
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
    }

    #[test]
    fn default_plugin_preselects_command_row() {
        // The selected row should be the Weather command (no cache needed).
        let app = App::with_stubs_and_default("Weather");
        let sel = app.state.unified_selected;
        assert!(app.state.unified_rows[sel].is_selectable());
        assert!(
            matches!(&app.state.unified_rows[sel], UnifiedRow::Command { name, .. } if name == "Weather"),
            "expected Weather command at row {sel}"
        );
    }

    #[test]
    fn missing_default_plugin_falls_back_to_zero() {
        // A plugin name that doesn't exist → unified_selected stays at 0.
        let app = App::with_stubs_and_default("DoesNotExist");
        assert_eq!(app.state.unified_selected, 0);
    }

    #[test]
    fn search_matches_command_names() {
        let mut app = App::with_stubs();
        // "sys" should fuzzy-match "System Info".
        app.handle_action(Action::Search('s'));
        app.handle_action(Action::Search('y'));
        app.handle_action(Action::Search('s'));
        let names = command_names(&app);
        assert!(
            names.contains(&"System Info"),
            "expected 'System Info' in {names:?}"
        );
    }

    #[test]
    fn search_no_match_returns_empty_rows() {
        let mut app = App::with_stubs();
        app.handle_action(Action::Search('z'));
        app.handle_action(Action::Search('z'));
        app.handle_action(Action::Search('z'));
        assert!(command_names(&app).is_empty());
    }

    #[test]
    fn search_results_carry_group_name_badge() {
        let mut app = App::with_stubs();
        // "sys" matches "System Info"; group_name badge should be "System Info".
        app.handle_action(Action::Search('s'));
        app.handle_action(Action::Search('y'));
        app.handle_action(Action::Search('s'));
        let has_badge = app.state.unified_rows.iter().any(|r| {
            matches!(r,
                UnifiedRow::Command { name, group_name: Some(g), .. }
                if name == "System Info" && g == "System Info"
            )
        });
        assert!(
            has_badge,
            "expected group_name badge on System Info search result"
        );
    }

    #[test]
    fn search_ranks_commands_across_plugins() {
        // "git" should match "GitHub PRs"; all rows are flat Commands.
        let mut app = App::with_stubs();
        app.handle_action(Action::Search('g'));
        app.handle_action(Action::Search('i'));
        app.handle_action(Action::Search('t'));
        let names = command_names(&app);
        assert!(
            !names.is_empty(),
            "expected at least one match for 'git' in {names:?}"
        );
    }

    #[test]
    fn move_up_down_in_view_output_changes_output_selected() {
        let mut app = App::with_stubs();
        // Set up ViewOutput mode with some items.
        app.state.mode = Mode::ViewOutput;
        app.state.unified_selected = 0;
        app.state.plugin_output = Some(PluginOutput {
            title: "test".into(),
            items: vec![
                crate::plugin::traits::OutputItem {
                    label: "item 0".into(),
                    ..Default::default()
                },
                crate::plugin::traits::OutputItem {
                    label: "item 1".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
        assert_eq!(app.state.output_selected, 0);
        app.handle_action(Action::MoveDown);
        assert_eq!(app.state.output_selected, 1);
        app.handle_action(Action::MoveDown); // At max, should not go past
        assert_eq!(app.state.output_selected, 1);
        app.handle_action(Action::MoveUp);
        assert_eq!(app.state.output_selected, 0);
        app.handle_action(Action::MoveUp); // At min, should not go below 0
        assert_eq!(app.state.output_selected, 0);
    }

    #[test]
    fn execute_action_without_output_is_noop() {
        let mut app = App::with_stubs();
        app.handle_action(Action::Execute);
        // Should not panic or error.
    }

    #[test]
    fn back_clears_plugin_output() {
        let mut app = App::with_stubs();
        app.state.mode = Mode::ViewOutput;
        app.state.plugin_output = Some(PluginOutput::default());
        app.state.output_selected = 2;
        app.handle_action(Action::Back);
        assert_eq!(app.state.mode, Mode::Unified);
        assert!(app.state.plugin_output.is_none());
        assert_eq!(app.state.output_selected, 0);
    }

    #[test]
    fn loading_started_set_on_plugin_started_cleared_on_finished() {
        use crate::plugin::engine::EngineEvent;
        let mut app = App::with_stubs();
        assert!(app.state.loading_started.is_none());
        let stamp = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 1,
        };
        app.track_plugin_execution(0, ExecutionSource::UserSelected, stamp);

        app.handle_engine_event(EngineEvent::PluginStarted {
            plugin_index: 0,
            stamp,
            source: crate::plugin::engine::ExecutionSource::UserSelected,
        });
        assert!(app.state.loading_started.is_some());
        assert!(app.state.is_loading);

        app.handle_engine_event(EngineEvent::PluginFinished {
            plugin_index: 0,
            stamp,
            result: Ok(PluginOutput::default()),
            source: crate::plugin::engine::ExecutionSource::UserSelected,
        });
        assert!(app.state.loading_started.is_none());
        assert!(!app.state.is_loading);
    }

    #[test]
    fn prefetch_start_preserves_ready_output_while_revalidating() {
        let mut app = App::with_stubs();
        app.state.result_cache.insert(
            0,
            CachedResult::Ready(PluginOutput {
                title: "stale".to_string(),
                ..Default::default()
            }),
        );
        let stamp = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 20,
        };
        app.track_plugin_execution(0, ExecutionSource::Prefetch, stamp);

        app.handle_engine_event(EngineEvent::PluginStarted {
            plugin_index: 0,
            stamp,
            source: ExecutionSource::Prefetch,
        });

        assert!(matches!(
            app.state.result_cache.get(&0),
            Some(CachedResult::Revalidating(output)) if output.title == "stale"
        ));
    }

    #[tokio::test]
    async fn refresh_plugins_scans_in_background_and_applies_on_event() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugin_dir = dir.path().join("new-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("manifest.toml"),
            r#"
[plugin]
name = "New Plugin"
description = "found by background scan"
version = "0.1.0"
author = "test"
icon = "N"
entry = "run.sh"
"#,
        )
        .unwrap();
        let mut config = Config::default();
        config.general.plugin_dirs = vec![dir.path().to_path_buf()];
        let mut app = App::new(
            stub_plugins(),
            &config,
            Vec::new(),
            std::collections::HashMap::new(),
        );

        app.handle_action(Action::RefreshPlugins);
        assert_ne!(
            app.state.plugins.len(),
            1,
            "the scan must run in the background, not apply on the key path"
        );

        let event = tokio::time::timeout(std::time::Duration::from_secs(10), app.bg_rx.recv())
            .await
            .expect("scan completion within timeout")
            .expect("scan completion event");
        app.handle_bg_command_event(event);

        assert_eq!(app.state.plugins.len(), 1);
        assert_eq!(app.state.plugins[0].name, "New Plugin");
        assert_eq!(app.state.mode, Mode::Unified);
    }

    #[tokio::test]
    async fn second_refresh_while_scanning_is_deduped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut config = Config::default();
        config.general.plugin_dirs = vec![dir.path().to_path_buf()];
        let mut app = App::new(
            stub_plugins(),
            &config,
            Vec::new(),
            std::collections::HashMap::new(),
        );

        app.handle_action(Action::RefreshPlugins);
        app.handle_action(Action::RefreshPlugins);

        // Exactly one scan completion must arrive.
        let event = tokio::time::timeout(std::time::Duration::from_secs(10), app.bg_rx.recv())
            .await
            .expect("first scan completion within timeout")
            .expect("scan completion event");
        app.handle_bg_command_event(event);
        assert!(
            app.bg_rx.try_recv().is_err(),
            "a refresh already in flight must not dispatch a second scan"
        );
    }

    #[test]
    fn pm_expand_reads_secret_source_from_snapshot_not_keychain() {
        let mut app = App::with_stubs();
        // Plugin 0 declares a secret that does NOT exist in any real
        // keychain — only the seeded snapshot claims it does. Seeing
        // SecretSource::Keychain therefore proves the expand keypress read
        // the snapshot instead of shelling out to `security`.
        app.state.plugins[0].secrets = vec!["LARK_TEST_FAKE_SECRET".to_string()];
        app.state.mode = Mode::PluginManager;
        let snapshot = crate::plugin_manager_state::PmSnapshot {
            all_meta: app.state.plugins.clone(),
            env_secrets: std::collections::HashMap::new(),
            keychain: std::iter::once(("LARK_TEST_FAKE_SECRET".to_string(), true)).collect(),
        };
        app.state.plugin_manager = Some(crate::plugin_manager_state::build(
            &snapshot,
            &app.pm_config,
        ));
        app.pm_snapshot = Some(snapshot);

        // Cursor starts on the first plugin header; expand it.
        app.handle_action(Action::PluginManagerExpand);

        let pm = app.state.plugin_manager.as_ref().expect("manager open");
        let secret_row = pm
            .rows
            .iter()
            .find_map(|row| match row {
                PluginManagerRow::Secret { key, source } if key == "LARK_TEST_FAKE_SECRET" => {
                    Some(source.clone())
                }
                _ => None,
            })
            .expect("expanded group must show its secret row");
        assert_eq!(
            secret_row,
            SecretSource::Keychain,
            "secret presence must come from the snapshot, not a live keychain lookup"
        );
    }

    #[tokio::test]
    async fn pm_snapshot_event_rebuilds_open_manager_rows() {
        let mut app = App::with_stubs();
        app.handle_action(Action::PluginManagerOpen);
        assert!(
            app.state.plugin_manager.is_some(),
            "manager opens instantly"
        );

        // The background gather completes and rebuilds the rows.
        let event = tokio::time::timeout(std::time::Duration::from_secs(10), app.bg_rx.recv())
            .await
            .expect("snapshot within timeout")
            .expect("snapshot event");
        app.handle_bg_command_event(event);

        assert!(app.pm_snapshot.is_some(), "snapshot cached for keypresses");
        assert!(app.state.plugin_manager.is_some(), "manager still open");
    }

    #[tokio::test]
    async fn confirmed_shell_action_does_not_block_the_ui_task() {
        let mut app = App::with_stubs();
        app.state.pending_confirmation = Some(PendingConfirmation {
            description: "slow child".to_string(),
            command: "/bin/sleep".to_string(),
            args: vec!["2".to_string()],
        });

        let started = std::time::Instant::now();
        app.handle_action(Action::Confirm);

        assert!(
            started.elapsed() < std::time::Duration::from_millis(500),
            "dispatch must return immediately, not wait for the child \
             (took {:?})",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn shell_completion_event_updates_output_pane() {
        let mut app = App::with_stubs();
        app.dispatch_shell_action("/bin/echo".to_string(), vec!["hello".to_string()]);

        let event = app.bg_rx.recv().await.expect("completion event");
        app.handle_bg_command_event(event);

        let output = app.state.plugin_output.as_ref().expect("output pane set");
        assert!(
            output.raw_text.as_deref().unwrap_or("").contains("hello"),
            "the child's stdout must land in the output pane"
        );
        assert_eq!(app.state.output_mode, OutputMode::RawText);
    }

    #[tokio::test]
    async fn late_shell_completion_after_navigation_does_not_steal_view() {
        let mut app = App::with_stubs();
        app.state.viewing_plugin_index = Some(0);
        app.dispatch_shell_action("/bin/echo".to_string(), vec!["hello".to_string()]);
        // The user navigates elsewhere before the child finishes.
        app.state.viewing_plugin_index = Some(1);
        app.state.plugin_output = Some(PluginOutput {
            title: "current view".to_string(),
            ..Default::default()
        });

        let event = app.bg_rx.recv().await.expect("completion event");
        app.handle_bg_command_event(event);

        assert_eq!(
            app.state.plugin_output.as_ref().map(|o| o.title.as_str()),
            Some("current view"),
            "a late completion must not replace the navigated-to view"
        );
        assert!(
            app.state.status_message.is_some(),
            "the completion degrades to a flash message"
        );
    }

    #[test]
    fn nvim_failure_result_flashes_error() {
        let mut app = App::with_stubs();

        app.apply_nvim_result(
            "/tmp/f.txt",
            Err(NvimOpenError::CommandFailed("boom".to_string())),
        );

        let (message, _) = app.state.status_message.as_ref().expect("flash message");
        assert!(message.contains("boom"), "failure must surface: {message}");
    }

    #[test]
    fn go_to_last_clamps_scroll_to_content_lines() {
        let mut app = App::with_stubs();
        app.state.mode = Mode::ViewOutput;
        app.state.output_mode = OutputMode::RawText;
        app.state.plugin_output = Some(PluginOutput {
            raw_text: Some("l1\nl2\nl3\nl4\nl5".to_string()),
            ..Default::default()
        });

        app.handle_action(Action::GoToLast);

        assert_eq!(
            app.state.scroll_offset, 4,
            "G must land on the last content line, not a huge sentinel that \
             truncates at the u16 render cast and blanks the pane"
        );
    }

    #[test]
    fn go_to_last_uses_rendered_markdown_line_count() {
        let mut app = App::with_stubs();
        app.state.mode = Mode::ViewOutput;
        app.state.output_mode = OutputMode::Markdown;
        app.state.plugin_output = Some(PluginOutput {
            raw_text: Some("# heading\nbody".to_string()),
            ..Default::default()
        });
        // Rendered markdown line count differs from the raw source's.
        app.state.markdown_cache = Some(ratatui::text::Text::from(vec![
            ratatui::text::Line::raw("heading"),
            ratatui::text::Line::raw(""),
            ratatui::text::Line::raw("body"),
        ]));

        app.handle_action(Action::GoToLast);

        assert_eq!(
            app.state.scroll_offset, 2,
            "markdown G must clamp against the rendered cache, not raw lines"
        );
    }

    #[test]
    fn go_to_last_in_mini_app_pane_clamps_scroll() {
        let mut app = App::with_stubs();
        app.state.mode = Mode::MiniApp;
        let pane = PaneState {
            content: crate::plugin::traits::PaneContent {
                raw_text: Some("a\nb\nc".to_string()),
                ..Default::default()
            },
            output_mode: OutputMode::RawText,
            ..Default::default()
        };
        app.state.mini_app = Some(MiniAppState {
            plugin_index: 0,
            layout: crate::plugin::traits::MiniAppLayout::Pane {
                id: "main".to_string(),
                content: crate::plugin::traits::PaneContent::default(),
            },
            panes: std::iter::once(("main".to_string(), pane)).collect(),
            focused_pane: "main".to_string(),
            pane_order: vec!["main".to_string()],
        });

        app.handle_action(Action::GoToLast);

        let mini = app.state.mini_app.as_ref().expect("mini app state");
        assert_eq!(
            mini.panes["main"].scroll_offset, 2,
            "mini-app G must clamp to the pane's content lines"
        );
    }

    #[test]
    fn due_widget_refresh_skips_in_flight_prefetch() {
        let mut app = App::with_stubs();
        app.state.plugins[0].widget = true;
        app.state.plugins[0].widget_refresh_secs = 1;
        app.state.widget_indices = vec![0];
        // Never refreshed → due.
        assert_eq!(app.due_widget_refreshes(std::time::Instant::now()), vec![0]);

        // Dispatch a prefetch (simulated) — while it runs, the plugin is not due.
        let stamp = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 40,
        };
        app.track_plugin_execution(0, ExecutionSource::Prefetch, stamp);
        assert!(
            app.due_widget_refreshes(std::time::Instant::now())
                .is_empty(),
            "a widget with a prefetch in flight must not be re-dispatched"
        );

        // Finish arrives → eligible again (still no timestamp → due).
        app.handle_engine_event(EngineEvent::PluginFinished {
            plugin_index: 0,
            stamp,
            result: Ok(PluginOutput::default()),
            source: ExecutionSource::Prefetch,
        });
        assert_eq!(app.due_widget_refreshes(std::time::Instant::now()), vec![0]);
    }

    #[test]
    fn due_status_refresh_skips_in_flight_prefetch() {
        let mut app = App::with_stubs();
        app.state.plugins[0].status = true;
        app.state.plugins[0].status_refresh_secs = 1;
        app.state.status_indices = vec![0];
        assert_eq!(app.due_status_refreshes(std::time::Instant::now()), vec![0]);

        let stamp = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 41,
        };
        app.track_plugin_execution(0, ExecutionSource::Prefetch, stamp);
        assert!(
            app.due_status_refreshes(std::time::Instant::now())
                .is_empty(),
            "a status chip with a prefetch in flight must not be re-dispatched"
        );
    }

    #[test]
    fn stale_prefetch_finish_does_not_clear_newer_in_flight_guard() {
        let mut app = App::with_stubs();
        app.state.plugins[0].widget = true;
        app.state.plugins[0].widget_refresh_secs = 1;
        app.state.widget_indices = vec![0];
        let older = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 42,
        };
        app.track_plugin_execution(0, ExecutionSource::Prefetch, older);
        let newer = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 43,
        };
        app.track_plugin_execution(0, ExecutionSource::Prefetch, newer);

        // The superseded execution's finish must not free the guard while
        // the newer execution is still running.
        app.handle_engine_event(EngineEvent::PluginFinished {
            plugin_index: 0,
            stamp: older,
            result: Ok(PluginOutput::default()),
            source: ExecutionSource::Prefetch,
        });
        assert!(
            app.due_widget_refreshes(std::time::Instant::now())
                .is_empty(),
            "stale finish must not mark the newer in-flight prefetch as done"
        );
    }

    #[tokio::test]
    async fn startup_prefetch_seeds_widget_and_status_refresh_timestamps() {
        let mut app = App::with_stubs();
        app.state.plugins[0].widget = true;
        app.state.plugins[0].widget_refresh_secs = 300;
        app.state.plugins[1].status = true;
        app.state.plugins[1].status_refresh_secs = 300;
        app.state.widget_indices = vec![0];
        app.state.status_indices = vec![1];

        app.dispatch_all_prefetch();

        assert!(
            app.state.widget_last_refresh.contains_key(&0),
            "startup dispatch must seed the widget refresh timestamp"
        );
        assert!(
            app.state.status_last_refresh.contains_key(&1),
            "startup dispatch must seed the status refresh timestamp"
        );
        // Non-widget/status plugins get no timestamps.
        assert!(!app.state.widget_last_refresh.contains_key(&2));
        assert!(!app.state.status_last_refresh.contains_key(&2));
        // The acceptance criterion: the first refresh scan right after
        // startup no longer re-dispatches what startup just dispatched.
        let now = std::time::Instant::now();
        assert!(app.due_widget_refreshes(now).is_empty());
        assert!(app.due_status_refreshes(now).is_empty());
    }

    #[test]
    fn unrelated_plugin_event_keeps_foreground_markdown_cache() {
        let mut app = App::with_stubs();
        app.state.viewing_plugin_index = Some(0);
        app.state.markdown_cache = Some(ratatui::text::Text::raw("rendered doc"));
        let stamp = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 30,
        };
        app.track_plugin_execution(1, ExecutionSource::Prefetch, stamp);

        app.handle_engine_event(EngineEvent::PluginStarted {
            plugin_index: 1,
            stamp,
            source: ExecutionSource::Prefetch,
        });

        assert!(
            app.state.markdown_cache.is_some(),
            "background refresh of another plugin must not invalidate the viewed markdown cache"
        );
    }

    #[test]
    fn viewed_plugin_event_invalidates_markdown_cache() {
        let mut app = App::with_stubs();
        app.state.viewing_plugin_index = Some(0);
        app.state.markdown_cache = Some(ratatui::text::Text::raw("rendered doc"));
        let stamp = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 31,
        };
        app.track_plugin_execution(0, ExecutionSource::Prefetch, stamp);

        app.handle_engine_event(EngineEvent::PluginStarted {
            plugin_index: 0,
            stamp,
            source: ExecutionSource::Prefetch,
        });

        assert!(
            app.state.markdown_cache.is_none(),
            "an event for the viewed plugin must invalidate its markdown cache"
        );
    }

    #[test]
    fn streaming_prefetch_finish_preserves_accumulated_output() {
        let mut app = App::with_stubs();
        let stamp = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 21,
        };
        app.track_plugin_execution(0, ExecutionSource::Prefetch, stamp);
        app.handle_engine_event(EngineEvent::PluginStarted {
            plugin_index: 0,
            stamp,
            source: ExecutionSource::Prefetch,
        });
        app.handle_engine_event(EngineEvent::PartialOutput {
            plugin_index: 0,
            stamp,
            title: Some("stream".to_string()),
            items: vec![crate::plugin::traits::OutputItem {
                label: "first".to_string(),
                ..Default::default()
            }],
            source: ExecutionSource::Prefetch,
        });

        app.handle_engine_event(EngineEvent::PluginFinished {
            plugin_index: 0,
            stamp,
            result: Ok(PluginOutput::default()),
            source: ExecutionSource::Prefetch,
        });

        let cached = match app.state.result_cache.get(&0) {
            Some(CachedResult::Ready(output)) => output,
            other => panic!("expected ready streaming output, got {other:?}"),
        };
        assert_eq!(
            (
                &cached.title,
                cached.items.first().map(|item| item.label.as_str())
            ),
            (&"stream".to_string(), Some("first"))
        );
    }

    #[test]
    fn streaming_revalidation_rebuilds_active_output_filter() {
        let mut app = App::with_stubs();
        let stale = PluginOutput {
            title: "stale".to_string(),
            items: vec![crate::plugin::traits::OutputItem {
                label: "keep old".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        app.state
            .result_cache
            .insert(0, CachedResult::Revalidating(stale.clone()));
        app.state.plugin_output = Some(stale);
        app.state.viewing_plugin_index = Some(0);
        app.state.output_query = "keep".to_string();
        crate::app_output::rebuild_output_filter(&mut app.state);
        let stamp = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 22,
        };
        app.track_plugin_execution(0, ExecutionSource::UserSelected, stamp);
        app.handle_engine_event(EngineEvent::PluginStarted {
            plugin_index: 0,
            stamp,
            source: ExecutionSource::UserSelected,
        });

        app.handle_engine_event(EngineEvent::PartialOutput {
            plugin_index: 0,
            stamp,
            title: Some("fresh".to_string()),
            items: vec![
                crate::plugin::traits::OutputItem {
                    label: "drop new".to_string(),
                    ..Default::default()
                },
                crate::plugin::traits::OutputItem {
                    label: "keep new".to_string(),
                    ..Default::default()
                },
            ],
            source: ExecutionSource::UserSelected,
        });

        assert_eq!(app.state.output_filtered_indices, vec![1]);
    }

    #[test]
    fn older_prefetch_finish_cannot_overwrite_newer_user_cache() {
        let mut app = App::with_stubs();
        let older = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 23,
        };
        let newer = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 24,
        };
        app.track_plugin_execution(0, ExecutionSource::Prefetch, older);
        app.handle_engine_event(EngineEvent::PluginStarted {
            plugin_index: 0,
            stamp: older,
            source: ExecutionSource::Prefetch,
        });
        app.track_plugin_execution(0, ExecutionSource::UserSelected, newer);
        app.handle_engine_event(EngineEvent::PluginStarted {
            plugin_index: 0,
            stamp: newer,
            source: ExecutionSource::UserSelected,
        });
        app.handle_engine_event(EngineEvent::PluginFinished {
            plugin_index: 0,
            stamp: newer,
            result: Ok(PluginOutput {
                title: "fresh".to_string(),
                ..Default::default()
            }),
            source: ExecutionSource::UserSelected,
        });

        app.handle_engine_event(EngineEvent::PluginFinished {
            plugin_index: 0,
            stamp: older,
            result: Ok(PluginOutput {
                title: "old".to_string(),
                ..Default::default()
            }),
            source: ExecutionSource::Prefetch,
        });

        assert!(matches!(
            app.state.result_cache.get(&0),
            Some(CachedResult::Ready(output)) if output.title == "fresh"
        ));
    }

    #[test]
    fn plugin_finished_after_back_does_not_reopen_output() {
        use crate::plugin::engine::EngineEvent;

        let mut app = App::with_stubs();
        app.state.mode = Mode::ViewOutput;
        app.state.viewing_plugin_index = Some(0);
        let stamp = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 2,
        };
        app.track_plugin_execution(0, ExecutionSource::UserSelected, stamp);
        app.handle_engine_event(EngineEvent::PluginStarted {
            plugin_index: 0,
            stamp,
            source: crate::plugin::engine::ExecutionSource::UserSelected,
        });

        app.handle_action(Action::Back);
        app.handle_engine_event(EngineEvent::PluginFinished {
            plugin_index: 0,
            stamp,
            result: Ok(PluginOutput {
                title: "stale".to_string(),
                ..Default::default()
            }),
            source: crate::plugin::engine::ExecutionSource::UserSelected,
        });

        assert_eq!(app.state.mode, Mode::Unified);
    }

    #[test]
    fn older_user_dispatch_cannot_overwrite_newer_dispatch() {
        let mut app = App::with_stubs();
        app.state.mode = Mode::ViewOutput;
        app.state.viewing_plugin_index = Some(0);
        let older = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 10,
        };
        let newer = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 11,
        };
        app.track_plugin_execution(0, ExecutionSource::UserSelected, older);
        app.track_plugin_execution(0, ExecutionSource::UserSelected, newer);

        app.handle_engine_event(EngineEvent::PluginFinished {
            plugin_index: 0,
            stamp: older,
            result: Ok(PluginOutput {
                title: "older".to_string(),
                ..Default::default()
            }),
            source: ExecutionSource::UserSelected,
        });

        assert!(app.state.plugin_output.is_none());
    }

    #[test]
    fn plugin_finished_from_previous_registry_generation_is_dropped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut config = Config::default();
        config.general.plugin_dirs = vec![dir.path().to_path_buf()];
        let mut app = App::new(
            stub_plugins(),
            &config,
            Vec::new(),
            std::collections::HashMap::new(),
        );
        app.state.mode = Mode::ViewOutput;
        app.state.viewing_plugin_index = Some(0);
        let old_stamp = ExecutionStamp {
            registry_generation: app.registry_generation,
            execution_id: 12,
        };
        app.track_plugin_execution(0, ExecutionSource::UserSelected, old_stamp);

        app.handle_action(Action::RefreshPlugins);
        app.track_plugin_execution(0, ExecutionSource::UserSelected, old_stamp);
        app.handle_engine_event(EngineEvent::PluginFinished {
            plugin_index: 0,
            stamp: old_stamp,
            result: Ok(PluginOutput {
                title: "old registry".to_string(),
                ..Default::default()
            }),
            source: ExecutionSource::UserSelected,
        });

        assert_eq!(app.state.mode, Mode::Unified);
    }

    #[test]
    fn scroll_half_page_down_and_up_in_view_output() {
        let mut app = App::with_stubs();
        app.state.mode = Mode::ViewOutput;
        let items = (0..25)
            .map(|i| crate::plugin::traits::OutputItem {
                label: format!("item {i}"),
                ..Default::default()
            })
            .collect();
        app.state.plugin_output = Some(PluginOutput {
            title: "test".into(),
            items,
            ..Default::default()
        });

        assert_eq!(app.state.output_selected, 0);
        app.handle_action(Action::ScrollHalfPageDown);
        assert_eq!(app.state.output_selected, 10);
        app.handle_action(Action::ScrollHalfPageDown);
        assert_eq!(app.state.output_selected, 20);
        app.handle_action(Action::ScrollHalfPageDown);
        assert_eq!(app.state.output_selected, 24); // clamped at max (25-1)
        app.handle_action(Action::ScrollHalfPageUp);
        assert_eq!(app.state.output_selected, 14);
        app.handle_action(Action::ScrollHalfPageUp);
        assert_eq!(app.state.output_selected, 4);
        app.handle_action(Action::ScrollHalfPageUp);
        assert_eq!(app.state.output_selected, 0); // clamped at 0
    }

    #[test]
    fn toggle_output_mode_flips_between_list_and_raw_text() {
        let mut app = App::with_stubs();
        app.state.mode = Mode::ViewOutput;
        assert_eq!(app.state.output_mode, OutputMode::List);
        app.handle_action(Action::ToggleOutputMode);
        assert_eq!(app.state.output_mode, OutputMode::RawText);
        app.handle_action(Action::ToggleOutputMode);
        assert_eq!(app.state.output_mode, OutputMode::List);
    }

    #[test]
    fn back_resets_output_mode_to_list() {
        let mut app = App::with_stubs();
        app.state.mode = Mode::ViewOutput;
        app.state.output_mode = OutputMode::RawText;
        app.handle_action(Action::Back);
        assert_eq!(app.state.output_mode, OutputMode::List);
    }

    #[test]
    fn power_menu_in_view_output_includes_focused_item_actions() {
        // Space-power-menu shows the focused item's actions as a "This
        // item" category with digit keys. Keeps the per-item shortcuts
        // discoverable without needing to remember `:` to open the
        // searchable palette.
        use crate::plugin::traits::{ActionKind, ItemAction, OutputItem, PluginOutput};
        let mut app = App::with_stubs();
        app.state.mode = Mode::ViewOutput;
        app.state.output_mode = OutputMode::List;
        app.state.plugin_output = Some(PluginOutput {
            title: "Test".to_string(),
            items: vec![OutputItem {
                label: "Row".to_string(),
                actions: vec![
                    ItemAction {
                        id: None,
                        label: "First action".to_string(),
                        kind: ActionKind::Open,
                        args: vec!["x".to_string()],
                        confirm: false,
                    },
                    ItemAction {
                        id: None,
                        label: "Second action".to_string(),
                        kind: ActionKind::Open,
                        args: vec!["y".to_string()],
                        confirm: false,
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        });
        app.state.output_selected = 0;

        let cats = crate::power_menu::build_power_menu_categories(&app.state);
        let first = cats.first().expect("at least one category");
        assert_eq!(first.name, "This item");
        assert_eq!(first.items.len(), 2);
        assert_eq!(first.items[0].key, '1');
        assert_eq!(first.items[0].label, "First action");
        assert_eq!(first.items[1].key, '2');
        assert!(matches!(
            first.items[0].action,
            crate::action::Action::RunFocusedItemAt(0)
        ));
        assert!(matches!(
            first.items[1].action,
            crate::action::Action::RunFocusedItemAt(1)
        ));
    }

    #[test]
    fn move_up_down_navigates_action_palette_not_underlying_list() {
        // Regression: action palette had no MoveUp/Down branch, so j/k
        // navigated the rows behind it instead of the palette itself.
        // Hit when a row carried many actions (Mail Inbox: 7 actions/row).
        use crate::plugin::traits::{ActionKind, ItemAction};
        let mut app = App::with_stubs();
        app.state.mode = Mode::ViewOutput;
        app.state.output_mode = OutputMode::List;
        let actions = vec![
            ItemAction {
                id: None,
                label: "First".to_string(),
                kind: ActionKind::Open,
                args: vec!["a".to_string()],
                confirm: false,
            },
            ItemAction {
                id: None,
                label: "Second".to_string(),
                kind: ActionKind::Open,
                args: vec!["b".to_string()],
                confirm: false,
            },
            ItemAction {
                id: None,
                label: "Third".to_string(),
                kind: ActionKind::Open,
                args: vec!["c".to_string()],
                confirm: false,
            },
        ];
        let mut palette = ActionPaletteState {
            actions,
            selected: 0,
            query: String::new(),
            filtered_indices: vec![0, 1, 2],
        };
        palette.rebuild_filter();
        app.state.action_palette = Some(palette);
        let underlying_selected = app.state.output_selected;

        app.handle_action(Action::MoveDown);
        assert_eq!(
            app.state.action_palette.as_ref().unwrap().selected,
            1,
            "MoveDown should advance palette selection",
        );
        app.handle_action(Action::MoveDown);
        assert_eq!(app.state.action_palette.as_ref().unwrap().selected, 2);
        // Clamp at the last filtered index.
        app.handle_action(Action::MoveDown);
        assert_eq!(app.state.action_palette.as_ref().unwrap().selected, 2);

        app.handle_action(Action::MoveUp);
        assert_eq!(app.state.action_palette.as_ref().unwrap().selected, 1);
        app.handle_action(Action::MoveUp);
        app.handle_action(Action::MoveUp);
        assert_eq!(
            app.state.action_palette.as_ref().unwrap().selected,
            0,
            "MoveUp should clamp at 0",
        );

        // Critically: the row selection behind the palette should NOT have moved.
        assert_eq!(
            app.state.output_selected, underlying_selected,
            "palette navigation must not leak through to ViewOutput rows",
        );
    }

    #[test]
    fn refresh_picks_up_newly_added_plugin() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut config = Config::default();
        config.general.plugin_dirs = vec![dir.path().to_path_buf()];
        let mut app = App::new(vec![], &config, vec![], std::collections::HashMap::new());
        assert_eq!(app.state.plugins.len(), 0);

        // Add a plugin manifest (entry existence not checked at scan time after Task 7).
        let plugin_dir = dir.path().join("new-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("manifest.toml"),
            r#"
[plugin]
name = "New Plugin"
description = "Added after init"
version = "0.1.0"
author = "test"
icon = "N"
entry = "run.sh"
"#,
        )
        .unwrap();

        app.handle_action(Action::RefreshPlugins);

        assert_eq!(app.state.plugins.len(), 1);
        assert_eq!(app.state.plugins[0].name, "New Plugin");
        assert_eq!(app.state.mode, Mode::Unified);
    }
}
