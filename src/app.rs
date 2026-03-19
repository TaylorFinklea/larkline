//! Core application state and event loop.

use std::sync::Arc;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as NucleoConfig, Matcher, Utf32Str};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use crate::action::Action;
use crate::config::{Config, Theme};
use crate::input;
use crate::plugin::engine::{EngineEvent, PluginEngine};
use crate::plugin::traits::{ActionKind, ItemAction, PluginOutput};
use crate::plugin::{Plugin, PluginMetadata};
use crate::tui::ui;

// ---------------------------------------------------------------------------
// State types
// ---------------------------------------------------------------------------

/// The current UI mode, which determines how keyboard input is interpreted.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Browsing the plugin list.
    #[default]
    Browse,
    /// Typing a fuzzy search query to filter the plugin list.
    Search,
    /// Viewing a plugin's output in the detail pane.
    ViewOutput,
}

/// Central application state.
///
/// The TUI layer reads this struct to render; it never writes to it.
/// State transitions happen here in [`App`].
#[derive(Debug, Default)]
pub struct AppState {
    /// All known plugins (loaded from the registry).
    pub plugins: Vec<PluginMetadata>,
    /// Indices into `plugins` after fuzzy filtering. Ordered by match score.
    pub filtered: Vec<usize>,
    /// Match indices per filtered plugin, for highlight rendering.
    /// Parallel to `filtered` — `match_indices[i]` corresponds to `filtered[i]`.
    pub match_indices: Vec<Vec<usize>>,
    /// Index into `filtered` that is currently highlighted.
    pub selected: usize,
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
}

// ---------------------------------------------------------------------------
// App runner
// ---------------------------------------------------------------------------

/// The main application runner.
pub struct App {
    state: AppState,
    theme: Theme,
    engine: PluginEngine,
    rx: mpsc::Receiver<EngineEvent>,
}

impl App {
    /// Create a new `App` with the given set of plugins and config.
    pub fn new(plugins: Vec<Arc<dyn Plugin>>, config: &Config) -> Self {
        let (tx, rx) = mpsc::channel(4);
        let metadata: Vec<PluginMetadata> = plugins.iter().map(|p| p.metadata().clone()).collect();
        let filtered: Vec<usize> = (0..metadata.len()).collect();
        let engine = PluginEngine::new(plugins, tx);
        // Resolve theme; fall back to defaults on invalid colors.
        let theme = config.theme.resolve().unwrap_or_else(|e| {
            tracing::warn!(error = %e, "invalid theme color, falling back to defaults");
            Theme::default_theme()
        });
        let mut app = Self {
            state: AppState {
                plugins: metadata,
                filtered,
                show_icons: config.ui.show_icons,
                favorites: config.favorites.pinned.clone(),
                ..Default::default()
            },
            theme,
            engine,
            rx,
        };
        // Apply favorites ordering and alphabetical sort at startup.
        app.update_filter();

        // Apply default_plugin pre-selection.
        if let Some(ref name) = config.general.default_plugin {
            if let Some(pos) = app
                .state
                .filtered
                .iter()
                .position(|&i| &app.state.plugins[i].name == name)
            {
                app.state.selected = pos;
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
        Self::new(stub_plugins(), &Config::default())
    }

    /// Create an `App` with stub plugins and a favorites list for testing.
    #[cfg(test)]
    pub fn with_stubs_and_favorites(pinned: Vec<String>) -> Self {
        use crate::config::FavoritesConfig;
        let mut config = Config::default();
        config.favorites = FavoritesConfig { pinned };
        Self::new(stub_plugins(), &config)
    }

    /// Create an `App` with stub plugins and a default_plugin setting for testing.
    #[cfg(test)]
    pub fn with_stubs_and_default(default_plugin: &str) -> Self {
        let mut config = Config::default();
        config.general.default_plugin = Some(default_plugin.to_string());
        Self::new(stub_plugins(), &config)
    }

    /// Run the main event loop until the user quits.
    // The event loop uses crossterm's sync poll + tokio::spawn for plugins.
    // No direct .await calls here, but `run` must be async so main can await it.
    #[allow(clippy::unused_async)]
    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.state.should_quit {
            terminal.draw(|frame| ui::render(frame, &self.state, &self.theme))?;

            if event::poll(std::time::Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    // Only process key press events, not repeats or releases.
                    if key.kind == KeyEventKind::Press {
                        if let Some(action) = input::handle_key(key, &self.state.mode) {
                            self.handle_action(action);
                        }
                    }
                }
            }

            // Drain engine events (non-blocking).
            while let Ok(event) = self.rx.try_recv() {
                match event {
                    EngineEvent::PluginStarted { .. } => {
                        self.state.is_loading = true;
                        self.state.plugin_output = None;
                        self.state.plugin_error = None;
                    }
                    EngineEvent::PluginFinished { result, .. } => {
                        self.state.is_loading = false;
                        match result {
                            Ok(output) => {
                                self.state.plugin_output = Some(output);
                            }
                            Err(e) => {
                                self.state.plugin_error = Some(e.to_string());
                            }
                        }
                        self.state.mode = Mode::ViewOutput;
                        self.state.output_selected = 0;
                    }
                }
            }

            // Advance spinner.
            if self.state.is_loading {
                self.state.spinner_tick = self.state.spinner_tick.wrapping_add(1);
            }
        }

        Ok(())
    }

    /// Apply an [`Action`] to the application state.
    pub fn handle_action(&mut self, action: Action) {
        match action {
            Action::Quit => self.state.should_quit = true,

            Action::MoveUp => {
                if self.state.mode == Mode::ViewOutput {
                    if self.state.output_selected > 0 {
                        self.state.output_selected -= 1;
                    }
                } else if self.state.selected > 0 {
                    self.state.selected -= 1;
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
                    let max = self.state.filtered.len().saturating_sub(1);
                    if self.state.selected < max {
                        self.state.selected += 1;
                    }
                }
            }

            Action::Search(c) => {
                // Entering search mode if not already in it.
                // The '/' key is the trigger but we don't add it to the query.
                if self.state.mode == Mode::Search {
                    self.state.query.push(c);
                } else {
                    self.state.mode = Mode::Search;
                    if c != '/' {
                        self.state.query.push(c);
                    }
                }
                self.update_filter();
            }

            Action::BackspaceSearch => {
                self.state.query.pop();
                if self.state.query.is_empty() {
                    self.state.mode = Mode::Browse;
                }
                self.update_filter();
            }

            Action::ClearSearch => {
                self.state.query.clear();
                self.state.mode = Mode::Browse;
                self.update_filter();
            }

            Action::Select => {
                if !self.state.filtered.is_empty() {
                    let plugin_index = self.state.filtered[self.state.selected];
                    self.state.is_loading = true;
                    self.state.plugin_output = None;
                    self.state.plugin_error = None;
                    self.state.mode = Mode::ViewOutput;
                    self.engine.execute(plugin_index);
                }
            }

            Action::Back => {
                self.state.mode = Mode::Browse;
                self.state.plugin_output = None;
                self.state.plugin_error = None;
                self.state.output_selected = 0;
            }

            Action::Execute => {
                if let Some(ref output) = self.state.plugin_output.clone() {
                    if let Some(item) = output.items.get(self.state.output_selected) {
                        if let Some(action) = item.actions.first() {
                            App::execute_item_action(action);
                        } else if let Some(ref url) = item.url {
                            open_url(url);
                        }
                    }
                }
            }
        }
    }

    fn execute_item_action(action: &ItemAction) {
        match action.kind {
            ActionKind::Open => {
                if let Some(url) = action.args.first() {
                    open_url(url);
                }
            }
            ActionKind::Clipboard => {
                if let Some(text) = action.args.first() {
                    if let Err(e) = copy_to_clipboard(text) {
                        tracing::warn!(error = %e, "clipboard copy failed");
                    }
                }
            }
            ActionKind::Shell => {
                // Phase 4: shell execution with confirmation.
                tracing::info!(args = ?action.args, "shell action not yet implemented");
            }
        }
    }

    /// Recompute the filtered list based on the current query using nucleo fuzzy matching.
    ///
    /// Results are sorted by match score descending so the best matches appear first.
    /// `match_indices` is populated in parallel so the UI can highlight matched characters.
    fn update_filter(&mut self) {
        self.state.selected = 0;

        if self.state.query.is_empty() {
            // Partition into favorited (config order) then rest (alphabetical).
            let favorites = &self.state.favorites;
            let mut fav_indices: Vec<usize> = favorites
                .iter()
                .filter_map(|name| self.state.plugins.iter().position(|p| &p.name == name))
                .collect();
            let fav_set: std::collections::HashSet<usize> = fav_indices.iter().copied().collect();
            let mut rest: Vec<usize> = (0..self.state.plugins.len())
                .filter(|i| !fav_set.contains(i))
                .collect();
            rest.sort_unstable_by(|&a, &b| {
                self.state.plugins[a].name.cmp(&self.state.plugins[b].name)
            });
            fav_indices.append(&mut rest);
            self.state.filtered = fav_indices;
            self.state.match_indices = self.state.plugins.iter().map(|_| Vec::new()).collect();
            return;
        }

        let pattern = Pattern::new(
            &self.state.query,
            CaseMatching::Ignore,
            Normalization::Smart,
            nucleo_matcher::pattern::AtomKind::Fuzzy,
        );

        let mut matcher = Matcher::new(NucleoConfig::DEFAULT);
        let mut indices_buf = Vec::new();

        // Score each plugin against the pattern; fall back to description when name misses.
        let mut scored: Vec<(usize, u32, Vec<u32>)> = self
            .state
            .plugins
            .iter()
            .enumerate()
            .filter_map(|(idx, plugin)| {
                let mut name_chars: Vec<char> = plugin.name.chars().collect();
                let haystack = Utf32Str::new(&plugin.name, &mut name_chars);

                indices_buf.clear();
                if let Some(score) = pattern.indices(haystack, &mut matcher, &mut indices_buf) {
                    return Some((idx, score, indices_buf.clone()));
                }

                // Try description as fallback — no index highlighting for description matches.
                let mut desc_chars: Vec<char> = plugin.description.chars().collect();
                let desc_hay = Utf32Str::new(&plugin.description, &mut desc_chars);
                indices_buf.clear();
                pattern
                    .indices(desc_hay, &mut matcher, &mut indices_buf)
                    .map(|score| (idx, score, Vec::new()))
            })
            .collect();

        scored.sort_unstable_by(|a, b| b.1.cmp(&a.1));

        self.state.filtered = scored.iter().map(|(idx, _, _)| *idx).collect();
        self.state.match_indices = scored
            .into_iter()
            .map(|(_, _, mi)| mi.into_iter().map(|i| i as usize).collect::<Vec<_>>())
            .collect();
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
                category: Some($cat.to_string()),
                keybinding: None,
                timeout: Duration::from_secs(10),
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

    fn app_with_query(query: &str) -> App {
        let mut app = App::with_stubs();
        for c in query.chars() {
            app.handle_action(Action::Search(c));
        }
        app
    }

    #[test]
    fn fuzzy_filter_empty_query_returns_all() {
        let app = App::with_stubs();
        assert_eq!(app.state.filtered.len(), app.state.plugins.len());
    }

    #[test]
    fn favorites_sort_to_top_with_empty_query() {
        // "Weather" is alphabetically last among stubs, but favorited → should be first.
        let app = App::with_stubs_and_favorites(vec!["Weather".to_string()]);
        assert!(!app.state.filtered.is_empty());
        let first_name = &app.state.plugins[app.state.filtered[0]].name;
        assert_eq!(first_name, "Weather");
    }

    #[test]
    fn favorites_config_order_preserved() {
        // Multiple favorites should appear in the order specified, not alphabetically.
        let app =
            App::with_stubs_and_favorites(vec!["Weather".to_string(), "GitHub PRs".to_string()]);
        let first = &app.state.plugins[app.state.filtered[0]].name;
        let second = &app.state.plugins[app.state.filtered[1]].name;
        assert_eq!(first, "Weather");
        assert_eq!(second, "GitHub PRs");
    }

    #[test]
    fn non_favorite_rest_sorted_alphabetically() {
        // With no favorites, the list should be alphabetical.
        let app = App::with_stubs();
        let names: Vec<&str> = app
            .state
            .filtered
            .iter()
            .map(|&i| app.state.plugins[i].name.as_str())
            .collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
    }

    #[test]
    fn default_plugin_sets_selected_index() {
        // "Weather" is last alphabetically; default_plugin should pre-select it.
        let app = App::with_stubs_and_default("Weather");
        let selected_name = &app.state.plugins[app.state.filtered[app.state.selected]].name;
        assert_eq!(selected_name, "Weather");
    }

    #[test]
    fn missing_default_plugin_falls_back_to_zero() {
        // A plugin name that doesn't exist → selected stays at 0.
        let app = App::with_stubs_and_default("DoesNotExist");
        assert_eq!(app.state.selected, 0);
    }

    #[test]
    fn fuzzy_filter_matches_partial_name() {
        let app = app_with_query("git");
        let names: Vec<&str> = app
            .state
            .filtered
            .iter()
            .map(|&i| app.state.plugins[i].name.as_str())
            .collect();
        assert!(
            names.contains(&"GitHub PRs"),
            "expected 'GitHub PRs' in {names:?}"
        );
    }

    #[test]
    fn fuzzy_filter_matches_out_of_order() {
        // "gpr" should fuzzy-match "GitHub PRs" (G…P…R).
        let app = app_with_query("gpr");
        let names: Vec<&str> = app
            .state
            .filtered
            .iter()
            .map(|&i| app.state.plugins[i].name.as_str())
            .collect();
        assert!(
            names.contains(&"GitHub PRs"),
            "expected 'GitHub PRs' in {names:?}"
        );
    }

    #[test]
    fn fuzzy_filter_no_match_returns_empty() {
        let app = app_with_query("zzzzz");
        assert!(app.state.filtered.is_empty());
    }

    #[test]
    fn fuzzy_filter_results_ordered_by_score() {
        // "sys" should rank "System Info" above other matches (if any).
        let app = app_with_query("sys");
        assert!(!app.state.filtered.is_empty());
        let top_name = &app.state.plugins[app.state.filtered[0]].name;
        assert_eq!(top_name, "System Info");
    }

    #[test]
    fn move_up_down_in_view_output_changes_output_selected() {
        let mut app = App::with_stubs();
        // Set up ViewOutput mode with some items.
        app.state.mode = Mode::ViewOutput;
        app.state.plugin_output = Some(PluginOutput {
            title: "test".into(),
            items: vec![
                crate::plugin::traits::OutputItem {
                    label: "item 0".into(),
                    detail: None,
                    icon: None,
                    url: None,
                    actions: Vec::new(),
                },
                crate::plugin::traits::OutputItem {
                    label: "item 1".into(),
                    detail: None,
                    icon: None,
                    url: None,
                    actions: Vec::new(),
                },
            ],
            raw_text: None,
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
        assert_eq!(app.state.mode, Mode::Browse);
        assert!(app.state.plugin_output.is_none());
        assert_eq!(app.state.output_selected, 0);
    }
}
