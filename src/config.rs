//! Configuration loading and defaults.

use std::path::PathBuf;

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
}

/// General application settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// Directories to scan for plugins.
    pub plugin_dirs: Vec<PathBuf>,
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

/// Loads configuration from `~/.config/larkline/config.toml`.
///
/// Returns the default config if the file doesn't exist.
/// Returns an error if the file exists but cannot be parsed.
pub fn load() -> anyhow::Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }

    let contents = std::fs::read_to_string(&path)?;
    let config: Config = toml::from_str(&contents)?;
    Ok(config)
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
}
