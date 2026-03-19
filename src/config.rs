//! Configuration loading and defaults.
// Config is not yet wired into `main.rs` (Phase 3). Suppress dead_code for scaffolding.
#![allow(dead_code)]

use std::path::PathBuf;

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
fn config_path() -> PathBuf {
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
}
