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
use crate::plugin::traits::{ActionKind, ItemAction, PluginOutput};
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
}

// ---------------------------------------------------------------------------
// State types
// ---------------------------------------------------------------------------

/// The current UI mode — describes *which pane is active*.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Unified launcher view — plugin sections + items, filterable by query.
    #[default]
    Unified,
    /// Viewing a plugin's output in the detail pane (table/raw-text fallback).
    ViewOutput,
}

/// A row in the unified launcher list.
#[derive(Debug, Clone)]
pub enum UnifiedRow {
    /// Non-selectable group header for multi-command plugins.
    GroupHeader { name: String, icon: String },
    /// A selectable command row.
    Command {
        /// Index into `AppState::plugins`.
        plugin_index: usize,
        name: String,
        description: String,
        icon: String,
        /// Quick-launch key badge (e.g., `"gb"`).
        quickkey: Option<String>,
        /// Parent group name shown as a dimmed badge during search (non-empty query).
        group_name: Option<String>,
        /// Nucleo match positions into `name` for character highlighting.
        match_positions: Vec<usize>,
    },
}

impl UnifiedRow {
    /// Returns true if this row can be selected by the user.
    pub fn is_selectable(&self) -> bool {
        matches!(self, Self::Command { .. })
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
}

impl App {
    /// Create a new `App` with the given set of plugins and config.
    pub fn new(plugins: Vec<Arc<dyn Plugin>>, config: &Config, warnings: Vec<String>) -> Self {
        let plugin_count = plugins.len();
        let (tx, rx) = mpsc::channel(plugin_count.max(1) * 3);
        let metadata: Vec<PluginMetadata> = plugins.iter().map(|p| p.metadata().clone()).collect();
        let engine = PluginEngine::new(plugins, tx);
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
        };
        app.rebuild_unified_list();

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
            } else {
                tracing::warn!(
                    plugin_name = %name,
                    "default_plugin not found in loaded plugins"
                );
            }
        }

        app
    }

    /// Create an `App` with stub plugins for testing.
    #[cfg(test)]
    pub fn with_stubs() -> Self {
        Self::new(stub_plugins(), &Config::default(), Vec::new())
    }

    /// Create an `App` with stub plugins and a favorites list for testing.
    #[cfg(test)]
    pub fn with_stubs_and_favorites(pinned: Vec<String>) -> Self {
        use crate::config::FavoritesConfig;
        let config = Config {
            favorites: FavoritesConfig { pinned },
            ..Config::default()
        };
        Self::new(stub_plugins(), &config, Vec::new())
    }

    /// Create an `App` with stub plugins and a `default_plugin` setting for testing.
    #[cfg(test)]
    pub fn with_stubs_and_default(default_plugin: &str) -> Self {
        let mut config = Config::default();
        config.general.default_plugin = Some(default_plugin.to_string());
        Self::new(stub_plugins(), &config, Vec::new())
    }

    /// Run the main event loop until the user quits.
    // The event loop uses crossterm's sync poll + tokio::spawn for plugins.
    // No direct .await calls here, but `run` must be async so main can await it.
    #[allow(clippy::unused_async)]
    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        // Kick off background prefetch for all eligible plugins.
        self.engine.execute_all();

        while !self.state.should_quit {
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
                        ) {
                            self.handle_action(action);
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
        }

        Ok(())
    }

    /// Process a single engine event, updating app state.
    ///
    /// Extracted from the run loop so it can be called from tests.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn handle_engine_event(&mut self, event: EngineEvent) {
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
                ExecutionSource::Prefetch => match result {
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
                },
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
                                    let has_columns = !output.columns.is_empty();
                                    self.state.plugin_output = Some(output);
                                    self.state.output_mode = if has_columns {
                                        OutputMode::Table
                                    } else {
                                        OutputMode::List
                                    };
                                }
                            } else {
                                // Fresh load: don't overwrite streaming output.
                                if self.state.plugin_output.is_none() {
                                    self.state.plugin_output = Some(output);
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
            },
        }
    }

    /// Apply an [`Action`] to the application state.
    #[allow(clippy::too_many_lines)]
    pub fn handle_action(&mut self, action: Action) {
        // Dismiss any config warnings on the first keypress.
        self.state.warnings.clear();

        match action {
            Action::Quit => self.state.should_quit = true,

            Action::MoveUp => {
                if self.state.mode == Mode::ViewOutput {
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
                    }
                }
            }

            Action::MoveDown => {
                if self.state.mode == Mode::ViewOutput {
                    let max = self
                        .state
                        .plugin_output
                        .as_ref()
                        .map_or(0, |o| o.items.len().saturating_sub(1));
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

            Action::Select => {
                if self.state.mode == Mode::ViewOutput {
                    // In ViewOutput, Select = Execute.
                    if let Some(ref output) = self.state.plugin_output.clone() {
                        if let Some(item) = output.items.get(self.state.output_selected) {
                            if let Some(action) = item.actions.first() {
                                self.execute_item_action(action);
                            } else if let Some(ref url) = item.url {
                                open_url(url);
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
                self.state.mode = Mode::Unified;
                self.state.plugin_output = None;
                self.state.plugin_error = None;
                self.state.output_selected = 0;
                self.state.output_mode = OutputMode::List;
                self.state.viewing_plugin_index = None;
            }

            Action::Execute => {
                if let Some(ref output) = self.state.plugin_output.clone() {
                    if let Some(item) = output.items.get(self.state.output_selected) {
                        if let Some(action) = item.actions.first() {
                            self.execute_item_action(action);
                        } else if let Some(ref url) = item.url {
                            open_url(url);
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
                if self.state.mode == Mode::ViewOutput {
                    let max = self
                        .state
                        .plugin_output
                        .as_ref()
                        .map_or(0, |o| o.items.len().saturating_sub(1));
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
                        }
                    }
                }
            }

            Action::ScrollHalfPageUp => {
                if self.state.mode == Mode::ViewOutput {
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
                self.state.output_mode = match self.state.output_mode {
                    OutputMode::List => OutputMode::RawText,
                    OutputMode::RawText if has_columns => OutputMode::Table,
                    OutputMode::RawText | OutputMode::Table => OutputMode::List,
                };
            }

            Action::Confirm => {
                if let Some(pending) = self.state.pending_confirmation.take() {
                    run_shell_action(&mut self.state, &pending.command, &pending.args);
                }
            }

            Action::Cancel => {
                self.state.pending_confirmation = None;
            }

            Action::EnterInsertMode => {
                self.state.vim_mode = VimMode::Insert;
            }

            Action::EnterNormalMode => {
                self.state.vim_mode = VimMode::Normal;
                self.state.command_input.clear();
                if self.state.mode == Mode::Unified {
                    self.state.query.clear();
                    self.rebuild_unified_list();
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

            Action::RefreshPlugins => match registry::scan(&self.plugin_dirs) {
                Ok(mut discovered) => {
                    // Resolve icons based on configured icon set.
                    if self.icon_set == crate::config::IconSet::Nerd {
                        for d in &mut discovered {
                            if let Some(ref nerd) = d.metadata.icon_nerd {
                                d.metadata.icon = nerd.clone();
                            }
                        }
                    }
                    let plugins: Vec<Arc<dyn Plugin>> = discovered
                        .into_iter()
                        .map(crate::plugin::build_plugin)
                        .collect();
                    let metadata: Vec<PluginMetadata> =
                        plugins.iter().map(|p| p.metadata().clone()).collect();
                    let plugin_count = plugins.len();
                    let (tx, rx) = mpsc::channel(plugin_count.max(1) * 3);
                    self.engine = PluginEngine::new(plugins, tx);
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
        }
    }

    /// Open a plugin's cached output in `ViewOutput` mode, or execute it if not cached.
    fn open_plugin_in_view_output(&mut self, plugin_index: usize) {
        self.state.viewing_plugin_index = Some(plugin_index);
        let cache_enabled = self.state.plugins.get(plugin_index).is_none_or(|p| p.cache);
        match self.state.result_cache.get(&plugin_index).cloned() {
            Some(CachedResult::Ready(output)) if cache_enabled => {
                // Stale-while-revalidate: show cached output immediately, refresh in background.
                let has_columns = !output.columns.is_empty();
                self.state.plugin_output = Some(output.clone());
                self.state.plugin_error = None;
                self.state.is_loading = false;
                self.state.output_selected = 0;
                self.state.output_mode = if has_columns {
                    OutputMode::Table
                } else {
                    OutputMode::List
                };
                self.state.mode = Mode::ViewOutput;
                self.state
                    .result_cache
                    .insert(plugin_index, CachedResult::Revalidating(output));
                self.engine.execute(plugin_index);
            }
            Some(CachedResult::Revalidating(output)) => {
                // Already revalidating — show stale data, don't trigger another execution.
                let has_columns = !output.columns.is_empty();
                self.state.plugin_output = Some(output);
                self.state.plugin_error = None;
                self.state.is_loading = false;
                self.state.output_selected = 0;
                self.state.output_mode = if has_columns {
                    OutputMode::Table
                } else {
                    OutputMode::List
                };
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
    }

    /// Rebuild the unified launcher list from plugin metadata.
    ///
    /// - **Empty query:** commands grouped by `plugin_group`, with favorites first.
    ///   Groups with >1 commands show a `GroupHeader` row before their `Command` rows.
    /// - **Non-empty query:** globally-ranked flat `Command` list scored by nucleo on
    ///   `name + description`. Each `Command` carries a `group_name` badge and
    ///   `match_positions` for character-level highlighting.
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
        rest.sort_unstable_by(|&a, &b| group_keys[a].cmp(&group_keys[b]));
        ordered.extend(rest);

        let rows = if query.is_empty() {
            // ── Grouped display (empty query) ─────────────────────────────────────
            // Walk `ordered`, collecting consecutive plugins with the same group key
            // into display groups. Emit a GroupHeader only for groups with >1 command.
            let mut result: Vec<UnifiedRow> = Vec::new();
            let mut i = 0;
            while i < ordered.len() {
                let this_key = group_keys[ordered[i]].clone();
                // Find the end of this group (first index with a different key).
                let group_end = ordered[i..]
                    .iter()
                    .position(|&j| group_keys[j] != this_key)
                    .map_or(ordered.len(), |pos| i + pos);
                let group_indices = ordered[i..group_end].to_vec();

                if group_indices.len() > 1 {
                    let icon = self.state.plugins[group_indices[0]].icon.clone();
                    result.push(UnifiedRow::GroupHeader {
                        name: this_key,
                        icon,
                    });
                }
                for pidx in group_indices {
                    let meta = &self.state.plugins[pidx];
                    result.push(UnifiedRow::Command {
                        plugin_index: pidx,
                        name: meta.name.clone(),
                        description: meta.description.clone(),
                        icon: meta.icon.clone(),
                        quickkey: meta.quickkey.clone(),
                        group_name: None,
                        match_positions: vec![],
                    });
                }
                i = group_end;
            }
            result
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
                let search_text = format!("{} {}", meta.name, meta.description);
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

        // Preserve selection on the same selectable row if possible.
        let old_selected = self.state.unified_selected;
        self.state.unified_rows = rows;

        // Clamp selection to a valid selectable row.
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

        // Find the nth selectable row where n = min(old_selected, selectable_count - 1).
        let target = old_selected.min(selectable_count.saturating_sub(1));
        self.state.unified_selected = self
            .state
            .unified_rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.is_selectable())
            .nth(target)
            .map_or(0, |(i, _)| i);
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
            state.plugin_output = Some(PluginOutput {
                title: format!("{cmd} (exit {})", output.status),
                raw_text: Some(combined),
                ..Default::default()
            });
            state.output_mode = OutputMode::RawText;
        }
        Err(e) => {
            state.plugin_error = Some(format!("shell command failed: {e}"));
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
            .filter_map(|r| match r {
                UnifiedRow::Command { name, .. } => Some(name.as_str()),
                UnifiedRow::GroupHeader { .. } => None,
            })
            .collect()
    }

    /// Returns true if any `GroupHeader` rows exist in `unified_rows`.
    fn has_group_headers(app: &App) -> bool {
        app.state
            .unified_rows
            .iter()
            .any(|r| matches!(r, UnifiedRow::GroupHeader { .. }))
    }

    #[test]
    fn empty_query_shows_all_commands() {
        let app = App::with_stubs();
        // All stubs are standalone (no plugin_group) → one Command row each, no GroupHeaders.
        let names = command_names(&app);
        assert_eq!(names.len(), app.state.plugins.len());
        assert!(!has_group_headers(&app));
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
        // "git" should match "GitHub PRs"; no group headers during search.
        let mut app = App::with_stubs();
        app.handle_action(Action::Search('g'));
        app.handle_action(Action::Search('i'));
        app.handle_action(Action::Search('t'));
        assert!(
            !has_group_headers(&app),
            "no group headers should appear during search"
        );
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
        let mut app = App::new(vec![], &config, vec![]);
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
