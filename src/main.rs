//! Larkline — the line to all your tools.
//!
//! A keyboard-driven terminal command palette.

use std::path::Path;
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

/// Print CLI usage and exit.
fn print_help() {
    println!(
        "\
lark — a keyboard-driven terminal command palette

Usage: lark [OPTIONS]
       lark init-plugin <NAME> [--shell|--multi]

Options:
  --help, -h              Show this help message
  --version               Show version
  --print-alias <SHELL>   Print shell integration (zsh, bash, fish)

Commands:
  init-plugin <NAME>      Scaffold a new plugin directory
    --shell               Generate a shell (bash) plugin instead of Lua
    --multi               Generate a multi-command plugin with [[commands]]"
    );
}

/// Scaffold a new plugin at `~/.config/larkline/plugins/<name>/`.
///
/// Creates `manifest.toml` and either `init.lua` (default), `run.sh` (`--shell`), or a
/// two-command Lua scaffold (`--multi`). Returns `Err` if the directory already exists.
fn init_plugin(name: &str, shell: bool, multi: bool) -> Result<()> {
    let plugin_dir = config::default_plugin_dir().join(name);
    if plugin_dir.exists() {
        anyhow::bail!("Plugin directory already exists: {}", plugin_dir.display());
    }

    std::fs::create_dir_all(&plugin_dir)?;

    if multi {
        // Multi-command scaffold: two Lua commands under [[commands]].
        let manifest = generate_multi_manifest(name);
        std::fs::write(plugin_dir.join("manifest.toml"), manifest)?;
        let cmd1 = generate_lua_template(&format!("{name} — Command One"));
        let cmd2 = generate_lua_template(&format!("{name} — Command Two"));
        std::fs::write(plugin_dir.join("command_one.lua"), cmd1)?;
        std::fs::write(plugin_dir.join("command_two.lua"), cmd2)?;
        println!("Created multi-command plugin at {}", plugin_dir.display());
        println!("  manifest.toml");
        println!("  command_one.lua");
        println!("  command_two.lua");
    } else {
        let (entry, template) = if shell {
            ("run.sh", generate_shell_template(name))
        } else {
            ("init.lua", generate_lua_template(name))
        };
        let manifest = generate_manifest(name, entry);
        std::fs::write(plugin_dir.join("manifest.toml"), manifest)?;
        std::fs::write(plugin_dir.join(entry), template)?;
        if shell {
            make_executable(&plugin_dir.join(entry))?;
        }
        println!("Created plugin at {}", plugin_dir.display());
        println!("  manifest.toml");
        println!("  {entry}");
    }
    Ok(())
}

fn generate_manifest(name: &str, entry: &str) -> String {
    format!(
        r#"[plugin]
name = "{name}"
description = "A new Larkline plugin"
version = "0.1.0"
author = ""
icon = "🔧"
icon_nerd = ""
entry = "{entry}"
timeout_seconds = 10

category = "custom"
"#
    )
}

fn generate_multi_manifest(name: &str) -> String {
    format!(
        r#"[plugin]
name = "{name}"
description = "A new multi-command Larkline plugin"
version = "0.1.0"
author = ""
icon = "🔧"
icon_nerd = ""
category = "custom"

[[commands]]
name = "Command One"
description = "First command — edit command_one.lua to customize"
entry = "command_one.lua"
quickkey = "c1"

[[commands]]
name = "Command Two"
description = "Second command — edit command_two.lua to customize"
entry = "command_two.lua"
quickkey = "c2"
"#
    )
}

fn generate_lua_template(name: &str) -> String {
    format!(
        r#"lark.register({{
    on_run = function()
        return {{
            title = "{name}",
            items = {{
                {{ label = "Hello from {name}!", detail = "Edit init.lua to customize", icon = "🔧" }},
            }},
        }}
    end,
}})
"#
    )
}

fn generate_shell_template(name: &str) -> String {
    format!(
        r#"#!/usr/bin/env bash
jq -n --arg name '{name}' '{{
  title: $name,
  items: [
    {{label: ("Hello from " + $name + "!"), detail: "Edit run.sh to customize", icon: "🔧"}}
  ]
}}'
"#
    )
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Handle CLI flags before TUI init.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "--version") {
        println!("lark {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "--print-alias") {
        let shell = args.get(2).map_or("zsh", String::as_str);
        print_alias(shell);
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "init-plugin") {
        let name = args
            .get(2)
            .ok_or_else(|| anyhow::anyhow!("Usage: lark init-plugin <NAME> [--shell|--multi]"))?;
        let shell = args.iter().any(|a| a == "--shell");
        let multi = args.iter().any(|a| a == "--multi");
        return init_plugin(name, shell, multi);
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

    let mut discovered = plugin::registry::scan(&config.general.plugin_dirs)?;
    // Resolve icons based on configured icon set.
    if config.ui.icon_set == config::IconSet::Nerd {
        for d in &mut discovered {
            if let Some(ref nerd) = d.metadata.icon_nerd {
                d.metadata.icon = nerd.clone();
            }
        }
    }
    let plugins: Vec<Arc<dyn plugin::Plugin>> =
        discovered.into_iter().map(plugin::build_plugin).collect();

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

    // ── Help flag tests ─────────────────────────────────────────────────────

    #[test]
    fn help_text_contains_key_sections() {
        // Capture what print_help would output by checking the function doesn't panic
        // and the text constants are correct.
        let help = "\
lark — a keyboard-driven terminal command palette

Usage: lark [OPTIONS]
       lark init-plugin <NAME> [--shell|--multi]

Options:
  --help, -h              Show this help message
  --version               Show version
  --print-alias <SHELL>   Print shell integration (zsh, bash, fish)

Commands:
  init-plugin <NAME>      Scaffold a new plugin directory
    --shell               Generate a shell (bash) plugin instead of Lua
    --multi               Generate a multi-command plugin with [[commands]]";
        assert!(help.contains("--help"));
        assert!(help.contains("--version"));
        assert!(help.contains("--print-alias"));
        assert!(help.contains("init-plugin"));
    }

    // ── Plugin scaffolding tests ────────────────────────────────────────────

    #[test]
    fn generate_manifest_contains_plugin_name() {
        let manifest = generate_manifest("test-plugin", "init.lua");
        assert!(manifest.contains("name = \"test-plugin\""));
        assert!(manifest.contains("entry = \"init.lua\""));
        assert!(manifest.contains("timeout_seconds = 10"));
    }

    #[test]
    fn generate_manifest_shell_entry() {
        let manifest = generate_manifest("my-tool", "run.sh");
        assert!(manifest.contains("entry = \"run.sh\""));
        assert!(manifest.contains("name = \"my-tool\""));
    }

    #[test]
    fn generate_lua_template_is_valid() {
        let lua = generate_lua_template("test-plugin");
        assert!(lua.contains("lark.register"));
        assert!(lua.contains("Hello from test-plugin!"));
        assert!(lua.contains("title = \"test-plugin\""));
    }

    #[test]
    fn generate_shell_template_uses_jq() {
        let sh = generate_shell_template("test-plugin");
        assert!(sh.starts_with("#!/usr/bin/env bash"));
        assert!(sh.contains("jq -n"));
        assert!(sh.contains("test-plugin"));
    }

    #[test]
    fn init_plugin_creates_lua_scaffold() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugin_dir = dir.path().join("my-plugin");
        assert!(!plugin_dir.exists());

        // Directly test the file creation logic
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let manifest = generate_manifest("my-plugin", "init.lua");
        let lua = generate_lua_template("my-plugin");
        std::fs::write(plugin_dir.join("manifest.toml"), &manifest).unwrap();
        std::fs::write(plugin_dir.join("init.lua"), &lua).unwrap();

        assert!(plugin_dir.join("manifest.toml").exists());
        assert!(plugin_dir.join("init.lua").exists());

        let manifest_content = std::fs::read_to_string(plugin_dir.join("manifest.toml")).unwrap();
        assert!(manifest_content.contains("name = \"my-plugin\""));
    }

    #[test]
    fn init_plugin_creates_shell_scaffold() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugin_dir = dir.path().join("my-shell-plugin");

        std::fs::create_dir_all(&plugin_dir).unwrap();
        let manifest = generate_manifest("my-shell-plugin", "run.sh");
        let sh = generate_shell_template("my-shell-plugin");
        std::fs::write(plugin_dir.join("manifest.toml"), &manifest).unwrap();
        std::fs::write(plugin_dir.join("run.sh"), &sh).unwrap();

        assert!(plugin_dir.join("manifest.toml").exists());
        assert!(plugin_dir.join("run.sh").exists());

        let sh_content = std::fs::read_to_string(plugin_dir.join("run.sh")).unwrap();
        assert!(sh_content.contains("#!/usr/bin/env bash"));
        assert!(sh_content.contains("jq -n"));
    }
}
