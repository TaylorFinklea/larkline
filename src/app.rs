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
use crate::plugin::engine::{EngineEvent, ExecutionSource, PluginEngine};
use crate::plugin::registry;
use crate::plugin::traits::{
    ActionKind, ItemAction, MiniAppLayout, PaneContent, PaneId, PluginOutput,
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
    /// Pending `g` key — waiting for second `g` to trigger `GoToFirst`.
    pub pending_g: bool,
    /// Whether the sidebar is hidden in `ViewOutput` mode.
    pub sidebar_hidden: bool,
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

/// A widget-eligible command for the widget picker overlay.
#[derive(Debug, Clone)]
pub struct WidgetPickerEntry {
    /// Display name: "Plugin: Command" or just "Command".
    pub label: String,
    /// Icon from the manifest.
    pub icon: String,
    /// Key used in `disabled_widgets` — `"GroupKey:CommandName"`.
    pub key: String,
    /// Whether this widget is currently enabled (not in the disabled list).
    pub enabled: bool,
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
pub struct App {
    pub(crate) state: AppState,
    pub(crate) theme: Theme,
    pub(crate) keybindings: ResolvedKeybindings,
    pub(crate) engine: PluginEngine,
    pub(crate) rx: mpsc::Receiver<EngineEvent>,
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
        let metadata: Vec<PluginMetadata> = plugins.iter().map(|p| p.metadata().clone()).collect();
        let engine = PluginEngine::new(plugins, tx, secrets.clone());
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
            plugin_dirs: config.general.plugin_dirs.clone(),
            keybindings_config: config.keybindings.clone(),
            icon_set: config.ui.icon_set.clone(),
            secrets,
            current_preset: config.theme.preset.clone(),
            pm_config: crate::config::load_plugin_manager_config(),
        };
        app.rebuild_unified_list();
        crate::widgets::rebuild_widget_indices(&mut app.state, &app.pm_config);

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
    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        // Kick off background prefetch for all eligible plugins.
        self.engine.execute_all();

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

        while !self.state.should_quit {
            self.refresh_markdown_cache();
            terminal.draw(|frame| ui::render(frame, &self.state, &self.theme))?;

            // Block up to 16ms waiting for input, then drain every queued event
            // before re-rendering. Draining matters on terminals that emit
            // non-Key events (resize, paste, focus) interleaved with keys —
            // otherwise each non-Key event wastes a frame and the next press
            // appears to "not register" until another key is pressed.
            if event::poll(std::time::Duration::from_millis(16))? {
                loop {
                    match event::read()? {
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

            // Drain engine events (non-blocking).
            while let Ok(event) = self.rx.try_recv() {
                self.handle_engine_event(event);
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
                let due: Vec<usize> = self
                    .state
                    .widget_indices
                    .iter()
                    .copied()
                    .filter(|pidx| {
                        let meta = &self.state.plugins[*pidx];
                        meta.widget_refresh_secs > 0
                            && now
                                .duration_since(
                                    self.state
                                        .widget_last_refresh
                                        .get(pidx)
                                        .copied()
                                        .unwrap_or(now),
                                )
                                .as_secs()
                                >= meta.widget_refresh_secs
                    })
                    .collect();
                if !due.is_empty() {
                    for pidx in &due {
                        self.state.widget_last_refresh.insert(*pidx, now);
                        self.engine.execute(*pidx);
                    }
                    self.rebuild_unified_list();
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
        }

        Ok(())
    }

    /// Process a single engine event, updating app state.
    ///
    /// Extracted from the run loop so it can be called from tests.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn handle_engine_event(&mut self, event: EngineEvent) {
        // Any engine event may deliver new output — invalidate markdown cache.
        self.state.markdown_cache = None;
        match event {
            EngineEvent::PluginStarted {
                plugin_index,
                source,
            } => match source {
                ExecutionSource::Prefetch => {
                    self.state.result_cache.insert(
                        plugin_index,
                        CachedResult::Loading(std::time::Instant::now()),
                    );
                }
                ExecutionSource::UserSelected => {
                    // Don't clear stale output during a stale-while-revalidate refresh.
                    let is_revalidating = matches!(
                        self.state.result_cache.get(&plugin_index),
                        Some(CachedResult::Revalidating(_))
                    );
                    if !is_revalidating {
                        self.state.is_loading = true;
                        self.state.loading_started = Some(std::time::Instant::now());
                        self.state.plugin_output = None;
                        self.state.plugin_error = None;
                    }
                }
            },
            EngineEvent::PartialOutput {
                plugin_index,
                title,
                items,
                source,
            } => match source {
                ExecutionSource::Prefetch => {
                    // Accumulate partials into cache (for commands with prefetch = true).
                    let entry = self
                        .state
                        .result_cache
                        .entry(plugin_index)
                        .or_insert_with(|| {
                            CachedResult::Ready(PluginOutput {
                                title: String::new(),
                                ..Default::default()
                            })
                        });
                    if let CachedResult::Ready(output) = entry {
                        if let Some(t) = title {
                            output.title = t;
                        }
                        output.items.extend(items);
                    } else {
                        let mut new_output = PluginOutput {
                            title: title.unwrap_or_default(),
                            ..Default::default()
                        };
                        new_output.items.extend(items);
                        *entry = CachedResult::Ready(new_output);
                    }
                }
                ExecutionSource::UserSelected => {
                    // Existing streaming behavior.
                    if let Some(ref t) = title {
                        self.state.plugin_output = Some(PluginOutput {
                            title: t.clone(),
                            items,
                            ..Default::default()
                        });
                        self.state.mode = Mode::ViewOutput;
                        self.state.output_selected = 0;
                        self.state.output_mode = OutputMode::List;
                    } else if let Some(ref mut output) = self.state.plugin_output {
                        output.items.extend(items);
                    }
                }
            },
            EngineEvent::PluginFinished {
                plugin_index,
                result,
                source,
            } => match source {
                ExecutionSource::Prefetch => {
                    match result {
                        Ok(output) => {
                            let entry = self
                                .state
                                .result_cache
                                .entry(plugin_index)
                                .or_insert(CachedResult::Ready(output.clone()));
                            if matches!(entry, CachedResult::Loading(_)) {
                                *entry = CachedResult::Ready(output);
                            }
                        }
                        Err(e) => {
                            self.state
                                .result_cache
                                .insert(plugin_index, CachedResult::Error(e.to_string()));
                        }
                    }
                    // Refresh widget summaries when prefetch results arrive.
                    if self
                        .state
                        .plugins
                        .get(plugin_index)
                        .is_some_and(|m| m.widget)
                    {
                        self.rebuild_unified_list();
                    }
                }
                ExecutionSource::UserSelected => {
                    let was_revalidating = matches!(
                        self.state.result_cache.get(&plugin_index),
                        Some(CachedResult::Revalidating(_))
                    );
                    let cache_enabled =
                        self.state.plugins.get(plugin_index).is_none_or(|p| p.cache);

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

                            if cache_enabled {
                                self.state
                                    .result_cache
                                    .insert(plugin_index, CachedResult::Ready(output.clone()));
                            } else {
                                self.state.result_cache.remove(&plugin_index);
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
                            if was_revalidating {
                                // Keep showing stale data; silently update cache to Error.
                                self.state
                                    .result_cache
                                    .insert(plugin_index, CachedResult::Error(e.to_string()));
                            } else {
                                self.state
                                    .result_cache
                                    .insert(plugin_index, CachedResult::Error(e.to_string()));
                                self.state.plugin_error = Some(e.to_string());
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
                            self.state.mini_app =
                                Some(crate::mini_app::build_mini_app_state(plugin_index, layout));
                            self.state.mode = Mode::MiniApp;
                        } else {
                            if self.state.mode != Mode::ViewOutput {
                                self.state.mode = Mode::ViewOutput;
                            }
                            self.state.output_selected = 0;
                            // Auto-select Table mode when columns are defined.
                            self.state.output_mode = if self
                                .state
                                .plugin_output
                                .as_ref()
                                .is_some_and(|o| !o.columns.is_empty())
                            {
                                OutputMode::Table
                            } else {
                                OutputMode::List
                            };
                        }
                    }
                }
            },

            EngineEvent::ActionResult {
                plugin_index,
                result,
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
                            self.state.plugin_output = Some(output);
                            self.state.output_mode = if self
                                .state
                                .plugin_output
                                .as_ref()
                                .is_some_and(|o| !o.columns.is_empty())
                            {
                                OutputMode::Table
                            } else {
                                OutputMode::List
                            };
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
                            if pane.selected > 0 {
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
                            let max = pane.content.items.len().saturating_sub(1);
                            if pane.selected < max {
                                pane.selected += 1;
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
                    run_shell_action(&mut self.state, &pending.command, &pending.args);
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
                    let url = crate::app_output::selected_output_item(&self.state)
                        .and_then(|item| item.url.clone());
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

            Action::CommandSubmit => {
                let cmd = self.state.command_input.trim().to_string();
                self.state.vim_mode = VimMode::Normal;
                self.state.command_input.clear();
                match cmd.as_str() {
                    "q" | "quit" => self.state.should_quit = true,
                    "r" | "refresh" => {
                        // Re-use the RefreshPlugins logic by recursing.
                        self.handle_action(Action::RefreshPlugins);
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
                Mode::ViewOutput | Mode::MiniApp => {
                    if matches!(
                        self.state.output_mode,
                        OutputMode::Markdown | OutputMode::RawText
                    ) {
                        self.state.scroll_offset = 0;
                    } else {
                        self.state.output_selected = 0;
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
                    Mode::ViewOutput | Mode::MiniApp => {
                        if matches!(
                            self.state.output_mode,
                            OutputMode::Markdown | OutputMode::RawText
                        ) {
                            // Set a large scroll offset; Paragraph rendering clamps naturally.
                            self.state.scroll_offset = usize::MAX / 2;
                        } else {
                            let max = crate::app_output::visible_output_count(&self.state)
                                .saturating_sub(1);
                            self.state.output_selected = max;
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
                if let Some(plugin_index) = self.state.viewing_plugin_index {
                    // Clear cache so execution always runs fresh.
                    self.state.result_cache.remove(&plugin_index);
                    self.state.plugin_output = None;
                    self.state.plugin_error = None;
                    self.state.is_loading = true;
                    self.state.loading_started = Some(std::time::Instant::now());
                    self.state.scroll_offset = 0;
                    self.engine.execute(plugin_index);
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
            Action::MiniAppClose => crate::mini_app::close(&mut self.state),
            Action::MiniAppExpand => crate::mini_app::expand(&mut self.state),
            Action::MiniAppSplitH => crate::mini_app::split_h(&mut self.state),
            Action::MiniAppSplitV => crate::mini_app::split_v(&mut self.state),
            Action::MiniAppClosePane => crate::mini_app::close_focused_pane(&mut self.state),
            Action::MiniAppResizeGrow => crate::mini_app::resize_grow(&mut self.state),
            Action::MiniAppResizeShrink => crate::mini_app::resize_shrink(&mut self.state),

            Action::RefreshPlugins => match registry::scan(&self.plugin_dirs) {
                Ok(mut discovered) => {
                    // Resolve icons based on configured icon set.
                    if self.icon_set == crate::config::IconSet::Nerd {
                        for d in &mut discovered {
                            if let Some(ref nerd) = d.metadata.icon_nerd {
                                if !nerd.is_empty() {
                                    d.metadata.icon = nerd.clone();
                                }
                            }
                        }
                    }
                    // Filter out disabled plugins/commands.
                    self.pm_config = crate::config::load_plugin_manager_config();
                    let discovered: Vec<_> = discovered
                        .into_iter()
                        .filter(|d| {
                            let gk = d
                                .metadata
                                .plugin_group
                                .as_deref()
                                .unwrap_or(&d.metadata.name);
                            if self.pm_config.is_plugin_disabled(gk) {
                                return false;
                            }
                            !self.pm_config.is_command_disabled(gk, &d.metadata.name)
                        })
                        .collect();
                    let plugins: Vec<Arc<dyn Plugin>> = discovered
                        .into_iter()
                        .map(crate::plugin::build_plugin)
                        .collect();
                    let metadata: Vec<PluginMetadata> =
                        plugins.iter().map(|p| p.metadata().clone()).collect();
                    let plugin_count = plugins.len();
                    let (tx, rx) = mpsc::channel(plugin_count.max(1) * 3);
                    // Reload secrets on refresh (with keychain fallback).
                    self.secrets = crate::config::load_secrets();
                    let declared_keys: Vec<&str> = plugins
                        .iter()
                        .flat_map(|p| p.metadata().secrets.iter().map(String::as_str))
                        .collect();
                    crate::config::resolve_keychain_secrets(&mut self.secrets, &declared_keys);
                    self.engine = PluginEngine::new(plugins, tx, self.secrets.clone());
                    self.rx = rx;
                    self.keybindings = self.keybindings_config.resolve(&metadata);
                    self.state.plugins = metadata;
                    self.state.mode = Mode::Unified;
                    self.state.output_mode = OutputMode::List;
                    self.state.plugin_output = None;
                    self.state.plugin_error = None;
                    self.state.is_loading = false;
                    self.state.loading_started = None;
                    self.state.result_cache.clear();
                    self.state.viewing_plugin_index = None;
                    self.state.navigation_history.clear();
                    self.engine.execute_all();
                    self.rebuild_unified_list();
                }
                Err(e) => {
                    self.state.warnings = vec![format!("Refresh failed: {e}")];
                }
            },
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
                    run_shell_action(&mut self.state, &cmd, &args);
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
                    self.engine
                        .execute_action(plugin_index, callback_id, context);
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
                    self.engine
                        .execute_action(plugin_index, callback_id, context);
                }
            }
            ActionKind::NvimEdit => {
                // args[0] = file path, args[1] = split kind (optional, default "edit").
                let Some(path) = action.args.first() else {
                    return;
                };
                let split = action.args.get(1).map_or("edit", String::as_str);
                match nvim_open_file(path, split) {
                    Ok(()) => {
                        self.state.status_message =
                            Some((format!("Opened in nvim: {path}"), std::time::Instant::now()));
                    }
                    Err(NvimOpenError::NotUnderNvim) => {
                        // No $NVIM — fall back to open_url so plugins stay useful
                        // outside Neovim sessions.
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

    fn open_plugin_in_view_output(&mut self, plugin_index: usize) {
        self.state.markdown_cache = None;
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
        match self.state.result_cache.get(&plugin_index).cloned() {
            Some(CachedResult::Ready(output)) if cache_enabled => {
                // Stale-while-revalidate: show cached output immediately, refresh in background.
                self.state.output_mode = crate::app_output::output_mode_for(&output);
                self.state.plugin_output = Some(output.clone());
                self.state.plugin_error = None;
                self.state.is_loading = false;
                self.state.output_selected = 0;
                self.state.scroll_offset = 0;
                self.state.mode = Mode::ViewOutput;
                self.state
                    .result_cache
                    .insert(plugin_index, CachedResult::Revalidating(output));
                self.engine.execute(plugin_index);
            }
            Some(CachedResult::Revalidating(output)) => {
                // Already revalidating — show stale data, don't trigger another execution.
                self.state.output_mode = crate::app_output::output_mode_for(&output);
                self.state.plugin_output = Some(output);
                self.state.plugin_error = None;
                self.state.is_loading = false;
                self.state.output_selected = 0;
                self.state.scroll_offset = 0;
                self.state.mode = Mode::ViewOutput;
            }
            Some(CachedResult::Loading(_)) => {
                self.state.plugin_output = None;
                self.state.plugin_error = None;
                self.state.is_loading = true;
                self.state.mode = Mode::ViewOutput;
            }
            Some(CachedResult::Error(e)) => {
                self.state.plugin_output = None;
                self.state.plugin_error = Some(e);
                self.state.is_loading = false;
                self.state.mode = Mode::ViewOutput;
            }
            // No cache, or Ready with cache disabled → execute fresh.
            _ => {
                self.state.is_loading = true;
                self.state.plugin_output = None;
                self.state.plugin_error = None;
                self.state.mode = Mode::ViewOutput;
                self.engine.execute(plugin_index);
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

// ---------------------------------------------------------------------------
// Action helpers
// ---------------------------------------------------------------------------

fn open_url(url: &str) {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(cmd).arg(url).spawn() {
        tracing::warn!(error = %e, url = url, "failed to open URL");
    }
}

/// Error opening a file in the parent Neovim instance.
pub(crate) enum NvimOpenError {
    /// `$NVIM` env var is not set — not running as a child of nvim.
    NotUnderNvim,
    /// The nvim command spawned but exited non-zero, or could not be spawned.
    CommandFailed(String),
}

impl std::fmt::Display for NvimOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotUnderNvim => f.write_str("not running under Neovim"),
            Self::CommandFailed(e) => write!(f, "nvim command failed: {e}"),
        }
    }
}

/// Open a file in the parent Neovim via `nvim --server $NVIM --remote-send`.
///
/// `split` is one of `edit`, `split`, `vsplit`, `tabedit`. Any other value
/// falls back to `edit`.
fn nvim_open_file(path: &str, split: &str) -> Result<(), NvimOpenError> {
    let Ok(socket) = std::env::var("NVIM") else {
        return Err(NvimOpenError::NotUnderNvim);
    };
    let cmd_verb = match split {
        "split" | "vsplit" | "tabedit" => split,
        _ => "edit",
    };
    // Build an ex sequence: <Esc>:<verb> <path><CR>. Escape backslashes and
    // double quotes in the path; nvim's remote-send parses the string as
    // key notation, so "<" and ">" embedded in the path would be misread.
    let escaped = path
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('<', "\\<");
    let keys = format!("<Esc>:{cmd_verb} {escaped}<CR>");

    let output = std::process::Command::new("nvim")
        .args(["--server", &socket, "--remote-send", &keys])
        .output()
        .map_err(|e| NvimOpenError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(NvimOpenError::CommandFailed(if stderr.is_empty() {
            format!("exit status {}", output.status)
        } else {
            stderr
        }));
    }
    Ok(())
}

fn copy_to_clipboard(text: &str) -> anyhow::Result<()> {
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_text(text)?;
    tracing::info!("copied to clipboard");
    Ok(())
}

/// Execute a shell command and display its output as raw text in the output pane.
///
/// Uses explicit args (no shell interpolation) for safety.
fn run_shell_action(state: &mut AppState, cmd: &str, args: &[String]) {
    tracing::info!(command = cmd, args = ?args, "executing shell action");
    match std::process::Command::new(cmd).args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = if stderr.is_empty() {
                stdout.into_owned()
            } else {
                format!("{stdout}{stderr}")
            };
            let trimmed = combined.trim();
            // If the output is empty or just a JSON empty array/object (typical API response),
            // show a flash message instead of replacing the output pane.
            if trimmed.is_empty() || trimmed == "[]" || trimmed == "{}" || trimmed.starts_with('[')
            {
                state.status_message = Some((
                    format!("{cmd} done (exit {})", output.status),
                    std::time::Instant::now(),
                ));
            } else {
                state.plugin_output = Some(PluginOutput {
                    title: format!("{cmd} (exit {})", output.status),
                    raw_text: Some(combined),
                    ..Default::default()
                });
                state.output_mode = OutputMode::RawText;
            }
        }
        Err(e) => {
            state.plugin_error = Some(format!("shell command failed: {e}"));
        }
    }
}

/// Copy a string to the system clipboard and show a flash message on the status bar.
fn copy_and_flash(text: &str, state: &mut AppState) {
    match copy_to_clipboard(text) {
        Ok(()) => {
            let preview = if text.len() > 40 {
                format!("{}…", &text[..40])
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
                mini_app: false,
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

        app.handle_engine_event(EngineEvent::PluginStarted {
            plugin_index: 0,
            source: crate::plugin::engine::ExecutionSource::UserSelected,
        });
        assert!(app.state.loading_started.is_some());
        assert!(app.state.is_loading);

        app.handle_engine_event(EngineEvent::PluginFinished {
            plugin_index: 0,
            result: Ok(PluginOutput::default()),
            source: crate::plugin::engine::ExecutionSource::UserSelected,
        });
        assert!(app.state.loading_started.is_none());
        assert!(!app.state.is_loading);
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
