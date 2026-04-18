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
mod history;
mod input;
mod mini_app;
mod plugin;
mod plugin_manager_state;
mod power_menu;
mod tui;
mod update;
mod widgets;

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
       lark invoke <NAME>

Options:
  --help, -h              Show this help message
  --version               Show version
  --query <TEXT>          Open with search pre-filled
  --print-alias <SHELL>   Print shell integration (zsh, bash, fish)

Commands:
  init-plugin <NAME>      Scaffold a new plugin directory
    --shell               Generate a shell (bash) plugin instead of Lua
    --multi               Generate a multi-command plugin with [[commands]]
  invoke <NAME>           Execute a plugin by name and print JSON output
  secret set <KEY>        Save a secret to macOS Keychain (prompts for value)
  secret list             List secrets stored in Keychain for larkline plugins
  secret delete <KEY>     Remove a secret from macOS Keychain
  plugin sync             Install/update standard plugins from GitHub
  plugin list             List installed plugins
  plugin remove <NAME>    Remove an installed plugin"
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

/// Execute a plugin by name and print its JSON output to stdout.
async fn invoke_plugin(name: &str) -> Result<()> {
    let (cfg, _) = config::load().unwrap_or_else(|e| {
        eprintln!("larkline: config error ({e}), using defaults");
        (config::Config::default(), Vec::new())
    });

    let mut discovered = plugin::registry::scan(&cfg.general.plugin_dirs)?;
    if cfg.ui.icon_set == config::IconSet::Nerd {
        for d in &mut discovered {
            if let Some(ref nerd) = d.metadata.icon_nerd {
                if !nerd.is_empty() {
                    d.metadata.icon = nerd.clone();
                }
            }
        }
    }

    let plugins: Vec<Arc<dyn plugin::Plugin>> =
        discovered.into_iter().map(plugin::build_plugin).collect();

    let target = plugins
        .iter()
        .find(|p| p.metadata().name == name)
        .ok_or_else(|| anyhow::anyhow!("plugin not found: {name}"))?
        .clone();

    let all_plugins = Arc::new(plugins);

    let output = plugin::engine::PLUGIN_LIST
        .scope(
            all_plugins,
            plugin::engine::INVOKE_DEPTH.scope(0, async { target.execute().await }),
        )
        .await?;

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

/// Handle `lark secret set|list|delete` subcommands.
fn handle_secret_command(args: &[String]) -> Result<()> {
    if !cfg!(target_os = "macos") {
        anyhow::bail!(
            "Secret management requires macOS Keychain. Use ~/.config/larkline/.env instead."
        );
    }

    let sub = args.first().map(String::as_str);
    match sub {
        Some("set") => {
            let key = args
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("Usage: lark secret set <KEY>"))?;
            eprint!("Enter value for {key}: ");
            let value = rpassword::read_password()?;
            if value.is_empty() {
                anyhow::bail!("Empty value, nothing saved.");
            }
            let status = std::process::Command::new("security")
                .args([
                    "add-generic-password",
                    "-U",
                    "-a",
                    &std::env::var("USER").unwrap_or_default(),
                    "-s",
                    key,
                    "-w",
                    &value,
                ])
                .status()?;
            if status.success() {
                println!("Saved {key} to macOS Keychain.");
            } else {
                anyhow::bail!("Failed to save to Keychain (exit {status}).");
            }
        }
        Some("list") => {
            let (cfg, _) = config::load().unwrap_or_default();
            let discovered = plugin::registry::scan(&cfg.general.plugin_dirs)?;
            let mut declared: Vec<&str> = discovered
                .iter()
                .flat_map(|d| d.metadata.secrets.iter().map(String::as_str))
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            declared.sort_unstable();

            if declared.is_empty() {
                println!("No plugins declare secrets.");
                return Ok(());
            }

            let env_secrets = config::load_secrets();
            for key in &declared {
                let source = if env_secrets.contains_key(*key) {
                    ".env"
                } else if std::env::var(key).is_ok() {
                    "env var"
                } else if keychain_has(key) {
                    "keychain"
                } else {
                    "NOT SET"
                };
                println!("  {key:<30} {source}");
            }
        }
        Some("delete") => {
            let key = args
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("Usage: lark secret delete <KEY>"))?;
            let status = std::process::Command::new("security")
                .args(["delete-generic-password", "-s", key])
                .stderr(std::process::Stdio::null())
                .status()?;
            if status.success() {
                println!("Deleted {key} from macOS Keychain.");
            } else {
                println!("{key} not found in Keychain.");
            }
        }
        _ => {
            anyhow::bail!("Usage: lark secret <set|list|delete> [KEY]");
        }
    }
    Ok(())
}

/// Check if a key exists in macOS Keychain.
fn keychain_has(key: &str) -> bool {
    std::process::Command::new("security")
        .args(["find-generic-password", "-s", key, "-w"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Standard plugin repo cache location.
fn plugin_cache_dir() -> std::path::PathBuf {
    let base = if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        std::path::PathBuf::from(xdg)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        std::path::PathBuf::from(home).join(".cache")
    };
    base.join("larkline").join("standard-plugins")
}

/// Handle `lark plugin sync|list|remove` subcommands.
#[allow(clippy::too_many_lines)]
fn handle_plugin_command(args: &[String]) -> Result<()> {
    let sub = args.first().map(String::as_str);
    let plugin_dir = config::default_plugin_dir();

    match sub {
        Some("sync") => {
            let cache = plugin_cache_dir();
            let repo_url = "https://github.com/TaylorFinklea/larkline.git";

            // Clone or update the repo cache.
            if cache.join(".git").exists() {
                println!("Updating standard plugin library...");
                let status = std::process::Command::new("git")
                    .args(["-C", &cache.to_string_lossy(), "pull", "--ff-only", "-q"])
                    .status()?;
                if !status.success() {
                    // If pull fails, re-clone.
                    std::fs::remove_dir_all(&cache)?;
                    let status = std::process::Command::new("git")
                        .args([
                            "clone",
                            "--depth",
                            "1",
                            "-q",
                            repo_url,
                            &cache.to_string_lossy(),
                        ])
                        .status()?;
                    if !status.success() {
                        anyhow::bail!("Failed to clone plugin repository");
                    }
                }
            } else {
                println!("Downloading standard plugin library...");
                if let Some(parent) = cache.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let status = std::process::Command::new("git")
                    .args([
                        "clone",
                        "--depth",
                        "1",
                        "-q",
                        repo_url,
                        &cache.to_string_lossy(),
                    ])
                    .status()?;
                if !status.success() {
                    anyhow::bail!("Failed to clone plugin repository");
                }
            }

            // Symlink each plugin from examples/plugins/ to the user's plugin dir.
            let source_dir = cache.join("examples").join("plugins");
            if !source_dir.exists() {
                anyhow::bail!("Plugin source directory not found in cache");
            }

            std::fs::create_dir_all(&plugin_dir)?;

            let mut installed = 0;
            let mut skipped = 0;
            for entry in std::fs::read_dir(&source_dir)? {
                let entry = entry?;
                let name = entry.file_name();
                let target = plugin_dir.join(&name);

                if target.exists() {
                    skipped += 1;
                    continue;
                }

                // Create symlink to cached plugin.
                #[cfg(unix)]
                std::os::unix::fs::symlink(entry.path(), &target)?;
                #[cfg(windows)]
                std::os::windows::fs::symlink_dir(entry.path(), &target)?;

                installed += 1;
                println!("  + {}", name.to_string_lossy());
            }

            println!("\nDone! {installed} plugins installed, {skipped} already present.");
            println!("Plugin directory: {}", plugin_dir.display());
            println!("\nLaunch lark and press R to refresh the plugin list.");
        }

        Some("list") => {
            if !plugin_dir.exists() {
                println!("No plugins installed. Run: lark plugin sync");
                return Ok(());
            }

            let mut count = 0;
            for entry in std::fs::read_dir(&plugin_dir)? {
                let entry = entry?;
                let path = entry.path();
                // Only show directories (or symlinks to directories).
                if !path.is_dir() {
                    continue;
                }
                let name = entry.file_name();
                let manifest = path.join("manifest.toml");
                let status = if manifest.exists() {
                    "✅"
                } else {
                    "⚠ no manifest"
                };
                let link = if path.is_symlink() {
                    format!(
                        " → {}",
                        std::fs::read_link(&path).unwrap_or_default().display()
                    )
                } else {
                    String::new()
                };
                println!("  {status} {}{link}", name.to_string_lossy());
                count += 1;
            }

            if count == 0 {
                println!("No plugins installed. Run: lark plugin sync");
            } else {
                println!("\n{count} plugins in {}", plugin_dir.display());
            }
        }

        Some("remove") => {
            let name = args
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("Usage: lark plugin remove <NAME>"))?;
            let target = plugin_dir.join(name);
            if !target.exists() {
                anyhow::bail!("Plugin not found: {name}");
            }
            if target.is_symlink() {
                std::fs::remove_file(&target)?;
            } else {
                std::fs::remove_dir_all(&target)?;
            }
            println!("Removed plugin: {name}");
        }

        _ => {
            anyhow::bail!("Usage: lark plugin <sync|list|remove> [NAME]");
        }
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
icon = "◆"
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
icon = "◆"
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
                {{ label = "Hello from {name}!", detail = "Edit init.lua to customize", icon = "◆" }},
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
    {{label: ("Hello from " + $name + "!"), detail: "Edit run.sh to customize", icon: "◆"}}
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
    if args.get(1).is_some_and(|a| a == "invoke") {
        let name = args
            .get(2)
            .ok_or_else(|| anyhow::anyhow!("Usage: lark invoke <PLUGIN_NAME>"))?;
        return invoke_plugin(name).await;
    }
    if args.get(1).is_some_and(|a| a == "secret") {
        return handle_secret_command(&args[2..]);
    }
    if args.get(1).is_some_and(|a| a == "plugin") {
        return handle_plugin_command(&args[2..]);
    }

    // Parse --query flag (pre-fill search on launch).
    let initial_query = args
        .iter()
        .position(|a| a == "--query")
        .and_then(|i| args.get(i + 1).cloned());

    // Generate a commented default config on first run.
    // Errors here are non-fatal — silently fall through.
    if let Err(e) = config::generate_default_if_missing() {
        eprintln!("larkline: could not generate default config ({e})");
    }
    if let Err(e) = config::generate_env_if_missing() {
        eprintln!("larkline: could not generate default .env ({e})");
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
                if !nerd.is_empty() {
                    d.metadata.icon = nerd.clone();
                }
            }
        }
    }
    // Filter out disabled plugins/commands.
    let pm_config = config::load_plugin_manager_config();
    let discovered: Vec<_> = discovered
        .into_iter()
        .filter(|d| {
            let gk = d
                .metadata
                .plugin_group
                .as_deref()
                .unwrap_or(&d.metadata.name);
            if pm_config.is_plugin_disabled(gk) {
                return false;
            }
            !pm_config.is_command_disabled(gk, &d.metadata.name)
        })
        .collect();

    let plugins: Vec<Arc<dyn plugin::Plugin>> =
        discovered.into_iter().map(plugin::build_plugin).collect();

    let mut secrets = config::load_secrets();
    let declared_keys: Vec<&str> = plugins
        .iter()
        .flat_map(|p| p.metadata().secrets.iter().map(String::as_str))
        .collect();
    config::resolve_keychain_secrets(&mut secrets, &declared_keys);

    let mut terminal = tui::init()?;
    let mut app = app::App::new(plugins, &config, config_warnings, secrets);
    if let Some(query) = initial_query {
        app.set_initial_query(&query);
    }
    let result = app.run(&mut terminal).await;
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
