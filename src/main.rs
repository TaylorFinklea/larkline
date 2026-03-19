//! Larkline — the line to all your tools.
//!
//! A keyboard-driven terminal command palette.

use std::sync::Arc;

use anyhow::Result;
use tracing::info;

mod action;
mod app;
mod config;
mod input;
mod plugin;
mod tui;

#[tokio::main]
async fn main() -> Result<()> {
    // Generate a commented default config on first run.
    // Errors here are non-fatal — silently fall through.
    if let Err(e) = config::generate_default_if_missing() {
        eprintln!("larkline: could not generate default config ({e})");
    }

    // Load config first so we can use the configured log level.
    let (config, config_warnings) = config::load().unwrap_or_else(|e| {
        // Can't log yet — write to stderr directly since TUI isn't up.
        eprintln!("larkline: config I/O error ({e}), using defaults");
        (config::Config::default(), Vec::new())
    });

    // Parse log level from config; fall back to WARN on invalid values.
    let log_level: tracing::Level = config.logging.level.parse().unwrap_or(tracing::Level::WARN);

    // Initialize logging to stderr (hidden when TUI is active).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(log_level.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("larkline starting");

    let discovered = plugin::registry::scan(&config.general.plugin_dirs)?;
    let plugins: Vec<Arc<dyn plugin::Plugin>> = discovered
        .into_iter()
        .map(|d| {
            Arc::new(plugin::script::ScriptPlugin::from_discovered(d)) as Arc<dyn plugin::Plugin>
        })
        .collect();

    let mut terminal = tui::init()?;
    let result = app::App::new(plugins, &config, config_warnings)
        .run(&mut terminal)
        .await;
    tui::restore()?;

    result
}
