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
    fn next(&self) -> Self {
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
    state: AppState,
    theme: Theme,
    keybindings: ResolvedKeybindings,
    engine: PluginEngine,
    rx: mpsc::Receiver<EngineEvent>,
    /// Plugin directories for re-scanning on refresh.
    plugin_dirs: Vec<PathBuf>,
    /// Raw keybindings config for re-resolving after refresh.
    keybindings_config: KeybindingsConfig,
    /// Icon set preference for resolving Nerd Font vs emoji icons.
    icon_set: crate::config::IconSet,
    /// Secrets loaded from `~/.config/larkline/.env`.
    secrets: std::collections::HashMap<String, String>,
    /// Currently active theme preset name (e.g. `"nord"`). `None` = default.
    current_preset: Option<String>,
    /// Plugin manager enable/disable config.
    pm_config: crate::config::PluginManagerConfig,
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
        app.rebuild_widget_indices();

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
                app.sync_preview_index();
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

            if event::poll(std::time::Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    // Only process key press events, not repeats or releases.
                    if key.kind == KeyEventKind::Press {
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
                            // Clear pending_g for any action except PendingG itself.
                            if !matches!(action, Action::PendingG) {
                                self.state.pending_g = false;
                            }
                            self.handle_action(action);
                        } else {
                            // No action produced — clear pending_g.
                            self.state.pending_g = false;
                        }
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
            if self.state.mode == Mode::Unified {
                let now = std::time::Instant::now();
                let due: Vec<usize> = self
                    .state
                    .plugins
                    .iter()
                    .enumerate()
                    .filter(|(pidx, meta)| {
                        meta.widget
                            && meta.widget_refresh_secs > 0
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
                    .map(|(pidx, _)| pidx)
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
                                    self.state.output_mode = Self::output_mode_for(&output);
                                    self.state.plugin_output = Some(output);
                                    self.rebuild_output_filter();
                                    self.check_form_init(plugin_index);
                                }
                            } else {
                                // Fresh load: don't overwrite streaming output.
                                if self.state.plugin_output.is_none() {
                                    self.state.plugin_output = Some(output);
                                    self.rebuild_output_filter();
                                    self.check_form_init(plugin_index);
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
                            self.state.mini_app = Some(
                                crate::mini_app::build_mini_app_state(plugin_index, layout),
                            );
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
                                self.state.mini_app = Some(
                                    crate::mini_app::build_mini_app_state(plugin_index, layout.clone()),
                                );
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
                            self.state.status_message = Some((
                                "Action completed".to_string(),
                                std::time::Instant::now(),
                            ));
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
                            self.state.status_message = Some((
                                "Action completed".to_string(),
                                std::time::Instant::now(),
                            ));
                        }
                    }
                    Err(e) => {
                        self.state.status_message = Some((
                            format!("Action failed: {e}"),
                            std::time::Instant::now(),
                        ));
                    }
                }
            },
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
                        self.sync_preview_index();
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
                    let max = self.visible_output_count().saturating_sub(1);
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
                        self.sync_preview_index();
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
                    if let Some(item) = self.selected_output_item().cloned() {
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
                    self.rebuild_output_filter();
                    return;
                }
                self.reset_output_search();

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
                } else if let Some(item) = self.selected_output_item().cloned() {
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
                    let max = self.visible_output_count().saturating_sub(1);
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
                            self.sync_preview_index();
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
                            self.sync_preview_index();
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
                    if let Some(item) = self.selected_output_item().cloned() {
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
                    let text = self
                        .selected_output_item()
                        .map(|item| item.copy_text.as_ref().unwrap_or(&item.label).clone());
                    if let Some(text) = text {
                        copy_and_flash(&text, &mut self.state);
                    }
                }
            }

            Action::CopyMenu => {
                if self.state.mode == Mode::ViewOutput {
                    if let Some(item) = self.selected_output_item().cloned() {
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
                self.rebuild_output_filter();
            }

            Action::OutputBackspaceSearch => {
                if self.state.output_query.pop().is_none() {
                    // Empty query + backspace → exit search mode.
                    self.state.output_searching = false;
                }
                self.rebuild_output_filter();
            }

            Action::OutputExitSearch => {
                // First Esc: exit search mode, keep filter visible for j/k navigation.
                self.state.output_searching = false;
            }

            Action::OpenUrl => {
                if self.state.mode == Mode::ViewOutput {
                    let url = self
                        .selected_output_item()
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

            Action::FormNextField => {
                if let Some(ref mut form) = self.state.form_state {
                    form.focused = (form.focused + 1) % form.fields.len();
                }
            }

            Action::FormPrevField => {
                if let Some(ref mut form) = self.state.form_state {
                    form.focused = if form.focused == 0 {
                        form.fields.len().saturating_sub(1)
                    } else {
                        form.focused - 1
                    };
                }
            }

            Action::FormInput(c) => {
                if let Some(ref mut form) = self.state.form_state {
                    if let Some(field) = form.fields.get_mut(form.focused) {
                        if matches!(
                            field.spec.field_type,
                            crate::plugin::traits::FieldType::Text
                        ) {
                            field.value.insert(field.cursor, c);
                            field.cursor += c.len_utf8();
                        }
                    }
                }
            }

            Action::FormBackspace => {
                if let Some(ref mut form) = self.state.form_state {
                    if let Some(field) = form.fields.get_mut(form.focused) {
                        if matches!(
                            field.spec.field_type,
                            crate::plugin::traits::FieldType::Text
                        ) && field.cursor > 0
                        {
                            // Find the previous char boundary.
                            let prev = field.value[..field.cursor]
                                .char_indices()
                                .next_back()
                                .map_or(0, |(i, _)| i);
                            field.value.remove(prev);
                            field.cursor = prev;
                        }
                    }
                }
            }

            Action::FormCursorLeft => {
                if let Some(ref mut form) = self.state.form_state {
                    if let Some(field) = form.fields.get_mut(form.focused) {
                        if field.cursor > 0 {
                            field.cursor = field.value[..field.cursor]
                                .char_indices()
                                .next_back()
                                .map_or(0, |(i, _)| i);
                        }
                    }
                }
            }

            Action::FormCursorRight => {
                if let Some(ref mut form) = self.state.form_state {
                    if let Some(field) = form.fields.get_mut(form.focused) {
                        if field.cursor < field.value.len() {
                            field.cursor = field.value[field.cursor..]
                                .char_indices()
                                .nth(1)
                                .map_or(field.value.len(), |(i, _)| field.cursor + i);
                        }
                    }
                }
            }

            Action::FormSelectNext => {
                if let Some(ref mut form) = self.state.form_state {
                    if let Some(field) = form.fields.get_mut(form.focused) {
                        if let crate::plugin::traits::FieldType::Select { ref options } =
                            field.spec.field_type
                        {
                            if !options.is_empty() {
                                field.selected_option = (field.selected_option + 1) % options.len();
                                field.value = options[field.selected_option].clone();
                            }
                        }
                    }
                }
            }

            Action::FormSelectPrev => {
                if let Some(ref mut form) = self.state.form_state {
                    if let Some(field) = form.fields.get_mut(form.focused) {
                        if let crate::plugin::traits::FieldType::Select { ref options } =
                            field.spec.field_type
                        {
                            if !options.is_empty() {
                                field.selected_option = if field.selected_option == 0 {
                                    options.len() - 1
                                } else {
                                    field.selected_option - 1
                                };
                                field.value = options[field.selected_option].clone();
                            }
                        }
                    }
                }
            }

            Action::FormToggle => {
                if let Some(ref mut form) = self.state.form_state {
                    if let Some(field) = form.fields.get_mut(form.focused) {
                        match field.spec.field_type {
                            crate::plugin::traits::FieldType::Toggle => {
                                field.toggled = !field.toggled;
                                field.value =
                                    if field.toggled { "true" } else { "false" }.to_string();
                            }
                            crate::plugin::traits::FieldType::Select { .. } => {
                                // Space cycles forward in Select fields too.
                                self.handle_action(Action::FormSelectNext);
                            }
                            crate::plugin::traits::FieldType::Text => {
                                // Space in a text field = insert a space character.
                                self.handle_action(Action::FormInput(' '));
                            }
                        }
                    }
                }
            }

            Action::FormSubmit => {
                if let Some(form) = self.state.form_state.take() {
                    // Validate required fields.
                    let all_valid = form
                        .fields
                        .iter()
                        .all(|f| !f.spec.required || !f.value.trim().is_empty());

                    if !all_valid {
                        self.state.status_message = Some((
                            "Required fields cannot be empty".to_string(),
                            std::time::Instant::now(),
                        ));
                        self.state.form_state = Some(form);
                    } else if form.is_settings {
                        // Settings form: persist values to plugin store, then rerun.
                        let plugin_index = form.plugin_index;
                        if let Some(meta) = self.state.plugins.get(plugin_index) {
                            let store_path = crate::plugin::store::store_path_for(
                                &meta.name,
                                meta.plugin_group.as_deref(),
                            );
                            let mut store = crate::plugin::store::PluginStore::load(store_path);
                            for field in &form.fields {
                                let value = match field.spec.field_type {
                                    crate::plugin::traits::FieldType::Toggle => {
                                        if field.toggled { "true" } else { "false" }.to_string()
                                    }
                                    crate::plugin::traits::FieldType::Select { ref options } => {
                                        options
                                            .get(field.selected_option)
                                            .cloned()
                                            .unwrap_or_default()
                                    }
                                    crate::plugin::traits::FieldType::Text => field.value.clone(),
                                };
                                let _ = store
                                    .set(field.spec.id.clone(), serde_json::Value::String(value));
                            }
                            if let Err(e) = store.save() {
                                tracing::warn!(error = %e, "failed to save plugin settings");
                            }
                        }
                        // Rerun the plugin with fresh execution.
                        self.state.result_cache.remove(&plugin_index);
                        self.state.plugin_output = None;
                        self.state.plugin_error = None;
                        self.state.is_loading = true;
                        self.state.loading_started = Some(std::time::Instant::now());
                        self.state.scroll_offset = 0;
                        self.engine.execute(plugin_index);
                    } else {
                        let mut values = std::collections::HashMap::new();
                        for field in &form.fields {
                            values.insert(field.spec.id.clone(), field.value.clone());
                        }
                        let plugin_index = form.plugin_index;
                        self.state.is_loading = true;
                        self.state.plugin_output = None;
                        self.state.loading_started = Some(std::time::Instant::now());
                        self.engine.execute_with_form(plugin_index, values);
                    }
                }
            }

            Action::FormCancel => {
                self.state.form_state = None;
                self.handle_action(Action::Back);
            }

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
                        self.sync_preview_index();
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
                            self.sync_preview_index();
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
                            let max = self.visible_output_count().saturating_sub(1);
                            self.state.output_selected = max;
                        }
                    }
                }
            }

            Action::ToggleSidebar => {
                self.state.sidebar_hidden = !self.state.sidebar_hidden;
            }

            Action::PowerMenuOpen => {
                let categories = self.build_power_menu_categories();
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

            Action::PluginManagerOpen => {
                self.state.power_menu = None;
                self.state.mode = Mode::PluginManager;
                self.state.vim_mode = VimMode::Normal;
                self.state.plugin_manager = Some(self.build_plugin_manager_state());
            }

            Action::PluginManagerClose => {
                self.state.plugin_manager = None;
                self.state.mode = Mode::Unified;
                // Trigger full refresh so disabled plugins are filtered out.
                self.handle_action(Action::RefreshPlugins);
            }

            Action::PluginManagerToggle => {
                if let Some(ref mut pm) = self.state.plugin_manager {
                    let changed = match pm.rows.get(pm.selected).cloned() {
                        Some(PluginManagerRow::PluginHeader { group_key, .. }) => {
                            self.pm_config.toggle_plugin(&group_key);
                            true
                        }
                        Some(PluginManagerRow::Command {
                            group_key, name, ..
                        }) => {
                            self.pm_config.toggle_command(&group_key, &name);
                            true
                        }
                        _ => false,
                    };
                    if changed {
                        if let Err(e) = crate::config::save_plugin_manager_config(&self.pm_config) {
                            tracing::warn!(error = %e, "failed to save plugin manager config");
                        }
                        // Rebuild rows to reflect toggle.
                        self.state.plugin_manager = Some(self.build_plugin_manager_state());
                        // Restore selection position.
                        if let Some(ref mut pm) = self.state.plugin_manager {
                            pm.selected = pm.selected.min(pm.rows.len().saturating_sub(1));
                        }
                    }
                }
            }

            Action::PluginManagerExpand => {
                if let Some(ref mut pm) = self.state.plugin_manager {
                    if let Some(PluginManagerRow::PluginHeader { group_key, .. }) =
                        pm.rows.get(pm.selected)
                    {
                        let key = group_key.clone();
                        if pm.expanded.contains(&key) {
                            pm.expanded.remove(&key);
                        } else {
                            pm.expanded.insert(key);
                        }
                        let expanded = pm.expanded.clone();
                        let sel = pm.selected;
                        let mut new_pm = self.build_plugin_manager_state_with_expanded(&expanded);
                        new_pm.selected = sel.min(new_pm.rows.len().saturating_sub(1));
                        new_pm.expanded = expanded;
                        self.state.plugin_manager = Some(new_pm);
                    }
                }
            }

            Action::PluginManagerSetSecret => {
                if let Some(ref pm) = self.state.plugin_manager {
                    if let Some(PluginManagerRow::Secret { key, .. }) = pm.rows.get(pm.selected) {
                        self.state.status_message = Some((
                            format!("Run: lark secret set {key}"),
                            std::time::Instant::now(),
                        ));
                    }
                }
            }

            Action::PluginManagerDeleteSecret => {
                if let Some(ref pm) = self.state.plugin_manager {
                    if let Some(PluginManagerRow::Secret { key, source, .. }) =
                        pm.rows.get(pm.selected)
                    {
                        if *source != SecretSource::NotSet {
                            let key = key.clone();
                            // Delete from keychain.
                            let _ = std::process::Command::new("security")
                                .args(["delete-generic-password", "-s", &key])
                                .stderr(std::process::Stdio::null())
                                .status();
                            // Refresh the manager state.
                            self.state.plugin_manager = Some(self.build_plugin_manager_state());
                            self.state.status_message =
                                Some((format!("Deleted {key}"), std::time::Instant::now()));
                        }
                    }
                }
            }

            Action::WidgetFocusUp => {
                if self.state.widgets_visible && !self.state.widget_indices.is_empty() {
                    self.state.widget_focused = true;
                    self.state.vim_mode = VimMode::Normal;
                }
            }

            Action::WidgetDisable => {
                if self.state.widget_focused {
                    if let Some(&pidx) = self.state.widget_indices.get(self.state.widget_selected) {
                        let meta = &self.state.plugins[pidx];
                        let gk = meta
                            .plugin_group
                            .as_deref()
                            .unwrap_or(&meta.name)
                            .to_string();
                        let name = meta.name.clone();
                        self.pm_config.toggle_widget(&gk, &name);
                        if let Err(e) = crate::config::save_plugin_manager_config(&self.pm_config) {
                            tracing::warn!(error = %e, "failed to save widget config");
                        }
                        self.rebuild_widget_indices();
                        self.state.status_message =
                            Some((format!("Hidden widget: {name}"), std::time::Instant::now()));
                    }
                }
            }

            Action::WidgetMoveLeft => {
                if self.state.widget_focused && self.state.widget_selected > 0 {
                    // Ensure widget_order has all current widgets.
                    self.ensure_widget_order();
                    if let Some(&pidx) = self.state.widget_indices.get(self.state.widget_selected) {
                        let meta = &self.state.plugins[pidx];
                        let gk = meta.plugin_group.as_deref().unwrap_or(&meta.name);
                        self.pm_config.move_widget_up(gk, &meta.name);
                        if let Err(e) = crate::config::save_plugin_manager_config(&self.pm_config) {
                            tracing::warn!(error = %e, "failed to save widget order");
                        }
                        self.state.widget_selected -= 1;
                        self.rebuild_widget_indices();
                    }
                }
            }

            Action::WidgetMoveRight => {
                if self.state.widget_focused
                    && self.state.widget_selected + 1 < self.state.widget_indices.len()
                {
                    self.ensure_widget_order();
                    if let Some(&pidx) = self.state.widget_indices.get(self.state.widget_selected) {
                        let meta = &self.state.plugins[pidx];
                        let gk = meta.plugin_group.as_deref().unwrap_or(&meta.name);
                        self.pm_config.move_widget_down(gk, &meta.name);
                        if let Err(e) = crate::config::save_plugin_manager_config(&self.pm_config) {
                            tracing::warn!(error = %e, "failed to save widget order");
                        }
                        self.state.widget_selected += 1;
                        self.rebuild_widget_indices();
                    }
                }
            }

            Action::WidgetToggleVisibility => {
                if !self.state.widget_indices.is_empty() {
                    self.state.widgets_visible = !self.state.widgets_visible;
                    self.state.widget_focused = false;
                }
            }

            Action::WidgetPickerOpen => {
                // Build entries from all widget-eligible commands.
                let entries: Vec<WidgetPickerEntry> = self
                    .state
                    .plugins
                    .iter()
                    .filter(|m| m.widget)
                    .map(|m| {
                        let gk = m.plugin_group.as_deref().unwrap_or(&m.name);
                        let key = format!("{gk}:{}", m.name);
                        let label = if let Some(ref pg) = m.plugin_group {
                            format!("{pg}: {}", m.name)
                        } else {
                            m.name.clone()
                        };
                        let enabled = !self.pm_config.is_widget_disabled(gk, &m.name);
                        WidgetPickerEntry {
                            label,
                            icon: m.icon.clone(),
                            key,
                            enabled,
                        }
                    })
                    .collect();

                if entries.is_empty() {
                    self.state.status_message = Some((
                        "No widget-eligible plugins found".to_string(),
                        std::time::Instant::now(),
                    ));
                } else {
                    self.state.widget_picker = Some(WidgetPickerState {
                        entries,
                        selected: 0,
                        query: String::new(),
                        filtered_indices: Vec::new(),
                    });
                }
            }

            Action::WidgetPickerClose => {
                self.state.widget_picker = None;
            }

            Action::WidgetPickerUp => {
                if let Some(ref mut picker) = self.state.widget_picker {
                    if picker.selected > 0 {
                        picker.selected -= 1;
                    }
                }
            }

            Action::WidgetPickerDown => {
                if let Some(ref mut picker) = self.state.widget_picker {
                    let count = picker.visible_entries().len();
                    if picker.selected + 1 < count {
                        picker.selected += 1;
                    }
                }
            }

            Action::WidgetPickerToggle => {
                if let Some(ref mut picker) = self.state.widget_picker {
                    // Resolve actual entry index through filter.
                    let actual_idx = if picker.query.is_empty() {
                        picker.selected
                    } else {
                        picker
                            .filtered_indices
                            .get(picker.selected)
                            .copied()
                            .unwrap_or(picker.selected)
                    };
                    if let Some(entry) = picker.entries.get_mut(actual_idx) {
                        // Parse group_key and command_name from the key.
                        if let Some((gk, cmd)) = entry.key.split_once(':') {
                            self.pm_config.toggle_widget(gk, cmd);
                            entry.enabled = !self.pm_config.is_widget_disabled(gk, cmd);
                            if let Err(e) =
                                crate::config::save_plugin_manager_config(&self.pm_config)
                            {
                                tracing::warn!(error = %e, "failed to save widget config");
                            }
                            self.rebuild_widget_indices();
                            self.state.widgets_visible = !self.state.widget_indices.is_empty();
                        }
                    }
                }
            }

            Action::WidgetPickerSearch(c) => {
                if let Some(ref mut picker) = self.state.widget_picker {
                    picker.query.push(c);
                    picker.rebuild_filter();
                    picker.selected = 0;
                }
            }

            Action::WidgetPickerBackspace => {
                if let Some(ref mut picker) = self.state.widget_picker {
                    picker.query.pop();
                    picker.rebuild_filter();
                    picker.selected = 0;
                }
            }

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

            Action::MiniAppFocusNext => {
                if let Some(ref mut mini) = self.state.mini_app {
                    if let Some(pos) = mini.pane_order.iter().position(|id| *id == mini.focused_pane) {
                        let next = (pos + 1) % mini.pane_order.len();
                        mini.focused_pane = mini.pane_order[next].clone();
                    }
                }
            }

            Action::MiniAppFocusPrev => {
                if let Some(ref mut mini) = self.state.mini_app {
                    if let Some(pos) = mini.pane_order.iter().position(|id| *id == mini.focused_pane) {
                        let prev = if pos == 0 {
                            mini.pane_order.len().saturating_sub(1)
                        } else {
                            pos - 1
                        };
                        mini.focused_pane = mini.pane_order[prev].clone();
                    }
                }
            }

            Action::MiniAppClose => {
                self.state.mini_app = None;
                self.state.mode = Mode::Unified;
                self.state.viewing_plugin_index = None;
            }

            Action::MiniAppExpand => {
                // Expand current ViewOutput into a single-pane mini app.
                if self.state.mode == Mode::ViewOutput {
                    if let Some(ref output) = self.state.plugin_output {
                        if let Some(ref layout) = output.layout {
                            // Plugin returned a layout — use it.
                            let plugin_index = self.state.viewing_plugin_index.unwrap_or(0);
                            self.state.mini_app = Some(
                                crate::mini_app::build_mini_app_state(plugin_index, layout.clone()),
                            );
                            self.state.mode = Mode::MiniApp;
                        }
                    }
                }
            }

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

    /// Build the plugin manager state (all plugins collapsed by default).
    fn build_plugin_manager_state(&self) -> PluginManagerState {
        self.build_plugin_manager_state_with_expanded(&std::collections::HashSet::new())
    }

    /// Build plugin manager state with specified expanded groups.
    #[allow(clippy::type_complexity, clippy::too_many_lines)]
    fn build_plugin_manager_state_with_expanded(
        &self,
        expanded_keys: &std::collections::HashSet<String>,
    ) -> PluginManagerState {
        let env_secrets = crate::config::load_secrets();

        // Scan ALL plugins (unfiltered) so disabled ones still appear as [ ] in the manager.
        let all_meta: Vec<PluginMetadata> = match registry::scan(&self.plugin_dirs) {
            Ok(discovered) => discovered.iter().map(|d| d.metadata.clone()).collect(),
            Err(_) => self.state.plugins.clone(), // fallback to active set
        };

        // Collect unique plugin groups from metadata.
        let mut seen_groups: Vec<String> = Vec::new();
        let mut group_meta: std::collections::HashMap<
            String,
            (
                String,
                String,
                String,
                String,
                Vec<(String, Option<String>)>,
                Vec<crate::plugin::traits::FormField>,
                Vec<String>,
            ),
        > = std::collections::HashMap::new();

        for meta in &all_meta {
            let gk = meta
                .plugin_group
                .as_deref()
                .unwrap_or(&meta.name)
                .to_string();
            let entry = group_meta.entry(gk.clone()).or_insert_with(|| {
                seen_groups.push(gk.clone());
                (
                    meta.icon.clone(),
                    meta.category.clone().unwrap_or_default(),
                    meta.version.clone(),
                    gk.clone(),
                    Vec::new(),
                    meta.settings_spec.clone(),
                    meta.secrets.clone(),
                )
            });
            entry.4.push((meta.name.clone(), meta.quickkey.clone()));
        }

        let mut rows = Vec::new();
        for gk in &seen_groups {
            let (icon, cat, ver, _display, commands, settings, secrets) = &group_meta[gk];
            let is_expanded = expanded_keys.contains(gk);
            let plugin_enabled = !self.pm_config.is_plugin_disabled(gk);

            rows.push(PluginManagerRow::PluginHeader {
                group_key: gk.clone(),
                name: gk.clone(),
                icon: icon.clone(),
                category: cat.clone(),
                version: ver.clone(),
                enabled: plugin_enabled,
                expanded: is_expanded,
                command_count: commands.len(),
            });

            if is_expanded {
                // Command rows.
                for (cmd_name, qk) in commands {
                    let cmd_enabled =
                        plugin_enabled && !self.pm_config.is_command_disabled(gk, cmd_name);
                    rows.push(PluginManagerRow::Command {
                        group_key: gk.clone(),
                        name: cmd_name.clone(),
                        quickkey: qk.clone(),
                        enabled: cmd_enabled,
                    });
                }
                // Setting rows.
                let store_path = crate::plugin::store::store_path_for(gk, None);
                let store = crate::plugin::store::PluginStore::load(store_path);
                for spec in settings {
                    let value = store
                        .get(&spec.id)
                        .and_then(|v| v.as_str().map(str::to_string))
                        .or_else(|| spec.default_value.clone())
                        .unwrap_or_else(|| "(not set)".to_string());
                    rows.push(PluginManagerRow::Setting {
                        group_key: gk.clone(),
                        id: spec.id.clone(),
                        label: spec.label.clone(),
                        value,
                    });
                }
                // Secret rows.
                for key in secrets {
                    let source = if env_secrets.contains_key(key) {
                        SecretSource::DotEnv
                    } else if std::env::var(key).is_ok() {
                        SecretSource::EnvVar
                    } else if crate::config::keychain_has(key) {
                        SecretSource::Keychain
                    } else {
                        SecretSource::NotSet
                    };
                    rows.push(PluginManagerRow::Secret {
                        key: key.clone(),
                        source,
                    });
                }
            }
        }

        PluginManagerState {
            rows,
            selected: 0,
            expanded: expanded_keys.clone(),
        }
    }

    /// Build context-aware power menu categories based on the current mode.
    #[allow(clippy::too_many_lines)]
    fn build_power_menu_categories(&self) -> Vec<PowerMenuCategory> {
        match self.state.mode {
            Mode::Unified if self.state.widget_focused => {
                // Widget-focused context menu.
                vec![
                    PowerMenuCategory {
                        name: "Widget".to_string(),
                        items: vec![
                            PowerMenuItem {
                                key: 'h',
                                key_hint: "h/l".to_string(),
                                label: "Navigate cards".to_string(),
                                action: Action::Back,
                            },
                            PowerMenuItem {
                                key: 'j',
                                key_hint: "j".to_string(),
                                label: "Back to list".to_string(),
                                action: Action::MoveDown,
                            },
                            PowerMenuItem {
                                key: 'H',
                                key_hint: "H".to_string(),
                                label: "Move card left".to_string(),
                                action: Action::WidgetMoveLeft,
                            },
                            PowerMenuItem {
                                key: 'L',
                                key_hint: "L".to_string(),
                                label: "Move card right".to_string(),
                                action: Action::WidgetMoveRight,
                            },
                            PowerMenuItem {
                                key: 'D',
                                key_hint: "D".to_string(),
                                label: "Hide this widget".to_string(),
                                action: Action::WidgetDisable,
                            },
                            PowerMenuItem {
                                key: 'W',
                                key_hint: "W".to_string(),
                                label: "Hide all widgets".to_string(),
                                action: Action::WidgetToggleVisibility,
                            },
                        ],
                    },
                    PowerMenuCategory {
                        name: "App".to_string(),
                        items: {
                            let mut items = vec![PowerMenuItem {
                                key: 'P',
                                key_hint: "P".to_string(),
                                label: "Plugins".to_string(),
                                action: Action::PluginManagerOpen,
                            }];
                            if self.state.update_hint.is_some() {
                                items.push(PowerMenuItem {
                                    key: 'U',
                                    key_hint: "U".to_string(),
                                    label: "Upgrade lark".to_string(),
                                    action: Action::RunUpgrade,
                                });
                            }
                            items.push(PowerMenuItem {
                                key: 'q',
                                key_hint: "q".to_string(),
                                label: "Quit".to_string(),
                                action: Action::Quit,
                            });
                            items
                        },
                    },
                ]
            }
            Mode::Unified => {
                let mut widget_items = vec![
                    PowerMenuItem {
                        key: 'K',
                        key_hint: "K".to_string(),
                        label: "Focus widgets".to_string(),
                        action: Action::WidgetFocusUp,
                    },
                    PowerMenuItem {
                        key: 'W',
                        key_hint: "W".to_string(),
                        label: if self.state.widgets_visible {
                            "Hide widgets".to_string()
                        } else {
                            "Show widgets".to_string()
                        },
                        action: Action::WidgetToggleVisibility,
                    },
                ];
                // Only show widget items if there are widgets.
                if self.state.widget_indices.is_empty() {
                    widget_items.clear();
                }

                vec![
                    PowerMenuCategory {
                        name: "Navigation".to_string(),
                        items: vec![
                            PowerMenuItem {
                                key: '/',
                                key_hint: "/".to_string(),
                                label: "Search".to_string(),
                                action: Action::EnterInsertMode,
                            },
                            PowerMenuItem {
                                key: ':',
                                key_hint: ":".to_string(),
                                label: "Command".to_string(),
                                action: Action::EnterCommandMode,
                            },
                        ],
                    },
                    PowerMenuCategory {
                        name: "Display".to_string(),
                        items: {
                            let mut items = vec![
                                PowerMenuItem {
                                    key: 'd',
                                    key_hint: "d".to_string(),
                                    label: "Descriptions".to_string(),
                                    action: Action::ToggleDescriptions,
                                },
                                PowerMenuItem {
                                    key: 'O',
                                    key_hint: "O".to_string(),
                                    label: format!("Sort: {}", self.state.sort_mode.next().label()),
                                    action: Action::CycleSort,
                                },
                                PowerMenuItem {
                                    key: 'R',
                                    key_hint: "R".to_string(),
                                    label: "Refresh".to_string(),
                                    action: Action::RefreshPlugins,
                                },
                                PowerMenuItem {
                                    key: 's',
                                    key_hint: "s".to_string(),
                                    label: "Sidebar".to_string(),
                                    action: Action::ToggleSidebar,
                                },
                                PowerMenuItem {
                                    key: 'T',
                                    key_hint: "T".to_string(),
                                    label: "Theme".to_string(),
                                    action: Action::ThemePickerOpen,
                                },
                            ];
                            items.extend(widget_items);
                            items
                        },
                    },
                    PowerMenuCategory {
                        name: "App".to_string(),
                        items: {
                            let mut items = vec![PowerMenuItem {
                                key: 'P',
                                key_hint: "P".to_string(),
                                label: "Plugins".to_string(),
                                action: Action::PluginManagerOpen,
                            }];
                            if self.state.update_hint.is_some() {
                                items.push(PowerMenuItem {
                                    key: 'U',
                                    key_hint: "U".to_string(),
                                    label: "Upgrade lark".to_string(),
                                    action: Action::RunUpgrade,
                                });
                            }
                            items.push(PowerMenuItem {
                                key: 'q',
                                key_hint: "q".to_string(),
                                label: "Quit".to_string(),
                                action: Action::Quit,
                            });
                            items
                        },
                    },
                ]
            }
            Mode::PluginManager => vec![
                PowerMenuCategory {
                    name: "Plugin Manager".to_string(),
                    items: vec![
                        PowerMenuItem {
                            key: ' ',
                            key_hint: "SPC".to_string(),
                            label: "Toggle enable".to_string(),
                            action: Action::PluginManagerToggle,
                        },
                        PowerMenuItem {
                            key: '\n',
                            key_hint: "⏎".to_string(),
                            label: "Expand/collapse".to_string(),
                            action: Action::PluginManagerExpand,
                        },
                        PowerMenuItem {
                            key: 's',
                            key_hint: "s".to_string(),
                            label: "Set secret".to_string(),
                            action: Action::PluginManagerSetSecret,
                        },
                        PowerMenuItem {
                            key: 'x',
                            key_hint: "x".to_string(),
                            label: "Delete secret".to_string(),
                            action: Action::PluginManagerDeleteSecret,
                        },
                    ],
                },
                PowerMenuCategory {
                    name: "App".to_string(),
                    items: vec![PowerMenuItem {
                        key: 'q',
                        key_hint: "q".to_string(),
                        label: "Back".to_string(),
                        action: Action::PluginManagerClose,
                    }],
                },
            ],
            Mode::ViewOutput => vec![
                {
                    let has_settings = self
                        .state
                        .viewing_plugin_index
                        .and_then(|i| self.state.plugins.get(i))
                        .is_some_and(|p| !p.settings_spec.is_empty());

                    let mut action_items = vec![
                        PowerMenuItem {
                            key: ':',
                            key_hint: ":".to_string(),
                            label: "Palette".to_string(),
                            action: Action::PaletteOpen,
                        },
                        PowerMenuItem {
                            key: 'o',
                            key_hint: "o".to_string(),
                            label: "Open URL".to_string(),
                            action: Action::OpenUrl,
                        },
                        PowerMenuItem {
                            key: 'y',
                            key_hint: "y".to_string(),
                            label: "Copy".to_string(),
                            action: Action::CopyLabel,
                        },
                        PowerMenuItem {
                            key: 'Y',
                            key_hint: "Y".to_string(),
                            label: "Copy Menu".to_string(),
                            action: Action::CopyMenu,
                        },
                    ];
                    if has_settings {
                        action_items.push(PowerMenuItem {
                            key: 'S',
                            key_hint: "S".to_string(),
                            label: "Settings".to_string(),
                            action: Action::OpenSettings,
                        });
                    }
                    PowerMenuCategory {
                        name: "Actions".to_string(),
                        items: action_items,
                    }
                },
                PowerMenuCategory {
                    name: "Display".to_string(),
                    items: vec![
                        PowerMenuItem {
                            key: 't',
                            key_hint: "t".to_string(),
                            label: "Toggle View".to_string(),
                            action: Action::ToggleOutputMode,
                        },
                        PowerMenuItem {
                            key: '/',
                            key_hint: "/".to_string(),
                            label: "Search".to_string(),
                            action: Action::OutputEnterSearch,
                        },
                        PowerMenuItem {
                            key: 's',
                            key_hint: "s".to_string(),
                            label: "Sidebar".to_string(),
                            action: Action::ToggleSidebar,
                        },
                        PowerMenuItem {
                            key: 'T',
                            key_hint: "T".to_string(),
                            label: "Theme".to_string(),
                            action: Action::ThemePickerOpen,
                        },
                    ],
                },
                PowerMenuCategory {
                    name: "App".to_string(),
                    items: vec![
                        PowerMenuItem {
                            key: 'r',
                            key_hint: "r".to_string(),
                            label: "Rerun".to_string(),
                            action: Action::RerunCommand,
                        },
                        PowerMenuItem {
                            key: 'd',
                            key_hint: "d".to_string(),
                            label: "Descriptions".to_string(),
                            action: Action::ToggleDescriptions,
                        },
                        PowerMenuItem {
                            key: 'q',
                            key_hint: "q".to_string(),
                            label: "Quit".to_string(),
                            action: Action::Quit,
                        },
                    ],
                },
            ],
            Mode::MiniApp => vec![], // TODO(Phase D): mini app power menu
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
                let context = action.args.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");
                if let Some(plugin_index) = self.state.viewing_plugin_index {
                    self.state.is_loading = true;
                    self.engine.execute_action(plugin_index, callback_id, context);
                }
            }
            ActionKind::UpdatePane => {
                // UpdatePane: call on_action to update a specific pane (mini app mode).
                // args[0] = pane_id, args[1] = callback_id, args[2..] = context.
                let _pane_id = action.args.first().cloned().unwrap_or_default();
                let callback_id = action.args.get(1).cloned().unwrap_or_default();
                let context = action.args.iter().skip(2).cloned().collect::<Vec<_>>().join(" ");
                if let Some(plugin_index) = self.state.viewing_plugin_index {
                    self.state.is_loading = true;
                    self.engine.execute_action(plugin_index, callback_id, context);
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

        self.reset_output_search();
        self.state.viewing_plugin_index = Some(plugin_index);
        let cache_enabled = self.state.plugins.get(plugin_index).is_none_or(|p| p.cache);
        match self.state.result_cache.get(&plugin_index).cloned() {
            Some(CachedResult::Ready(output)) if cache_enabled => {
                // Stale-while-revalidate: show cached output immediately, refresh in background.
                self.state.output_mode = Self::output_mode_for(&output);
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
                self.state.output_mode = Self::output_mode_for(&output);
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
        self.rebuild_output_filter();
        self.check_form_init(plugin_index);
        self.state.vim_mode = VimMode::Normal;
    }

    /// Number of visible output items (filtered count when searching, total otherwise).
    fn visible_output_count(&self) -> usize {
        if !self.state.output_filtered_indices.is_empty() || !self.state.output_query.is_empty() {
            self.state.output_filtered_indices.len()
        } else {
            self.state
                .plugin_output
                .as_ref()
                .map_or(0, |o| o.items.len())
        }
    }

    /// Returns the output item at the current `output_selected` position,
    /// mapped through `output_filtered_indices` when a search is active.
    fn selected_output_item(&self) -> Option<&crate::plugin::traits::OutputItem> {
        let items = &self.state.plugin_output.as_ref()?.items;
        if self.state.output_filtered_indices.is_empty() && self.state.output_query.is_empty() {
            items.get(self.state.output_selected)
        } else {
            let real_index = *self
                .state
                .output_filtered_indices
                .get(self.state.output_selected)?;
            items.get(real_index)
        }
    }

    /// Rebuild `output_filtered_indices` based on `output_query`.
    ///
    /// Empty query → all item indices. Non-empty → case-insensitive substring match on label+detail.
    fn rebuild_output_filter(&mut self) {
        let items = if let Some(ref o) = self.state.plugin_output {
            &o.items
        } else {
            self.state.output_filtered_indices.clear();
            return;
        };

        if self.state.output_query.is_empty() {
            self.state.output_filtered_indices = (0..items.len()).collect();
        } else {
            let query_lower = self.state.output_query.to_lowercase();
            self.state.output_filtered_indices = items
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    let haystack = match item.detail {
                        Some(ref d) => format!("{} {d}", item.label),
                        None => item.label.clone(),
                    };
                    haystack.to_lowercase().contains(&query_lower)
                })
                .map(|(i, _)| i)
                .collect();
        }

        // Clamp selection to filtered range.
        let max = self.state.output_filtered_indices.len().saturating_sub(1);
        if self.state.output_selected > max {
            self.state.output_selected = max;
        }
    }

    /// Reset output search state (called when entering `ViewOutput` or going Back).
    fn reset_output_search(&mut self) {
        self.state.output_query.clear();
        self.state.output_searching = false;
        self.state.output_filtered_indices.clear();
    }

    /// Determine the best output mode for the given output.
    fn output_mode_for(output: &PluginOutput) -> OutputMode {
        if output.output_format.as_deref() == Some("markdown") && output.raw_text.is_some() {
            OutputMode::Markdown
        } else if !output.columns.is_empty() {
            OutputMode::Table
        } else if output.raw_text.is_some() && output.items.is_empty() {
            OutputMode::RawText
        } else {
            OutputMode::List
        }
    }

    /// Check if the current plugin output has a form and initialize form state.
    fn check_form_init(&mut self, plugin_index: usize) {
        let form = self
            .state
            .plugin_output
            .as_ref()
            .and_then(|o| o.form.clone());
        if let Some(form_spec) = form {
            self.initialize_form(plugin_index, &form_spec);
        }
    }

    /// Initialize form state from a `FormSpec` returned by a plugin.
    fn initialize_form(
        &mut self,
        plugin_index: usize,
        form_spec: &crate::plugin::traits::FormSpec,
    ) {
        use crate::plugin::traits::FieldType;

        let fields: Vec<FormFieldState> = form_spec
            .fields
            .iter()
            .map(|field| {
                let default = field.default_value.clone().unwrap_or_default();
                let selected_option = if let FieldType::Select { ref options } = field.field_type {
                    options.iter().position(|o| o == &default).unwrap_or(0)
                } else {
                    0
                };
                let toggled = default == "true";
                let cursor = default.len();
                FormFieldState {
                    spec: field.clone(),
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
            submit_label: form_spec
                .submit_label
                .clone()
                .unwrap_or_else(|| "Submit".to_string()),
            is_settings: false,
        });
    }

    /// Ensure `widget_order` contains all current widget keys (for reordering).
    fn ensure_widget_order(&mut self) {
        let current: Vec<String> = self
            .state
            .widget_indices
            .iter()
            .map(|&i| {
                let m = &self.state.plugins[i];
                format!(
                    "{}:{}",
                    m.plugin_group.as_deref().unwrap_or(&m.name),
                    m.name
                )
            })
            .collect();
        // Add any missing keys to the order.
        for key in &current {
            if !self.pm_config.widget_order.contains(key) {
                self.pm_config.widget_order.push(key.clone());
            }
        }
        // Remove stale keys.
        self.pm_config.widget_order.retain(|k| current.contains(k));
    }

    /// Rebuild the list of widget plugin indices.
    fn rebuild_widget_indices(&mut self) {
        // Collect all widget-eligible indices, excluding disabled widgets.
        let mut indices: Vec<usize> = self
            .state
            .plugins
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                m.widget && {
                    let gk = m.plugin_group.as_deref().unwrap_or(&m.name);
                    !self.pm_config.is_widget_disabled(gk, &m.name)
                }
            })
            .map(|(i, _)| i)
            .collect();

        // Sort by widget_order config: ordered widgets first, rest in discovery order.
        if !self.pm_config.widget_order.is_empty() {
            let order = &self.pm_config.widget_order;
            indices.sort_by_key(|&i| {
                let m = &self.state.plugins[i];
                let key = format!(
                    "{}:{}",
                    m.plugin_group.as_deref().unwrap_or(&m.name),
                    m.name
                );
                order.iter().position(|k| k == &key).unwrap_or(usize::MAX)
            });
        }

        self.state.widget_indices = indices;
        self.state.widgets_visible = !self.state.widget_indices.is_empty();
        if self.state.widget_selected >= self.state.widget_indices.len() {
            self.state.widget_selected = 0;
        }
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
            scored.sort_unstable_by(|a, b| b.1.cmp(&a.1));

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
            self.sync_preview_index();
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
                self.sync_preview_index();
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
        self.sync_preview_index();
    }

    /// Update `preview_plugin_index` to match the currently selected unified row.
    fn sync_preview_index(&mut self) {
        self.state.preview_plugin_index = self
            .state
            .unified_rows
            .get(self.state.unified_selected)
            .map(|r| match r {
                UnifiedRow::Command { plugin_index, .. } => *plugin_index,
            });
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
