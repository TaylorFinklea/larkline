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

/// Returns shell alias/function text that binds Ctrl+L to launch `lark`.
fn alias_for_shell(shell: &str) -> String {
    match shell {
        "bash" => [
            "lark-widget() { lark; }",
            r#"bind -x '"\C-l": lark-widget'"#,
        ]
        .join("\n"),
        "fish" => [
            "function lark-widget",
            "    lark",
            "    commandline -f repaint",
            "end",
            r"bind \cl lark-widget",
        ]
        .join("\n"),
        // Default: zsh
        _ => [
            "lark-widget() { lark; zle reset-prompt; }",
            "zle -N lark-widget",
            "bindkey '^L' lark-widget",
        ]
        .join("\n"),
    }
}

fn print_alias(shell: &str) {
    println!("{}", alias_for_shell(shell));
}

#[tokio::main]
async fn main() -> Result<()> {
    // Handle CLI flags before TUI init.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "--version") {
        println!("lark {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "--print-alias") {
        let shell = args.get(2).map_or("zsh", String::as_str);
        print_alias(shell);
        return Ok(());
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_zsh_contains_key_components() {
        let output = alias_for_shell("zsh");
        assert!(output.contains("bindkey"), "zsh alias should use bindkey");
        assert!(output.contains("^L"), "zsh alias should bind Ctrl+L");
        assert!(
            output.contains("lark-widget"),
            "zsh alias should define widget"
        );
    }

    #[test]
    fn alias_unknown_shell_defaults_to_zsh() {
        let zsh = alias_for_shell("zsh");
        let unknown = alias_for_shell("unknown-shell");
        assert_eq!(zsh, unknown, "unknown shell should default to zsh output");
    }

    #[test]
    fn alias_bash_contains_bind() {
        let output = alias_for_shell("bash");
        assert!(output.contains("bind"), "bash alias should use bind");
        assert!(output.contains(r"\C-l"), "bash alias should bind Ctrl+L");
    }

    #[test]
    fn alias_fish_contains_bind() {
        let output = alias_for_shell("fish");
        assert!(output.contains("bind"), "fish alias should use bind");
        assert!(output.contains(r"\cl"), "fish alias should bind Ctrl+L");
        assert!(
            output.contains("commandline -f repaint"),
            "fish alias should repaint"
        );
    }
}
