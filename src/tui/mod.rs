//! Terminal initialization and teardown.

pub mod ui;

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

/// Set up the terminal for TUI rendering.
///
/// Enables raw mode and enters the alternate screen buffer.
/// Always pair with [`restore`] — use `defer` or run in a scope that calls it.
pub fn init() -> Result<ratatui::DefaultTerminal> {
    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen)?;
    Ok(ratatui::init())
}

/// Restore the terminal to its original state.
///
/// Disables raw mode and exits the alternate screen buffer.
/// This must be called even if the application panics — consider a panic hook.
pub fn restore() -> Result<()> {
    ratatui::restore();
    execute!(std::io::stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
