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
    // Initialize logging to stderr (hidden when TUI is active).
    // Set RUST_LOG=debug to see output in a log file.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("larkline starting");

    let config = config::load()?;
    let discovered = plugin::registry::scan(&config.general.plugin_dirs)?;
    let plugins: Vec<Arc<dyn plugin::Plugin>> = discovered
        .into_iter()
        .map(|d| {
            Arc::new(plugin::script::ScriptPlugin::from_discovered(d)) as Arc<dyn plugin::Plugin>
        })
        .collect();

    let mut terminal = tui::init()?;
    let result = app::App::new(plugins).run(&mut terminal).await;
    tui::restore()?;

    result
}
