//! Configuration loading and defaults.

use std::collections::HashMap;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// Top-level application configuration.
///
/// Loaded from `~/.config/larkline/config.toml` on startup.
/// All fields have sensible defaults — a missing config file is not an error.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// General settings.
    pub general: GeneralConfig,
    /// UI appearance settings.
    pub ui: UiConfig,
    /// Logging settings.
    pub logging: LoggingConfig,
    /// Color theme settings.
    pub theme: ThemeConfig,
    /// Pinned/favorite plugins.
    pub favorites: FavoritesConfig,
    /// Keybinding overrides.
    pub keybindings: KeybindingsConfig,
    /// AI provider settings (Phase 5+).
    pub ai: AiConfig,
}

/// General application settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// Directories to scan for plugins.
    pub plugin_dirs: Vec<PathBuf>,
    /// Name of the plugin to pre-select when the app launches.
    pub default_plugin: Option<String>,
}

/// Which icon set to display in the plugin list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IconSet {
    /// Nerd Font glyphs (requires a Nerd Font installed). Falls back to emoji when `icon_nerd` is absent.
    Nerd,
    /// Standard emoji icons (works in any terminal).
    #[default]
    Emoji,
}

/// UI appearance settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Show icons next to plugin names.
    pub show_icons: bool,
    /// Maximum items visible in the plugin list before scrolling.
    pub visible_items: usize,
    /// Which icon set to use: `"nerd"` (default) or `"emoji"`.
    pub icon_set: IconSet,
    /// Maximum items shown per section in the unified list (0 = unlimited).
    pub max_items_per_section: usize,
    /// Show plugin descriptions in the unified list.
    pub show_descriptions: bool,
    /// Sidebar width percentage in browse mode (20-80, default 50).
    pub sidebar_ratio: u16,
}

/// Logging settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log level: error, warn, info, debug, trace.
    pub level: String,
}

/// Keybinding overrides for navigation actions.
///
/// Each field is an optional key string. If unset, the default hardcoded key is used.
/// Format: single char (`"k"`), named key (`"Enter"`, `"Escape"`), or modifier (`"Ctrl+d"`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    /// Move selection up. Default: `"k"` / Up arrow.
    pub move_up: Option<String>,
    /// Move selection down. Default: `"j"` / Down arrow.
    pub move_down: Option<String>,
    /// Execute the selected plugin. Default: `"Enter"`.
    pub select: Option<String>,
    /// Go back / close output pane. Default: `"Escape"`.
    pub back: Option<String>,
    /// Quit the application. Default: `"q"`.
    pub quit: Option<String>,
    /// Run the focused action in `ViewOutput`. Default: `"Enter"`.
    pub execute: Option<String>,
    /// Scroll the output pane down by half a page. Default: `"Ctrl+d"`.
    pub scroll_half_page_down: Option<String>,
    /// Scroll the output pane up by half a page. Default: `"Ctrl+u"`.
    pub scroll_half_page_up: Option<String>,
    /// Direct-launch map: key string → plugin name.
    #[serde(default)]
    pub launch: HashMap<String, String>,
}

/// Resolved keybindings — `KeyEvent` → `Action` maps built from [`KeybindingsConfig`].
///
/// Built once at startup; looked up on every keystroke in `Browse` and `ViewOutput` modes.
#[allow(clippy::struct_field_names)]
pub struct ResolvedKeybindings {
    pub browse_map: HashMap<KeyEvent, BrowseAction>,
    pub view_output_map: HashMap<KeyEvent, ViewOutputAction>,
    /// Direct-launch: key → plugin name.
    pub launch_map: HashMap<KeyEvent, String>,
}

/// Actions available in Browse mode (subset of all actions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowseAction {
    MoveUp,
    MoveDown,
    Select,
    Quit,
    /// Re-scan plugin directories.
    Refresh,
    /// Scroll the unified list down by half a page.
    ScrollHalfPageDown,
    /// Scroll the unified list up by half a page.
    ScrollHalfPageUp,
}

/// Actions available in `ViewOutput` mode (subset of all actions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewOutputAction {
    MoveUp,
    MoveDown,
    Back,
    Execute,
    Quit,
    /// Scroll down by half a page.
    ScrollHalfPageDown,
    /// Scroll up by half a page.
    ScrollHalfPageUp,
    /// Toggle between list and raw-text view.
    ToggleOutputMode,
    /// Copy the selected item's label to the clipboard.
    CopyLabel,
    /// Open the copy-menu overlay.
    CopyMenu,
    /// Enter search mode to filter output items.
    Search,
    /// Open the selected item's URL in the system browser.
    OpenUrl,
    /// Open the action palette for the selected item.
    ActionPalette,
}

impl KeybindingsConfig {
    /// Build [`ResolvedKeybindings`] from the config.
    ///
    /// Uses defaults for any unset field. Logs and skips invalid key strings.
    pub fn resolve(&self, plugins: &[crate::plugin::PluginMetadata]) -> ResolvedKeybindings {
        let browse_map = self.build_browse_map();
        let view_output_map = self.build_view_output_map();

        // ── Launch map ───────────────────────────────────────────────────────
        let mut launch_map: HashMap<KeyEvent, String> = HashMap::new();

        // Populate from plugin metadata keybindings first (lower priority).
        // For multi-command plugins, `quickkey` takes precedence over the legacy
        // `keybinding` field (multi-char quickkeys like "gb" are visual badges only
        // and are skipped by `parse_key`; single-char quickkeys become real shortcuts).
        for plugin in plugins {
            let kb = plugin.quickkey.as_deref().or(plugin.keybinding.as_deref());
            if let Some(kb) = kb {
                if let Ok(ev) = parse_key(kb) {
                    launch_map.entry(ev).or_insert_with(|| plugin.name.clone());
                }
            }
        }
        // Config overrides plugin metadata.
        for (key_str, plugin_name) in &self.launch {
            match parse_key(key_str) {
                Ok(ev) => {
                    launch_map.insert(ev, plugin_name.clone());
                }
                Err(e) => {
                    tracing::warn!(key = %key_str, error = %e, "invalid launch keybinding, skipping");
                }
            }
        }

        ResolvedKeybindings {
            browse_map,
            view_output_map,
            launch_map,
        }
    }

    fn build_browse_map(&self) -> HashMap<KeyEvent, BrowseAction> {
        let mut m: HashMap<KeyEvent, BrowseAction> = HashMap::new();
        m.insert(
            key(KeyCode::Char('k'), KeyModifiers::NONE),
            BrowseAction::MoveUp,
        );
        m.insert(key(KeyCode::Up, KeyModifiers::NONE), BrowseAction::MoveUp);
        m.insert(
            key(KeyCode::Char('j'), KeyModifiers::NONE),
            BrowseAction::MoveDown,
        );
        m.insert(
            key(KeyCode::Down, KeyModifiers::NONE),
            BrowseAction::MoveDown,
        );
        m.insert(
            key(KeyCode::Enter, KeyModifiers::NONE),
            BrowseAction::Select,
        );
        // l (vim: move right) — enter/select the highlighted plugin.
        m.insert(
            key(KeyCode::Char('l'), KeyModifiers::NONE),
            BrowseAction::Select,
        );
        m.insert(
            key(KeyCode::Char('q'), KeyModifiers::NONE),
            BrowseAction::Quit,
        );
        if let Some(ev) = parse_key_opt(self.move_up.as_deref()) {
            m.insert(ev, BrowseAction::MoveUp);
        }
        if let Some(ev) = parse_key_opt(self.move_down.as_deref()) {
            m.insert(ev, BrowseAction::MoveDown);
        }
        if let Some(ev) = parse_key_opt(self.select.as_deref()) {
            m.insert(ev, BrowseAction::Select);
        }
        if let Some(ev) = parse_key_opt(self.quit.as_deref()) {
            m.insert(ev, BrowseAction::Quit);
        }
        // R (uppercase) to refresh plugin list.
        m.insert(
            key(KeyCode::Char('R'), KeyModifiers::NONE),
            BrowseAction::Refresh,
        );
        m.insert(
            key(KeyCode::Char('R'), KeyModifiers::SHIFT),
            BrowseAction::Refresh,
        );
        // Default half-page scroll bindings for unified list.
        m.insert(
            key(KeyCode::Char('d'), KeyModifiers::CONTROL),
            BrowseAction::ScrollHalfPageDown,
        );
        m.insert(
            key(KeyCode::Char('u'), KeyModifiers::CONTROL),
            BrowseAction::ScrollHalfPageUp,
        );
        // Right arrow drills into a plugin (same as l / Enter).
        m.insert(
            key(KeyCode::Right, KeyModifiers::NONE),
            BrowseAction::Select,
        );
        // Config overrides for scroll bindings.
        if let Some(ev) = parse_key_opt(self.scroll_half_page_down.as_deref()) {
            m.insert(ev, BrowseAction::ScrollHalfPageDown);
        }
        if let Some(ev) = parse_key_opt(self.scroll_half_page_up.as_deref()) {
            m.insert(ev, BrowseAction::ScrollHalfPageUp);
        }
        m
    }

    fn build_view_output_map(&self) -> HashMap<KeyEvent, ViewOutputAction> {
        let mut m: HashMap<KeyEvent, ViewOutputAction> = HashMap::new();
        m.insert(
            key(KeyCode::Char('k'), KeyModifiers::NONE),
            ViewOutputAction::MoveUp,
        );
        m.insert(
            key(KeyCode::Up, KeyModifiers::NONE),
            ViewOutputAction::MoveUp,
        );
        m.insert(
            key(KeyCode::Char('j'), KeyModifiers::NONE),
            ViewOutputAction::MoveDown,
        );
        m.insert(
            key(KeyCode::Down, KeyModifiers::NONE),
            ViewOutputAction::MoveDown,
        );
        m.insert(
            key(KeyCode::Esc, KeyModifiers::NONE),
            ViewOutputAction::Back,
        );
        // h (vim: move left) — go back to the plugin list.
        m.insert(
            key(KeyCode::Char('h'), KeyModifiers::NONE),
            ViewOutputAction::Back,
        );
        m.insert(
            key(KeyCode::Enter, KeyModifiers::NONE),
            ViewOutputAction::Execute,
        );
        m.insert(
            key(KeyCode::Char('q'), KeyModifiers::NONE),
            ViewOutputAction::Quit,
        );
        m.insert(
            key(KeyCode::Backspace, KeyModifiers::NONE),
            ViewOutputAction::Back,
        );
        // Left arrow goes back (same as h / Esc / Backspace).
        m.insert(
            key(KeyCode::Left, KeyModifiers::NONE),
            ViewOutputAction::Back,
        );
        if let Some(ev) = parse_key_opt(self.move_up.as_deref()) {
            m.insert(ev, ViewOutputAction::MoveUp);
        }
        if let Some(ev) = parse_key_opt(self.move_down.as_deref()) {
            m.insert(ev, ViewOutputAction::MoveDown);
        }
        if let Some(ev) = parse_key_opt(self.back.as_deref()) {
            m.insert(ev, ViewOutputAction::Back);
        }
        if let Some(ev) = parse_key_opt(self.execute.as_deref()) {
            m.insert(ev, ViewOutputAction::Execute);
        }
        if let Some(ev) = parse_key_opt(self.quit.as_deref()) {
            m.insert(ev, ViewOutputAction::Quit);
        }
        // Default half-page scroll bindings.
        m.insert(
            key(KeyCode::Char('d'), KeyModifiers::CONTROL),
            ViewOutputAction::ScrollHalfPageDown,
        );
        m.insert(
            key(KeyCode::Char('u'), KeyModifiers::CONTROL),
            ViewOutputAction::ScrollHalfPageUp,
        );
        // Default toggle output mode binding.
        m.insert(
            key(KeyCode::Char('t'), KeyModifiers::NONE),
            ViewOutputAction::ToggleOutputMode,
        );
        // Clipboard: y copies label, Y (shift) opens copy menu.
        m.insert(
            key(KeyCode::Char('y'), KeyModifiers::NONE),
            ViewOutputAction::CopyLabel,
        );
        m.insert(
            key(KeyCode::Char('Y'), KeyModifiers::SHIFT),
            ViewOutputAction::CopyMenu,
        );
        // Search within output items.
        m.insert(
            key(KeyCode::Char('/'), KeyModifiers::NONE),
            ViewOutputAction::Search,
        );
        // Open URL shortcut.
        m.insert(
            key(KeyCode::Char('o'), KeyModifiers::NONE),
            ViewOutputAction::OpenUrl,
        );
        // Action palette (: opens searchable action list).
        m.insert(
            key(KeyCode::Char(':'), KeyModifiers::NONE),
            ViewOutputAction::ActionPalette,
        );
        // Config overrides for scroll bindings.
        if let Some(ev) = parse_key_opt(self.scroll_half_page_down.as_deref()) {
            m.insert(ev, ViewOutputAction::ScrollHalfPageDown);
        }
        if let Some(ev) = parse_key_opt(self.scroll_half_page_up.as_deref()) {
            m.insert(ev, ViewOutputAction::ScrollHalfPageUp);
        }
        m
    }
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

fn parse_key_opt(s: Option<&str>) -> Option<KeyEvent> {
    let s = s?;
    match parse_key(s) {
        Ok(ev) => Some(ev),
        Err(e) => {
            tracing::warn!(key = %s, error = %e, "invalid keybinding, using default");
            None
        }
    }
}

/// Parse a key string into a [`KeyEvent`].
///
/// Supported formats:
/// - Single printable char: `"k"`, `"j"`, `"q"`, `"/"`
/// - Named keys: `"Enter"`, `"Escape"`, `"Up"`, `"Down"`, `"Backspace"`, `"Tab"`, `"Delete"`
/// - Ctrl modifier: `"Ctrl+c"`, `"Ctrl+d"` (case-insensitive prefix)
pub fn parse_key(s: &str) -> anyhow::Result<KeyEvent> {
    // Ctrl+x modifier form
    if let Some(rest) = s.strip_prefix("Ctrl+").or_else(|| s.strip_prefix("ctrl+")) {
        let chars: Vec<char> = rest.chars().collect();
        anyhow::ensure!(
            chars.len() == 1,
            "Ctrl+ modifier requires a single character, got {rest:?}"
        );
        return Ok(KeyEvent::new(
            KeyCode::Char(chars[0].to_lowercase().next().unwrap()),
            KeyModifiers::CONTROL,
        ));
    }

    // Named keys (case-insensitive)
    match s.to_lowercase().as_str() {
        "enter" => return Ok(key(KeyCode::Enter, KeyModifiers::NONE)),
        "escape" | "esc" => return Ok(key(KeyCode::Esc, KeyModifiers::NONE)),
        "up" => return Ok(key(KeyCode::Up, KeyModifiers::NONE)),
        "down" => return Ok(key(KeyCode::Down, KeyModifiers::NONE)),
        "left" => return Ok(key(KeyCode::Left, KeyModifiers::NONE)),
        "right" => return Ok(key(KeyCode::Right, KeyModifiers::NONE)),
        "backspace" => return Ok(key(KeyCode::Backspace, KeyModifiers::NONE)),
        "delete" | "del" => return Ok(key(KeyCode::Delete, KeyModifiers::NONE)),
        "tab" => return Ok(key(KeyCode::Tab, KeyModifiers::NONE)),
        _ => {}
    }

    // Single printable character
    let chars: Vec<char> = s.chars().collect();
    anyhow::ensure!(
        chars.len() == 1 && !chars[0].is_control(),
        "key must be a single printable character or a named key, got {s:?}"
    );
    Ok(key(KeyCode::Char(chars[0]), KeyModifiers::NONE))
}

/// Favorites / pinned plugins configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FavoritesConfig {
    /// Plugin names to pin to the top of the list (in config order).
    pub pinned: Vec<String>,
}

/// AI provider identifiers. Selects which backend implements
/// [`crate::agent::Provider`] at startup based on `[ai] provider = "..."`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiProviderName {
    /// Anthropic Messages API (default — supports prompt caching for
    /// tool definitions, which saves ~40-70% on multi-turn agent loops).
    #[default]
    Anthropic,
    /// `OpenAI` Responses API (the forward-direction successor to Chat
    /// Completions).
    Openai,
    /// `OpenRouter` (`OpenAI`-compatible Chat Completions transport with a
    /// different base URL). Model capability varies — Larkline filters
    /// at startup via `/models`.
    Openrouter,
    /// Ollama local server (OpenAI-compatible at `localhost:11434/v1`).
    /// No API key. Tool-use requires a recent OSS model (Llama 3.2,
    /// Mistral 7B Instruct v0.3+, Qwen 2.5 7B+).
    Ollama,
}

#[allow(dead_code)] // wired in Phase 5.C+ by provider implementations
impl AiProviderName {
    /// Stable string identifier used in config files and CLI flags.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Openai => "openai",
            Self::Openrouter => "openrouter",
            Self::Ollama => "ollama",
        }
    }

    /// Conventional Keychain key holding this provider's API key. Ollama
    /// returns `None` because the local server doesn't require auth.
    #[must_use]
    pub const fn api_key_env(self) -> Option<&'static str> {
        match self {
            Self::Anthropic => Some("ANTHROPIC_API_KEY"),
            Self::Openai => Some("OPENAI_API_KEY"),
            Self::Openrouter => Some("OPENROUTER_API_KEY"),
            Self::Ollama => None,
        }
    }

    /// Provider-recommended default model. Used when the user hasn't
    /// configured one. Tuned for the launch thesis (Anthropic Opus 4.7
    /// for the headline experience; `OpenAI` 4o-mini for cost-conscious
    /// users; Ollama Llama 3.2 as a viable local default).
    #[must_use]
    pub const fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => "claude-opus-4-7",
            Self::Openai => "gpt-4o",
            Self::Openrouter => "anthropic/claude-3.5-sonnet",
            Self::Ollama => "llama3.2",
        }
    }
}

/// AI provider configuration. Selects the active provider and per-provider
/// model + base URL overrides. API keys come from the secrets pipeline
/// (Keychain / `.env`), not from this struct — the file lives in the
/// repo and shouldn't carry credentials.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    /// Active provider. Defaults to Anthropic.
    pub provider: AiProviderName,
    /// Model identifier passed to the active provider. Empty string ⇒
    /// use the provider's [`AiProviderName::default_model`].
    pub model: String,
    /// Override for the `OpenRouter` base URL. Empty ⇒
    /// `https://openrouter.ai/api/v1`.
    pub openrouter_base_url: String,
    /// Override for the Ollama base URL. Empty ⇒ `http://localhost:11434/v1`.
    pub ollama_base_url: String,
    /// Hard cap on output tokens per request. 0 ⇒ provider default.
    pub max_tokens: u32,
}

#[allow(dead_code)] // wired in Phase 5.C+ by provider implementations
impl AiConfig {
    /// Resolved model name for the active provider. Falls back to the
    /// provider default when the user hasn't set one.
    #[must_use]
    pub fn resolved_model(&self) -> &str {
        if self.model.is_empty() {
            self.provider.default_model()
        } else {
            self.model.as_str()
        }
    }

    /// Resolved `OpenRouter` base URL.
    #[must_use]
    pub fn resolved_openrouter_base_url(&self) -> &str {
        if self.openrouter_base_url.is_empty() {
            "https://openrouter.ai/api/v1"
        } else {
            self.openrouter_base_url.as_str()
        }
    }

    /// Resolved Ollama base URL.
    #[must_use]
    pub fn resolved_ollama_base_url(&self) -> &str {
        if self.ollama_base_url.is_empty() {
            "http://localhost:11434/v1"
        } else {
            self.ollama_base_url.as_str()
        }
    }

    /// `Some` token cap when the user set one, else `None` for provider
    /// default. Kept as the on-disk `u32` so an unset field round-trips
    /// as `0` instead of disappearing.
    #[must_use]
    pub const fn resolved_max_tokens(&self) -> Option<u32> {
        if self.max_tokens == 0 {
            None
        } else {
            Some(self.max_tokens)
        }
    }
}

/// All AI-provider Keychain/env keys we want resolved at startup. Passed
/// to [`resolve_keychain_secrets`] so users can store credentials with
/// `lark secret set <KEY>` and never see them in plaintext config files.
pub const AI_SECRET_KEYS: [&str; 3] = [
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "OPENROUTER_API_KEY",
];

/// Color theme configuration.
///
/// Colors can be ratatui named colors (e.g. `"cyan"`) or hex strings (e.g. `"#89b4fa"`).
/// Set `preset` to use a built-in theme; individual fields override preset values.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    /// Built-in preset name (e.g. `"nord"`, `"catppuccin-mocha"`). Optional.
    pub preset: Option<String>,
    /// Accent color — active borders, titles, cursor.
    pub accent: Option<String>,
    /// Primary text color.
    pub text: Option<String>,
    /// Dimmed text — descriptions, inactive borders.
    pub text_dimmed: Option<String>,
    /// Background color for the highlighted list row.
    pub highlight_bg: Option<String>,
    /// Foreground color for the highlighted list row.
    pub highlight_fg: Option<String>,
    /// Error message color.
    pub error: Option<String>,
    /// Status bar background.
    pub status_bar_bg: Option<String>,
}

/// Resolved color theme with ratatui `Color` values ready to use in rendering.
#[derive(Debug, Clone)]
pub struct Theme {
    pub accent: Color,
    pub text: Color,
    pub text_dimmed: Color,
    pub highlight_bg: Color,
    pub highlight_fg: Color,
    pub error: Color,
    pub status_bar_bg: Color,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            plugin_dirs: vec![default_plugin_dir()],
            default_plugin: None,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_icons: true,
            visible_items: 15,
            icon_set: IconSet::default(),
            max_items_per_section: 5,
            show_descriptions: false,
            sidebar_ratio: 50,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "warn".to_string(),
        }
    }
}

/// Preset color definitions — all strings, not resolved `Color` values.
struct PresetColors {
    accent: &'static str,
    text: &'static str,
    text_dimmed: &'static str,
    highlight_bg: &'static str,
    highlight_fg: &'static str,
    error: &'static str,
    status_bar_bg: &'static str,
}

/// Ordered list of built-in preset names and display labels.
pub const PRESET_NAMES: &[(&str, &str)] = &[
    ("default", "Default"),
    ("catppuccin-mocha", "Catppuccin Mocha"),
    ("nord", "Nord"),
    ("tokyo-night", "Tokyo Night"),
    ("dracula", "Dracula"),
    ("gruvbox-dark", "Gruvbox Dark"),
];

fn preset_colors(name: &str) -> &'static PresetColors {
    static DEFAULT: PresetColors = PresetColors {
        accent: "cyan",
        text: "white",
        text_dimmed: "darkgray",
        highlight_bg: "darkgray",
        highlight_fg: "white",
        error: "red",
        status_bar_bg: "black",
    };
    static CATPPUCCIN_MOCHA: PresetColors = PresetColors {
        accent: "#cba6f7",
        text: "#cdd6f4",
        text_dimmed: "#6c7086",
        highlight_bg: "#313244",
        highlight_fg: "#cdd6f4",
        error: "#f38ba8",
        status_bar_bg: "#1e1e2e",
    };
    static NORD: PresetColors = PresetColors {
        accent: "#88c0d0",
        text: "#eceff4",
        text_dimmed: "#4c566a",
        highlight_bg: "#3b4252",
        highlight_fg: "#eceff4",
        error: "#bf616a",
        status_bar_bg: "#2e3440",
    };
    static TOKYO_NIGHT: PresetColors = PresetColors {
        accent: "#7aa2f7",
        text: "#c0caf5",
        text_dimmed: "#565f89",
        highlight_bg: "#292e42",
        highlight_fg: "#c0caf5",
        error: "#f7768e",
        status_bar_bg: "#1a1b26",
    };
    static DRACULA: PresetColors = PresetColors {
        accent: "#bd93f9",
        text: "#f8f8f2",
        text_dimmed: "#6272a4",
        highlight_bg: "#44475a",
        highlight_fg: "#f8f8f2",
        error: "#ff5555",
        status_bar_bg: "#282a36",
    };
    static GRUVBOX_DARK: PresetColors = PresetColors {
        accent: "#d79921",
        text: "#ebdbb2",
        text_dimmed: "#928374",
        highlight_bg: "#3c3836",
        highlight_fg: "#ebdbb2",
        error: "#cc241d",
        status_bar_bg: "#282828",
    };

    match name {
        "catppuccin-mocha" => &CATPPUCCIN_MOCHA,
        "nord" => &NORD,
        "tokyo-night" => &TOKYO_NIGHT,
        "dracula" => &DRACULA,
        "gruvbox-dark" => &GRUVBOX_DARK,
        _ => &DEFAULT,
    }
}

impl ThemeConfig {
    /// Resolve into a `Theme` by applying the preset as base, then user overrides.
    ///
    /// Resolution order (highest priority wins):
    /// 1. Individual color fields in this config (`Some` values override the preset)
    /// 2. Built-in preset (named by `preset`, defaults to `"default"`)
    pub fn resolve(&self) -> anyhow::Result<Theme> {
        let base = preset_colors(self.preset.as_deref().unwrap_or("default"));
        Ok(Theme {
            accent: parse_color(self.accent.as_deref().unwrap_or(base.accent))?,
            text: parse_color(self.text.as_deref().unwrap_or(base.text))?,
            text_dimmed: parse_color(self.text_dimmed.as_deref().unwrap_or(base.text_dimmed))?,
            highlight_bg: parse_color(self.highlight_bg.as_deref().unwrap_or(base.highlight_bg))?,
            highlight_fg: parse_color(self.highlight_fg.as_deref().unwrap_or(base.highlight_fg))?,
            error: parse_color(self.error.as_deref().unwrap_or(base.error))?,
            status_bar_bg: parse_color(
                self.status_bar_bg.as_deref().unwrap_or(base.status_bar_bg),
            )?,
        })
    }

    /// Resolve a named preset directly into a `Theme` (ignoring per-field overrides).
    pub fn resolve_preset(name: &str) -> Theme {
        let base = preset_colors(name);
        Theme {
            accent: parse_color(base.accent).expect("preset colors are always valid"),
            text: parse_color(base.text).expect("preset colors are always valid"),
            text_dimmed: parse_color(base.text_dimmed).expect("preset colors are always valid"),
            highlight_bg: parse_color(base.highlight_bg).expect("preset colors are always valid"),
            highlight_fg: parse_color(base.highlight_fg).expect("preset colors are always valid"),
            error: parse_color(base.error).expect("preset colors are always valid"),
            status_bar_bg: parse_color(base.status_bar_bg).expect("preset colors are always valid"),
        }
    }
}

impl Theme {
    /// Returns the default theme (cyan accent, dark background).
    pub fn default_theme() -> Self {
        ThemeConfig::default()
            .resolve()
            .expect("default theme colors are always valid")
    }
}

/// Persist the selected theme preset name to the user's config file.
///
/// Uses `toml_edit` to update (or create) the `[theme] preset` key while
/// preserving all other config content and comments.
pub fn save_theme_preset(preset: &str) -> anyhow::Result<()> {
    let path = config_path();
    let content = if path.exists() {
        std::fs::read_to_string(&path)?
    } else {
        String::new()
    };
    let mut doc = content
        .parse::<toml_edit::DocumentMut>()
        .unwrap_or_default();
    if doc.get("theme").is_none() {
        doc["theme"] = toml_edit::table();
    }
    doc["theme"]["preset"] = toml_edit::value(preset);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, doc.to_string())?;
    Ok(())
}

/// Parse a color string into a ratatui `Color`.
///
/// Supported formats:
/// - Named: `"black"`, `"red"`, `"green"`, `"yellow"`, `"blue"`, `"magenta"`,
///   `"cyan"`, `"gray"`, `"darkgray"`, `"white"`
/// - Hex: `"#rrggbb"`
fn parse_color(s: &str) -> anyhow::Result<Color> {
    match s.to_lowercase().as_str() {
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "gray" => Ok(Color::Gray),
        "darkgray" => Ok(Color::DarkGray),
        "white" => Ok(Color::White),
        hex if hex.starts_with('#') && hex.len() == 7 => {
            let r = u8::from_str_radix(&hex[1..3], 16)?;
            let g = u8::from_str_radix(&hex[3..5], 16)?;
            let b = u8::from_str_radix(&hex[5..7], 16)?;
            Ok(Color::Rgb(r, g, b))
        }
        _ => anyhow::bail!("unknown color {s:?} — use a named color or #rrggbb hex"),
    }
}

/// The commented default config template written on first run.
const DEFAULT_CONFIG_TEMPLATE: &str = r#"# ~/.config/larkline/config.toml
#
# All fields are optional — defaults are shown below.
# Remove the leading '#' from any line to activate that setting.

[general]
# Directories to scan for plugins (tilde expansion not yet supported — use full paths).
# plugin_dirs = ["~/.config/larkline/plugins"]

# Pre-select a plugin by name when the app launches.
# default_plugin = "GitHub PRs"

[ui]
# Show emoji icons next to plugin names.
# show_icons = true

# Maximum items visible before scrolling.
# visible_items = 15

# Icon set: "nerd" (default, requires Nerd Font) or "emoji".
# icon_set = "nerd"

# Maximum items shown per section in the unified list (0 = unlimited).
# max_items_per_section = 5

# Sidebar width percentage in browse mode (20-80, default 50).
# sidebar_ratio = 50

[logging]
# Log level written to stderr. Options: error, warn, info, debug, trace.
# level = "warn"

[theme]
# Built-in preset: "default", "catppuccin-mocha", "nord", "tokyo-night", "dracula", "gruvbox-dark"
# Switch themes with Space → T in the app, or set preset here.
# preset = "default"
#
# Individual colors override the preset. Named values or #rrggbb hex.
# accent        = "cyan"
# text          = "white"
# text_dimmed   = "darkgray"
# highlight_bg  = "darkgray"
# highlight_fg  = "white"
# error         = "red"
# status_bar_bg = "black"

[favorites]
# Plugin names to pin to the top of the list (shown in this order, then rest alphabetically).
# pinned = ["GitHub PRs", "System Info"]

[keybindings]
# Override navigation keys. Formats: single char ("k"), named key ("Enter"),
# or Ctrl modifier ("Ctrl+d"). Search mode and Ctrl+C are not configurable.
# move_up   = "k"
# move_down = "j"
# select    = "Enter"
# back      = "Escape"
# quit      = "q"
# execute   = "Enter"

# Direct-launch: press a key from Browse mode to immediately execute a plugin.
# [keybindings.launch]
# "Ctrl+g" = "GitHub PRs"
# "Ctrl+s" = "System Info"

# AI provider (Phase 5+) — used by the AI plugins shipped in v1.0.
# API keys come from the secrets pipeline: run `lark secret set
# ANTHROPIC_API_KEY` (or OPENAI_API_KEY / OPENROUTER_API_KEY) once and
# they're stored in macOS Keychain, never on disk.
# [ai]
# provider             = "anthropic"            # anthropic | openai | openrouter | ollama
# model                = ""                     # blank = provider default
# max_tokens           = 0                      # 0 = provider default
# openrouter_base_url  = ""                     # blank = https://openrouter.ai/api/v1
# ollama_base_url      = ""                     # blank = http://localhost:11434/v1
"#;

/// Write the default commented config file if none exists.
///
/// Creates parent directories as needed. No-ops if the file already exists.
pub fn generate_default_if_missing() -> anyhow::Result<()> {
    write_default_if_missing(&config_path())
}

/// Inner implementation — testable with an arbitrary path.
fn write_default_if_missing(path: &std::path::Path) -> anyhow::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, DEFAULT_CONFIG_TEMPLATE)?;
    tracing::info!(path = %path.display(), "generated default config");
    Ok(())
}

/// Loads configuration from `~/.config/larkline/config.toml`.
///
/// Returns the default config if the file doesn't exist.
/// Returns a pair of `(Config, warnings)` — warnings is non-empty when the config
/// had parse errors or invalid field values (falls back to defaults for those fields).
/// Returns `Err` only for unrecoverable I/O errors (can't read the file at all).
pub fn load() -> anyhow::Result<(Config, Vec<String>)> {
    let path = config_path();
    if !path.exists() {
        return Ok((Config::default(), Vec::new()));
    }

    let contents = std::fs::read_to_string(&path)?;
    match toml::from_str::<Config>(&contents) {
        Ok(config) => Ok((config, Vec::new())),
        Err(e) => {
            let warning = format!("Config error: {e} — using defaults");
            tracing::error!(error = %e, "failed to parse config, falling back to defaults");
            Ok((Config::default(), vec![warning]))
        }
    }
}

/// Returns the path to the config file, respecting `XDG_CONFIG_HOME` if set.
pub fn config_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("larkline").join("config.toml")
    } else {
        home_dir()
            .join(".config")
            .join("larkline")
            .join("config.toml")
    }
}

/// Returns the default plugin directory.
pub fn default_plugin_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("larkline").join("plugins")
    } else {
        home_dir().join(".config").join("larkline").join("plugins")
    }
}

fn home_dir() -> PathBuf {
    // std::env::home_dir is deprecated due to Windows quirks, but
    // HOME env var is reliable on macOS/Linux which are our targets.
    std::env::var("HOME").map_or_else(|_| PathBuf::from("/tmp"), PathBuf::from)
}

/// Returns the path to the secrets `.env` file.
#[must_use]
pub fn env_path() -> PathBuf {
    config_path()
        .parent()
        .map_or_else(|| PathBuf::from(".env"), |p| p.join(".env"))
}

/// Strip matching surrounding single or double quotes from a `.env` value.
fn strip_quotes(value: &str) -> &str {
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

/// Load secrets from `~/.config/larkline/.env`.
///
/// Format: `KEY=VALUE` per line. Lines starting with `#` are comments.
/// Missing file returns an empty map. Invalid lines are silently skipped.
#[must_use]
pub fn load_secrets() -> std::collections::HashMap<String, String> {
    let path = env_path();
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return std::collections::HashMap::new();
    };

    let mut secrets = std::collections::HashMap::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            let key = key.trim();
            let value = strip_quotes(value.trim());
            if !key.is_empty() {
                secrets.insert(key.to_string(), value.to_string());
            }
        }
    }
    secrets
}

/// Fill in missing secrets from macOS Keychain.
///
/// For each declared secret key that isn't already in the map or the process
/// environment, queries `security find-generic-password -s <KEY> -w`. No-op
/// on non-macOS platforms.
pub fn resolve_keychain_secrets(
    secrets: &mut std::collections::HashMap<String, String>,
    declared_keys: &[&str],
) {
    if !cfg!(target_os = "macos") {
        return;
    }
    for key in declared_keys {
        if secrets.contains_key(*key) || std::env::var(key).is_ok() {
            continue;
        }
        if let Ok(output) = std::process::Command::new("security")
            .args(["find-generic-password", "-s", key, "-w"])
            .output()
        {
            if output.status.success() {
                let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !value.is_empty() {
                    tracing::info!(key = key, "loaded secret from macOS Keychain");
                    secrets.insert((*key).to_string(), value);
                }
            }
        }
    }
}

/// Generate a default `.env` template if one doesn't exist yet.
pub fn generate_env_if_missing() -> anyhow::Result<()> {
    let path = env_path();
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &path,
        "\
# Larkline Secrets
# Add API keys here. Plugins access them via lark.env(\"KEY_NAME\").
# This file should never be committed to version control.
#
# Fallback order: .env → process environment → macOS Keychain.
# To store a secret in Keychain instead of here:
#   security add-generic-password -U -a \"$USER\" -s KEY_NAME -w 'value'
#
# GITHUB_TOKEN=ghp_your_token_here
# OPENAI_API_KEY=sk-your_key_here
",
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Plugin Manager — enable/disable state
// ---------------------------------------------------------------------------

/// Persisted enable/disable state for the plugin manager.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PluginManagerConfig {
    /// Plugin group keys that are fully disabled.
    #[serde(default)]
    pub disabled_plugins: Vec<String>,
    /// Individual commands disabled as `"GroupKey:CommandName"`.
    #[serde(default)]
    pub disabled_commands: Vec<String>,
    /// Widget commands that are hidden from the dashboard as `"GroupKey:CommandName"`.
    #[serde(default)]
    pub disabled_widgets: Vec<String>,
    /// Ordered list of widget command keys for card arrangement.
    /// Commands not in this list appear after those that are, in default order.
    #[serde(default)]
    pub widget_order: Vec<String>,
}

/// Path to the plugin manager state file.
fn plugin_manager_path() -> std::path::PathBuf {
    let base = if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        std::path::PathBuf::from(xdg).join("larkline")
    } else {
        let home = std::env::var("HOME").map_or_else(
            |_| std::path::PathBuf::from("/tmp"),
            std::path::PathBuf::from,
        );
        home.join(".local").join("share").join("larkline")
    };
    base.join("plugin-manager.json")
}

/// Load plugin manager config from disk. Returns defaults if missing/corrupt.
#[must_use]
pub fn load_plugin_manager_config() -> PluginManagerConfig {
    let path = plugin_manager_path();
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return PluginManagerConfig::default();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

/// Save plugin manager config to disk.
pub fn save_plugin_manager_config(cfg: &PluginManagerConfig) -> anyhow::Result<()> {
    let path = plugin_manager_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(cfg)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Check if a key exists in macOS Keychain.
#[must_use]
pub fn keychain_has(key: &str) -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }
    std::process::Command::new("security")
        .args(["find-generic-password", "-s", key, "-w"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

impl PluginManagerConfig {
    /// Check if a plugin group is disabled.
    pub fn is_plugin_disabled(&self, group_key: &str) -> bool {
        self.disabled_plugins.iter().any(|k| k == group_key)
    }

    /// Check if a specific command is disabled.
    pub fn is_command_disabled(&self, group_key: &str, command_name: &str) -> bool {
        let key = format!("{group_key}:{command_name}");
        self.disabled_commands.iter().any(|k| k == &key)
    }

    /// Toggle a plugin's enabled state. Returns the new enabled state.
    pub fn toggle_plugin(&mut self, group_key: &str) -> bool {
        if let Some(pos) = self.disabled_plugins.iter().position(|k| k == group_key) {
            self.disabled_plugins.remove(pos);
            true // now enabled
        } else {
            self.disabled_plugins.push(group_key.to_string());
            false // now disabled
        }
    }

    /// Toggle a command's enabled state. Returns the new enabled state.
    pub fn toggle_command(&mut self, group_key: &str, command_name: &str) -> bool {
        let key = format!("{group_key}:{command_name}");
        if let Some(pos) = self.disabled_commands.iter().position(|k| k == &key) {
            self.disabled_commands.remove(pos);
            true // now enabled
        } else {
            self.disabled_commands.push(key);
            false // now disabled
        }
    }

    /// Check if a widget is disabled.
    pub fn is_widget_disabled(&self, group_key: &str, command_name: &str) -> bool {
        let key = format!("{group_key}:{command_name}");
        self.disabled_widgets.iter().any(|k| k == &key)
    }

    /// Toggle a widget's visibility. Returns the new visible state.
    pub fn toggle_widget(&mut self, group_key: &str, command_name: &str) -> bool {
        let key = format!("{group_key}:{command_name}");
        if let Some(pos) = self.disabled_widgets.iter().position(|k| k == &key) {
            self.disabled_widgets.remove(pos);
            true // now visible
        } else {
            self.disabled_widgets.push(key);
            false // now hidden
        }
    }

    /// Move a widget earlier in the order. Returns true if moved.
    pub fn move_widget_up(&mut self, group_key: &str, command_name: &str) -> bool {
        let key = format!("{group_key}:{command_name}");
        if let Some(pos) = self.widget_order.iter().position(|k| k == &key) {
            if pos > 0 {
                self.widget_order.swap(pos, pos - 1);
                return true;
            }
        }
        false
    }

    /// Move a widget later in the order. Returns true if moved.
    pub fn move_widget_down(&mut self, group_key: &str, command_name: &str) -> bool {
        let key = format!("{group_key}:{command_name}");
        if let Some(pos) = self.widget_order.iter().position(|k| k == &key) {
            if pos + 1 < self.widget_order.len() {
                self.widget_order.swap(pos, pos + 1);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = Config::default();
        assert!(config.ui.visible_items > 0);
        assert!(!config.general.plugin_dirs.is_empty());
        assert_eq!(config.logging.level, "warn");
    }

    #[test]
    fn config_parses_from_toml() {
        let toml = r#"
            [general]
            plugin_dirs = ["/tmp/plugins"]

            [ui]
            show_icons = false
            visible_items = 10
        "#;
        let config: Config = toml::from_str(toml).expect("parse failed");
        assert!(!config.ui.show_icons);
        assert_eq!(config.ui.visible_items, 10);
        assert_eq!(
            config.general.plugin_dirs,
            vec![PathBuf::from("/tmp/plugins")]
        );
    }

    #[test]
    fn missing_config_fields_use_defaults() {
        let toml = "[ui]\nshow_icons = false";
        let config: Config = toml::from_str(toml).expect("parse failed");
        // Only show_icons was overridden; everything else should be default
        assert!(!config.ui.show_icons);
        assert_eq!(config.ui.visible_items, 15);
    }

    // ── Theme tests ──────────────────────────────────────────────────────────

    #[test]
    fn default_theme_resolves_successfully() {
        ThemeConfig::default()
            .resolve()
            .expect("default theme must always resolve");
    }

    #[test]
    fn hex_color_parses_correctly() {
        let color = parse_color("#89b4fa").expect("valid hex");
        assert_eq!(color, Color::Rgb(0x89, 0xb4, 0xfa));
    }

    #[test]
    fn named_colors_parse() {
        assert_eq!(parse_color("cyan").unwrap(), Color::Cyan);
        assert_eq!(parse_color("CYAN").unwrap(), Color::Cyan);
        assert_eq!(parse_color("darkgray").unwrap(), Color::DarkGray);
        assert_eq!(parse_color("black").unwrap(), Color::Black);
        assert_eq!(parse_color("red").unwrap(), Color::Red);
    }

    #[test]
    fn invalid_color_returns_error() {
        assert!(parse_color("notacolor").is_err());
        assert!(parse_color("#gg0000").is_err());
        assert!(parse_color("#fff").is_err()); // short hex not supported
    }

    #[test]
    fn custom_theme_from_toml() {
        let toml = r##"
            [theme]
            accent = "#89b4fa"
            text_dimmed = "gray"
        "##;
        let config: Config = toml::from_str(toml).expect("parse failed");
        let theme = config.theme.resolve().expect("resolve failed");
        assert_eq!(theme.accent, Color::Rgb(0x89, 0xb4, 0xfa));
        assert_eq!(theme.text_dimmed, Color::Gray);
        // Unset fields use defaults
        assert_eq!(theme.error, Color::Red);
    }

    // ── Default config generation tests ─────────────────────────────────────

    #[test]
    fn generate_creates_file_when_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("larkline").join("config.toml");
        assert!(!path.exists());

        write_default_if_missing(&path).expect("should create file");

        assert!(path.exists(), "config file should have been created");
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("[general]"));
    }

    #[test]
    fn generate_does_not_overwrite_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "# custom").unwrap();

        write_default_if_missing(&path).expect("should succeed");

        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            contents, "# custom",
            "existing file must not be overwritten"
        );
    }

    // ── Key parsing tests ────────────────────────────────────────────────────

    #[test]
    fn parse_single_char_key() {
        let ev = parse_key("k").unwrap();
        assert_eq!(ev.code, KeyCode::Char('k'));
        assert_eq!(ev.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn parse_named_keys() {
        assert_eq!(parse_key("Enter").unwrap().code, KeyCode::Enter);
        assert_eq!(parse_key("enter").unwrap().code, KeyCode::Enter);
        assert_eq!(parse_key("Escape").unwrap().code, KeyCode::Esc);
        assert_eq!(parse_key("esc").unwrap().code, KeyCode::Esc);
        assert_eq!(parse_key("Up").unwrap().code, KeyCode::Up);
        assert_eq!(parse_key("Down").unwrap().code, KeyCode::Down);
        assert_eq!(parse_key("Backspace").unwrap().code, KeyCode::Backspace);
        assert_eq!(parse_key("Delete").unwrap().code, KeyCode::Delete);
        assert_eq!(parse_key("Tab").unwrap().code, KeyCode::Tab);
    }

    #[test]
    fn parse_ctrl_modifier() {
        let ev = parse_key("Ctrl+c").unwrap();
        assert_eq!(ev.code, KeyCode::Char('c'));
        assert_eq!(ev.modifiers, KeyModifiers::CONTROL);

        let ev2 = parse_key("ctrl+d").unwrap();
        assert_eq!(ev2.code, KeyCode::Char('d'));
        assert_eq!(ev2.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn parse_invalid_key_returns_error() {
        assert!(parse_key("notakey").is_err());
        assert!(parse_key("Ctrl+abc").is_err()); // multi-char after Ctrl+
        assert!(parse_key("").is_err());
    }

    #[test]
    fn default_keybindings_resolve() {
        let kb = KeybindingsConfig::default();
        let resolved = kb.resolve(&[]);
        // Default browse map should have j/k mapped
        assert!(
            resolved
                .browse_map
                .contains_key(&key(KeyCode::Char('k'), KeyModifiers::NONE))
        );
        assert!(
            resolved
                .browse_map
                .contains_key(&key(KeyCode::Char('j'), KeyModifiers::NONE))
        );
    }

    // ── Graceful config error handling tests ─────────────────────────────────

    #[test]
    fn malformed_toml_returns_defaults_with_warning() {
        // We can't call load() directly since it reads a real file path,
        // but we can test the TOML parse fallback logic inline.
        let bad_toml = "this is not valid toml ===";
        let result = toml::from_str::<Config>(bad_toml);
        assert!(result.is_err(), "bad TOML should fail to parse");
        // Verify that load() would fall back: simulate the match arm.
        let (config, warnings) = match result {
            Ok(c) => (c, Vec::new()),
            Err(e) => (
                Config::default(),
                vec![format!("Config error: {e} — using defaults")],
            ),
        };
        assert!(!warnings.is_empty(), "should have a warning");
        assert!(warnings[0].contains("Config error"));
        assert_eq!(config.logging.level, "warn"); // defaults
    }

    #[test]
    fn invalid_theme_color_falls_back_with_default_theme() {
        let theme_cfg = ThemeConfig {
            accent: Some("not_a_color".to_string()),
            ..ThemeConfig::default()
        };
        // resolve() returns Err — caller falls back to default theme.
        assert!(theme_cfg.resolve().is_err());
        // Default theme always resolves.
        let default_theme = ThemeConfig::default().resolve().unwrap();
        assert_eq!(default_theme.accent, Color::Cyan);
    }
}
