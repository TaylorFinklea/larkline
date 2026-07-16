//! Terminal initialization and teardown.

pub mod ansi_cache;
pub mod highlight;
pub mod markdown;
pub mod profile;
pub mod ui;

use std::io::stdout;

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

fn restore_after_panic() {
    let _ = ratatui::try_restore();
}

fn install_panic_hook_with(restore_terminal: fn()) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if crate::plugin::engine::PLUGIN_PANIC_ISOLATED
            .try_with(|()| ())
            .is_ok()
        {
            return;
        }
        restore_terminal();
        previous(info);
    }));
}

fn install_panic_hook() {
    install_panic_hook_with(restore_after_panic);
}

/// Set up the terminal for TUI rendering.
///
/// Enables raw mode and enters the alternate screen buffer.
/// Always pair with [`restore`] — use `defer` or run in a scope that calls it.
pub fn init() -> Result<ratatui::DefaultTerminal> {
    install_panic_hook();
    enable_raw_mode()?;
    if let Err(error) = execute!(stdout(), EnterAlternateScreen) {
        let _ = ratatui::try_restore();
        return Err(error.into());
    }
    let backend = CrosstermBackend::new(stdout());
    Terminal::new(backend).map_err(|error| {
        let _ = ratatui::try_restore();
        error.into()
    })
}

/// Restore the terminal to its original state.
///
/// Disables raw mode and exits the alternate screen buffer.
pub fn restore() -> Result<()> {
    ratatui::try_restore()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    const PANIC_HOOK_CHILD: &str = "LARKLINE_PANIC_HOOK_CHILD";
    const PANIC_HOOK_ISOLATED_CHILD: &str = "LARKLINE_PANIC_HOOK_ISOLATED_CHILD";
    const PANIC_HOOK_MARKER: &str = "LARKLINE_PANIC_HOOK_MARKER";

    fn append_marker(marker: &str) {
        let path = std::env::var(PANIC_HOOK_MARKER).expect("marker path");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("open marker");
        writeln!(file, "{marker}").expect("write marker");
    }

    fn marker_restore() {
        append_marker("restore");
    }

    #[test]
    fn release_profile_keeps_unwind_enabled() {
        let manifest = include_str!("../../Cargo.toml");

        assert!(!manifest.contains("panic = \"abort\""));
    }

    #[test]
    fn panic_hook_child() {
        if std::env::var_os(PANIC_HOOK_CHILD).is_none() {
            return;
        }
        std::panic::set_hook(Box::new(|_| append_marker("previous")));
        super::install_panic_hook_with(marker_restore);

        panic!("forced panic for terminal restoration test");
    }

    #[test]
    fn panic_hook_restores_before_previous_hook() {
        let dir = tempfile::tempdir().expect("tempdir");
        let marker = dir.path().join("panic-hook-order");
        let child = std::process::Command::new(std::env::current_exe().expect("current test exe"))
            .arg("tui::tests::panic_hook_child")
            .arg("--exact")
            .arg("--nocapture")
            .env(PANIC_HOOK_CHILD, "1")
            .env(PANIC_HOOK_MARKER, &marker)
            .output()
            .expect("run panic hook child");
        let order = std::fs::read_to_string(marker).expect("read marker");

        assert_eq!(
            (child.status.success(), order.as_str()),
            (false, "restore\nprevious\n")
        );
    }

    #[tokio::test]
    async fn isolated_plugin_panic_child() {
        if std::env::var_os(PANIC_HOOK_ISOLATED_CHILD).is_none() {
            return;
        }
        std::panic::set_hook(Box::new(|_| append_marker("previous")));
        super::install_panic_hook_with(marker_restore);

        let result = tokio::spawn(
            crate::plugin::engine::PLUGIN_PANIC_ISOLATED.scope((), async {
                panic!("forced isolated plugin panic");
            }),
        )
        .await;

        assert!(result.is_err());
    }

    #[test]
    fn isolated_plugin_panic_does_not_restore_the_running_tui() {
        let dir = tempfile::tempdir().expect("tempdir");
        let marker = dir.path().join("isolated-panic-hook");
        let child = std::process::Command::new(std::env::current_exe().expect("current test exe"))
            .arg("tui::tests::isolated_plugin_panic_child")
            .arg("--exact")
            .arg("--nocapture")
            .env(PANIC_HOOK_ISOLATED_CHILD, "1")
            .env(PANIC_HOOK_MARKER, &marker)
            .output()
            .expect("run isolated panic child");
        let hook_output = std::fs::read_to_string(marker).unwrap_or_default();

        assert_eq!((child.status.success(), hook_output), (true, String::new()));
    }
}
