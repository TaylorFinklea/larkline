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

/// UI appearance settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Show emoji icons next to plugin names.
    pub show_icons: bool,
    /// Maximum items visible in the plugin list before scrolling.
    pub visible_items: usize,
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
}

/// Actions available in `ViewOutput` mode (subset of all actions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewOutputAction {
    MoveUp,
    MoveDown,
    Back,
    Execute,
    Quit,
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
        for plugin in plugins {
            if let Some(ref kb) = plugin.keybinding {
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

/// Color theme configuration.
///
/// Colors can be ratatui named colors (e.g. `"cyan"`) or hex strings (e.g. `"#89b4fa"`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    /// Accent color — active borders, titles, cursor.
    pub accent: String,
    /// Primary text color.
    pub text: String,
    /// Dimmed text — descriptions, inactive borders.
    pub text_dimmed: String,
    /// Background color for the highlighted list row.
    pub highlight_bg: String,
    /// Foreground color for the highlighted list row.
    pub highlight_fg: String,
    /// Error message color.
    pub error: String,
    /// Status bar background.
    pub status_bar_bg: String,
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

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            accent: "cyan".to_string(),
            text: "white".to_string(),
            text_dimmed: "darkgray".to_string(),
            highlight_bg: "darkgray".to_string(),
            highlight_fg: "white".to_string(),
            error: "red".to_string(),
            status_bar_bg: "black".to_string(),
        }
    }
}

impl ThemeConfig {
    /// Resolve color strings into ratatui `Color` values.
    ///
    /// Returns an error if any color string cannot be parsed.
    pub fn resolve(&self) -> anyhow::Result<Theme> {
        Ok(Theme {
            accent: parse_color(&self.accent)?,
            text: parse_color(&self.text)?,
            text_dimmed: parse_color(&self.text_dimmed)?,
            highlight_bg: parse_color(&self.highlight_bg)?,
            highlight_fg: parse_color(&self.highlight_fg)?,
            error: parse_color(&self.error)?,
            status_bar_bg: parse_color(&self.status_bar_bg)?,
        })
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

[logging]
# Log level written to stderr. Options: error, warn, info, debug, trace.
# level = "warn"

[theme]
# Colors accept named values (black, red, green, yellow, blue, magenta, cyan,
# gray, darkgray, white) or hex strings (#rrggbb).
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
fn default_plugin_dir() -> PathBuf {
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
        let mut theme_cfg = ThemeConfig::default();
        theme_cfg.accent = "not_a_color".to_string();
        // resolve() returns Err — caller falls back to default theme.
        assert!(theme_cfg.resolve().is_err());
        // Default theme always resolves.
        let default_theme = ThemeConfig::default().resolve().unwrap();
        assert_eq!(default_theme.accent, Color::Cyan);
    }
}
