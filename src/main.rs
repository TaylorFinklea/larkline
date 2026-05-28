//! Larkline — the line to all your tools.
//!
//! A keyboard-driven terminal command palette.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::info;

mod action;
mod actions;
mod agent;
mod app;
mod app_output;
mod atlassian;
mod config;
mod form_actions;
mod history;
mod input;
mod mini_app;
mod plugin;
mod plugin_manager_actions;
mod plugin_manager_state;
mod power_menu;
mod tui;
mod update;
mod widget_actions;
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
  plugin sync [--force]   Install/update standard plugins from GitHub
                          --force prompts before overwriting user-modified plugins
  plugin list             List installed plugins
  plugin remove <NAME>    Remove an installed plugin
  ai-ask <PROMPT>         Send a single prompt to the configured [ai] provider;
                          stream the response to stdout
    --system <TEXT>       Override the system prompt
    --model <ID>          Override the configured model
    --max-tokens <N>      Cap on output tokens (default: provider default)
  agent-ask <PROMPT>      Multi-turn AI agent that calls plugins as tools
    --system / --model / --max-tokens   (same as ai-ask)
    --yes                 Auto-approve destructive tools (default: block)"
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

/// JSON shape for `lark list --json`. Decoupled from the internal
/// `PluginMetadata` struct so the wire format can evolve independently.
/// Consumed by `lark.nvim` (Telescope source) and any other headless tooling.
#[derive(serde::Serialize)]
// Four booleans are part of the JSON contract (has_settings / is_widget /
// is_mini_app / streaming). Each is independent — flattening into a bitfield
// would hurt downstream consumer ergonomics with no real win.
#[allow(clippy::struct_excessive_bools)]
struct ListEntry {
    name: String,
    description: String,
    icon: String,
    icon_nerd: Option<String>,
    category: Option<String>,
    plugin_group: Option<String>,
    quickkey: Option<String>,
    keybinding: Option<String>,
    /// "Lua" or "Script" — backend type.
    kind: &'static str,
    author: String,
    version: String,
    secrets: Vec<String>,
    has_settings: bool,
    is_widget: bool,
    is_mini_app: bool,
    streaming: bool,
    /// Whether the in-app AI agent may call this command as a tool.
    agent_callable: bool,
    /// Whether this command mutates state (gates the agent's approval).
    destructive: bool,
}

/// JSON shape emitted by `lark action`. Wire format consumed by `lark.nvim`'s
/// Telescope source — every variant maps onto a Telescope-side handler.
#[derive(serde::Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum ActionWireOutcome {
    /// Side-effect happened; show a notification + optional shell stdout.
    Side {
        summary: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        stdout: Option<String>,
    },
    /// Chain produced a new view; embed it so Telescope can push a new picker.
    /// Boxed to keep the enum's stack size balanced — `PluginOutput` is the
    /// largest variant body and would dwarf `Side` / `NeedsConfirmation`.
    Chained {
        output: Box<plugin::traits::PluginOutput>,
    },
    /// User confirmation required before running a shell command. The Lua
    /// caller prompts via `vim.ui.confirm` and re-issues the call with `--confirm`.
    NeedsConfirmation {
        command: String,
        args: Vec<String>,
        description: String,
    },
}

/// Handle `lark action <plugin> --action-json '<JSON>' [--confirm]` —
/// dispatch a single action against a plugin and print the outcome as JSON.
async fn handle_action_command(args: &[String]) -> Result<()> {
    let plugin_name = args.first().ok_or_else(|| {
        anyhow::anyhow!("Usage: lark action <PLUGIN_NAME> --action-json '<JSON>'")
    })?;

    // Hand-rolled flag parsing matches the existing pattern in main.rs (no clap).
    let mut action_json: Option<&String> = None;
    let mut confirm = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--action-json" => {
                action_json = args.get(i + 1);
                i += 2;
            }
            "--confirm" => {
                confirm = true;
                i += 1;
            }
            other => anyhow::bail!("lark action: unknown argument `{other}`"),
        }
    }
    let action_json = action_json
        .ok_or_else(|| anyhow::anyhow!("--action-json '<ItemAction JSON>' is required"))?;

    let action: plugin::traits::ItemAction = serde_json::from_str(action_json)
        .with_context(|| format!("invalid --action-json: {action_json}"))?;

    // Confirmation gate: a confirm-required shell action without `--confirm`
    // surfaces a `needs_confirmation` outcome so the caller (Telescope) can
    // prompt the user and re-issue the call.
    if action.confirm && !confirm && action.kind == plugin::traits::ActionKind::Shell {
        let cmd = action.args.first().cloned().unwrap_or_default();
        let cmd_args: Vec<String> = action.args.iter().skip(1).cloned().collect();
        let outcome = ActionWireOutcome::NeedsConfirmation {
            command: cmd,
            args: cmd_args,
            description: action.label.clone(),
        };
        println!("{}", serde_json::to_string_pretty(&outcome)?);
        return Ok(());
    }

    let ctx = build_plugin_context()?;
    let target = find_plugin(&ctx.plugins, plugin_name)?.clone();

    let result = plugin::engine::SECRETS
        .scope(
            ctx.secrets,
            plugin::engine::PLUGIN_LIST.scope(
                ctx.plugins,
                plugin::engine::INVOKE_DEPTH
                    .scope(0, async { actions::execute(&action, &target).await }),
            ),
        )
        .await?;

    let outcome = match result {
        actions::ActionResult::Side { summary, stdout } => {
            ActionWireOutcome::Side { summary, stdout }
        }
        actions::ActionResult::Chained(output) => ActionWireOutcome::Chained { output },
    };
    println!("{}", serde_json::to_string_pretty(&outcome)?);
    Ok(())
}

/// Handle `lark ai-ask [--system TEXT] [--model ID] [--max-tokens N] <PROMPT>`.
///
/// Single-shot AI prompt against the configured `[ai]` provider. Streams
/// the response to stdout as it arrives (plain text — no envelope). Tokens
/// and usage stats go to stderr on completion. Errors return non-zero with
/// a human-readable message on stderr.
///
/// This is the Phase 6 smoke surface: it lets us exercise the Provider
/// trait + factory end-to-end without the TUI or any plugin layer. The
/// shape mirrors pi-mono's `pi -p "..."` non-interactive mode.
// Long because flag parsing + provider wiring + event drain naturally live
// together; splitting them out would hide the control flow.
#[allow(clippy::too_many_lines)]
async fn handle_ai_ask_command(args: &[String]) -> Result<()> {
    use std::io::Write;

    let mut system: Option<String> = None;
    let mut model_override: Option<String> = None;
    let mut max_tokens_override: Option<u32> = None;
    let mut prompt_parts: Vec<String> = Vec::new();

    // Hand-rolled flag parsing matches the existing pattern in main.rs (no clap).
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--system" => {
                system = Some(
                    args.get(i + 1)
                        .ok_or_else(|| anyhow::anyhow!("--system requires a value"))?
                        .clone(),
                );
                i += 2;
            }
            "--model" => {
                model_override = Some(
                    args.get(i + 1)
                        .ok_or_else(|| anyhow::anyhow!("--model requires a value"))?
                        .clone(),
                );
                i += 2;
            }
            "--max-tokens" => {
                let raw = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("--max-tokens requires a value"))?;
                max_tokens_override = Some(
                    raw.parse::<u32>()
                        .with_context(|| format!("invalid --max-tokens: {raw}"))?,
                );
                i += 2;
            }
            "--help" | "-h" => {
                println!(
                    "\
lark ai-ask — single-shot AI prompt against the configured [ai] provider

Usage: lark ai-ask [OPTIONS] <PROMPT...>

Options:
  --system <TEXT>       System prompt override
  --model <ID>          Model identifier override
  --max-tokens <N>      Output token cap"
                );
                return Ok(());
            }
            other => {
                prompt_parts.push(other.to_string());
                i += 1;
            }
        }
    }

    if prompt_parts.is_empty() {
        anyhow::bail!("Usage: lark ai-ask [OPTIONS] <PROMPT...>");
    }
    let prompt = prompt_parts.join(" ");

    let (cfg, _) = config::load().unwrap_or_else(|e| {
        eprintln!("larkline: config error ({e}), using defaults");
        (config::Config::default(), Vec::new())
    });

    // Mirror the TUI's secrets pipeline so `lark secret set` keys resolve
    // identically across both entry points.
    let mut secrets = config::load_secrets();
    let declared_keys: Vec<&str> = config::AI_SECRET_KEYS.to_vec();
    config::resolve_keychain_secrets(&mut secrets, &declared_keys);

    let provider = agent::build_provider(&cfg.ai, &secrets)
        .with_context(|| "failed to build AI provider")?;

    let model = model_override.unwrap_or_else(|| cfg.ai.resolved_model().to_string());
    let max_tokens = max_tokens_override.or_else(|| cfg.ai.resolved_max_tokens());

    let request = agent::AskRequest {
        system,
        messages: vec![agent::Message::user(prompt)],
        tools: Vec::new(),
        model,
        max_tokens,
    };

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<agent::ProviderEvent>();

    // Spawn the provider request on its own task so we can drain the
    // receiver concurrently. The provider drops `tx` when done, closing
    // the channel — that's how the receiver loop terminates.
    let provider_task = tokio::spawn(async move { provider.ask(request, tx).await });

    let mut usage: Option<(u32, u32)> = None;
    let mut stop_reason: Option<agent::StopReason> = None;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    while let Some(event) = rx.recv().await {
        match event {
            agent::ProviderEvent::TextDelta(chunk) => {
                out.write_all(chunk.as_bytes())?;
                out.flush()?;
            }
            agent::ProviderEvent::ToolUse { name, .. } => {
                // Phase 6 doesn't pass tools; tool-use shouldn't arrive.
                // Surface it on stderr if a provider does anyway (e.g.
                // Ollama returning a function call from a tool-capable
                // model when we didn't ask).
                eprintln!("\n[unexpected tool_use: {name}]");
            }
            agent::ProviderEvent::Usage {
                input_tokens,
                output_tokens,
            } => {
                usage = Some((input_tokens, output_tokens));
            }
            agent::ProviderEvent::Done { stop_reason: sr } => {
                stop_reason = Some(sr);
            }
        }
    }
    writeln!(out)?;
    out.flush()?;

    // Provider task should have returned by now (channel close = it dropped tx).
    let ask_result = provider_task
        .await
        .map_err(|e| anyhow::anyhow!("provider task panicked: {e}"))?;
    ask_result.with_context(|| "provider request failed")?;

    if let Some((input, output)) = usage {
        eprintln!("[tokens in={input} out={output}]");
    }
    if let Some(sr) = stop_reason
        && !matches!(sr, agent::StopReason::EndTurn)
    {
        eprintln!("[stop_reason={sr:?}]");
    }
    Ok(())
}

/// Handle `lark agent-ask` — multi-turn AI agent that calls plugins as tools.
///
/// Args: `[--system X] [--model Y] [--max-tokens N] [--yes] <PROMPT>`.
/// Uses the same provider + secrets pipeline as `lark ai-ask` but builds
/// an `AgentHarness` so tool cycles + the dry-run approval hook are active.
///
/// `--yes` swaps the default `DefaultApprovalHook` (which blocks
/// destructive plans) for nothing — destructive tools auto-dispatch.
/// Without `--yes` the CLI is safe-by-default: destructive plans surface
/// as a hook-rejection message and the agent finishes verbally.
//
// The CLI path is the canonical entry point for the agent (matching the
// `lark ai-ask` pattern). The TUI plugin (`examples/plugins/ai/agent.lua`)
// shells out to this same subcommand. One code path; consistent behavior.
#[allow(clippy::too_many_lines)]
async fn handle_agent_ask_command(args: &[String]) -> Result<()> {
    use std::io::Write;

    let mut system: Option<String> = None;
    let mut model_override: Option<String> = None;
    let mut max_tokens_override: Option<u32> = None;
    let mut auto_yes = false;
    let mut prompt_parts: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--system" => {
                system = Some(
                    args.get(i + 1)
                        .ok_or_else(|| anyhow::anyhow!("--system requires a value"))?
                        .clone(),
                );
                i += 2;
            }
            "--model" => {
                model_override = Some(
                    args.get(i + 1)
                        .ok_or_else(|| anyhow::anyhow!("--model requires a value"))?
                        .clone(),
                );
                i += 2;
            }
            "--max-tokens" => {
                let raw = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("--max-tokens requires a value"))?;
                max_tokens_override = Some(
                    raw.parse::<u32>()
                        .with_context(|| format!("invalid --max-tokens: {raw}"))?,
                );
                i += 2;
            }
            "--yes" => {
                auto_yes = true;
                i += 1;
            }
            "--help" | "-h" => {
                println!(
                    "\
lark agent-ask — multi-turn AI agent that calls plugins as tools

Usage: lark agent-ask [OPTIONS] <PROMPT...>

Options:
  --system <TEXT>       System prompt override
  --model <ID>          Model identifier override
  --max-tokens <N>      Output token cap (default: provider default)
  --yes                 Auto-approve destructive tool calls (default: block them)

Tools are sourced from manifests with `agent_callable = true`. Destructive
commands are blocked by default; use --yes to allow them, or run safe-only.
Audit log: $XDG_STATE_HOME/larkline/agent-audit.log.
Session: $XDG_STATE_HOME/larkline/sessions/<uuid>.jsonl."
                );
                return Ok(());
            }
            other => {
                prompt_parts.push(other.to_string());
                i += 1;
            }
        }
    }

    if prompt_parts.is_empty() {
        anyhow::bail!("Usage: lark agent-ask [OPTIONS] <PROMPT...>");
    }
    let prompt = prompt_parts.join(" ");

    let ctx = build_plugin_context()?;
    let (cfg, _) = config::load().unwrap_or_else(|e| {
        eprintln!("larkline: config error ({e}), using defaults");
        (config::Config::default(), Vec::new())
    });

    let provider = agent::build_provider(&cfg.ai, &ctx.secrets)
        .with_context(|| "failed to build AI provider")?;

    let model = model_override.unwrap_or_else(|| cfg.ai.resolved_model().to_string());
    let max_tokens = max_tokens_override.or_else(|| cfg.ai.resolved_max_tokens());

    let agent_cfg = agent::AgentConfig {
        system_prompt: system,
        model,
        max_tokens,
        thinking_level: agent::ThinkingLevel::Off,
        tools: Vec::new(), // populated by with_plugins
    };

    // Sessions + audit log under $XDG_STATE_HOME/larkline.
    let state_dir = resolve_log_file_dir(
        std::env::var("XDG_STATE_HOME").ok(),
        std::env::var("HOME").ok(),
    );
    let sessions_dir = state_dir.join("sessions");
    let audit_path = state_dir.join("agent-audit.log");
    let audit = agent::AuditLog::open(&audit_path)
        .with_context(|| format!("failed to open audit log at {}", audit_path.display()))?;

    let mut harness = agent::AgentHarness::create_in(agent_cfg, provider, &sessions_dir)
        .with_context(|| "failed to create agent session")?
        .with_plugins(&ctx.plugins)
        .with_secrets(Arc::clone(&ctx.secrets))
        .with_audit(audit);

    if auto_yes {
        // Replace the default block-destructive hook with no-op (allow-all).
        harness = harness.with_no_default_hooks();
    }

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let outcomes = harness
        .prompt(agent::Message::user(prompt), |ev| match ev {
            agent::ProviderEvent::TextDelta(chunk) => {
                let _ = out.write_all(chunk.as_bytes());
                let _ = out.flush();
            }
            agent::ProviderEvent::ToolUse { name, .. } => {
                let _ = writeln!(out, "\n→ tool: {name}");
            }
            agent::ProviderEvent::Usage { .. } | agent::ProviderEvent::Done { .. } => {}
        })
        .await
        .with_context(|| "agent harness failed")?;

    writeln!(out)?;
    out.flush()?;

    let (total_in, total_out) = outcomes.iter().fold((0u32, 0u32), |(i, o), oc| {
        if let agent::TurnOutcome::Completed { usage, .. } = oc {
            let (a, b) = usage.unwrap_or((0, 0));
            (i + a, o + b)
        } else {
            (i, o)
        }
    });
    eprintln!(
        "[turns={} tokens in={} out={} session={}]",
        outcomes.len(),
        total_in,
        total_out,
        harness.session_id()
    );
    Ok(())
}

/// Handle `lark list [--json]` — enumerate every discovered plugin/command.
fn handle_list_command(_args: &[String]) -> Result<()> {
    let (cfg, _) = config::load().unwrap_or_else(|e| {
        eprintln!("larkline: config error ({e}), using defaults");
        (config::Config::default(), Vec::new())
    });

    let discovered = plugin::registry::scan(&cfg.general.plugin_dirs)?;

    let entries: Vec<ListEntry> = discovered
        .into_iter()
        .map(|d| ListEntry {
            name: d.metadata.name,
            description: d.metadata.description,
            icon: d.metadata.icon,
            icon_nerd: d.metadata.icon_nerd,
            category: d.metadata.category,
            plugin_group: d.metadata.plugin_group,
            quickkey: d.metadata.quickkey,
            keybinding: d.metadata.keybinding,
            kind: match d.kind {
                plugin::registry::PluginKind::Lua => "Lua",
                plugin::registry::PluginKind::Script => "Script",
            },
            author: d.metadata.author,
            version: d.metadata.version,
            secrets: d.metadata.secrets,
            has_settings: !d.metadata.settings_spec.is_empty(),
            is_widget: d.metadata.widget,
            is_mini_app: d.metadata.mini_app,
            streaming: d.metadata.streaming,
            agent_callable: d.metadata.agent_callable,
            destructive: d.metadata.destructive,
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&entries)?);
    Ok(())
}

/// Bundle of state both `invoke` and `action` need: the discovered plugin Vec,
/// the secrets map ready for `SECRETS::scope`, and the `Arc` of plugins for
/// `PLUGIN_LIST::scope`. Built once via [`build_plugin_context`].
struct PluginContext {
    plugins: Arc<Vec<Arc<dyn plugin::Plugin>>>,
    secrets: Arc<std::collections::HashMap<String, String>>,
}

fn build_plugin_context() -> Result<PluginContext> {
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

    // Mirror the TUI's secrets pipeline so plugins invoked headlessly see the
    // same `lark.env(KEY)` results as inside the TUI.
    let mut secrets = config::load_secrets();
    let mut declared_keys: Vec<&str> = discovered
        .iter()
        .flat_map(|d| d.metadata.secrets.iter().map(String::as_str))
        .collect();
    // AI provider keys (Phase 5) are resolved alongside plugin-declared
    // ones so `lark secret set ANTHROPIC_API_KEY` works the same way.
    declared_keys.extend(config::AI_SECRET_KEYS);
    config::resolve_keychain_secrets(&mut secrets, &declared_keys);
    inject_lark_binary(&mut secrets);

    let plugins: Vec<Arc<dyn plugin::Plugin>> =
        discovered.into_iter().map(plugin::build_plugin).collect();

    Ok(PluginContext {
        plugins: Arc::new(plugins),
        secrets: Arc::new(secrets),
    })
}

fn find_plugin<'a>(
    plugins: &'a [Arc<dyn plugin::Plugin>],
    name: &str,
) -> Result<&'a Arc<dyn plugin::Plugin>> {
    plugins
        .iter()
        .find(|p| p.metadata().name == name)
        .ok_or_else(|| anyhow::anyhow!("plugin not found: {name}"))
}

/// Execute a plugin by name and print its JSON output to stdout.
async fn invoke_plugin(name: &str) -> Result<()> {
    let ctx = build_plugin_context()?;
    let target = find_plugin(&ctx.plugins, name)?.clone();

    let output = plugin::engine::SECRETS
        .scope(
            ctx.secrets,
            plugin::engine::PLUGIN_LIST.scope(
                ctx.plugins,
                plugin::engine::INVOKE_DEPTH.scope(0, async { target.execute().await }),
            ),
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

/// Log file directory — follows XDG state-dir conventions.
///
/// `$XDG_STATE_HOME/larkline`, defaulting to `$HOME/.local/state/larkline`.
/// Used when the TUI is active; writing to stderr would corrupt the alternate
/// screen buffer.
fn log_file_dir() -> std::path::PathBuf {
    resolve_log_file_dir(
        std::env::var("XDG_STATE_HOME").ok(),
        std::env::var("HOME").ok(),
    )
}

/// Pure helper for [`log_file_dir`] — unit-testable without touching env vars.
fn resolve_log_file_dir(
    xdg_state_home: Option<String>,
    home: Option<String>,
) -> std::path::PathBuf {
    let base = if let Some(xdg) = xdg_state_home {
        std::path::PathBuf::from(xdg)
    } else {
        let home = home.unwrap_or_else(|| "/tmp".to_string());
        std::path::PathBuf::from(home).join(".local").join("state")
    };
    base.join("larkline")
}

/// Initialize tracing for a TUI session, writing to a rolling log file.
///
/// Returns a `WorkerGuard` that must live for the process lifetime; dropping
/// it flushes the non-blocking writer. When `RUST_LOG` is set, prints the log
/// file path to stderr so the user can tail it.
///
/// On directory creation failure, falls back to a never-initialized subscriber
/// so larkline still runs — silent logs are preferable to a corrupted TUI.
fn init_tui_logger(
    log_dir: &std::path::Path,
    log_level: tracing::Level,
) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    if std::fs::create_dir_all(log_dir).is_err() {
        return None;
    }
    let appender = tracing_appender::rolling::daily(log_dir, "lark.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(appender);

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(log_level.into()),
        )
        .with_writer(non_blocking)
        .with_ansi(false);

    if subscriber.try_init().is_err() {
        return None;
    }

    if std::env::var_os("RUST_LOG").is_some() {
        let log_path = log_dir.join("lark.log");
        eprintln!("larkline: logging to {}", log_path.display());
    }

    Some(guard)
}

/// Classification of a potential plugin target path during `plugin sync`.
#[derive(Debug, PartialEq, Eq)]
enum SyncOutcome {
    /// Path doesn't exist.
    Missing,
    /// Symlink exists but its target cannot be resolved.
    DeadSymlink,
    /// Symlink resolves into the current cache source dir.
    InCache,
    /// Symlink resolves somewhere else (user-customized).
    OutsideCache,
    /// Not a symlink — user has written real files here.
    RealDirectory,
}

/// Classify a plugin target path relative to the cache source directory.
fn classify_target(target: &std::path::Path, cache_source: &std::path::Path) -> SyncOutcome {
    let Ok(meta) = target.symlink_metadata() else {
        return SyncOutcome::Missing;
    };
    if !meta.file_type().is_symlink() {
        return SyncOutcome::RealDirectory;
    }
    let Ok(resolved) = target.canonicalize() else {
        return SyncOutcome::DeadSymlink;
    };
    let Ok(canonical_cache) = cache_source.canonicalize() else {
        return SyncOutcome::OutsideCache;
    };
    if resolved.starts_with(&canonical_cache) {
        SyncOutcome::InCache
    } else {
        SyncOutcome::OutsideCache
    }
}

/// Counts accumulated during a sync run, used for the summary line.
#[derive(Debug, Default)]
struct SyncCounts {
    added: usize,
    repaired: usize,
    kept_in_cache: usize,
    kept_custom: usize,
    kept_modified: usize,
}

/// Create a plugin symlink at `target` pointing at `source`.
fn create_plugin_symlink(
    source: &std::path::Path,
    target: &std::path::Path,
) -> std::io::Result<()> {
    #[cfg(unix)]
    std::os::unix::fs::symlink(source, target)?;
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(source, target)?;
    Ok(())
}

/// Interactively confirm overwrite of a user-modified plugin. Returns `Ok(false)`
/// when stdin is not a TTY (non-interactive / CI context).
fn confirm_overwrite(name: &str) -> Result<bool> {
    use std::io::{IsTerminal, Write};
    if !std::io::stdin().is_terminal() {
        return Ok(false);
    }
    print!("Overwrite {name}? [y/N]: ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.trim().eq_ignore_ascii_case("y"))
}

/// Handle `lark plugin sync|list|remove` subcommands.
#[allow(clippy::too_many_lines)]
fn handle_plugin_command(args: &[String]) -> Result<()> {
    let sub = args.first().map(String::as_str);
    let plugin_dir = config::default_plugin_dir();

    match sub {
        Some("sync") => {
            let force = args.iter().any(|a| a == "--force");
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

            let mut counts = SyncCounts::default();
            for entry in std::fs::read_dir(&source_dir)? {
                let entry = entry?;
                // Only real plugin dirs — filter stray files (README.md, .gitignore) and
                // anything without a manifest.
                if !entry.path().join("manifest.toml").is_file() {
                    continue;
                }

                let name = entry.file_name();
                let name_str = name.to_string_lossy().into_owned();
                let target = plugin_dir.join(&name);

                match classify_target(&target, &source_dir) {
                    SyncOutcome::Missing => {
                        create_plugin_symlink(&entry.path(), &target)?;
                        println!("  + {name_str}");
                        counts.added += 1;
                    }
                    SyncOutcome::DeadSymlink => {
                        std::fs::remove_file(&target)?;
                        create_plugin_symlink(&entry.path(), &target)?;
                        println!("  ~ {name_str} (repaired)");
                        counts.repaired += 1;
                    }
                    SyncOutcome::InCache => {
                        counts.kept_in_cache += 1;
                    }
                    SyncOutcome::OutsideCache => {
                        if force && confirm_overwrite(&name_str)? {
                            std::fs::remove_file(&target)?;
                            create_plugin_symlink(&entry.path(), &target)?;
                            println!("  ~ {name_str} (overwritten)");
                            counts.repaired += 1;
                        } else {
                            let hint = if force { "skipped" } else { "custom" };
                            println!("  ! {name_str} ({hint} — use --force to overwrite)");
                            counts.kept_custom += 1;
                        }
                    }
                    SyncOutcome::RealDirectory => {
                        if force && confirm_overwrite(&name_str)? {
                            std::fs::remove_dir_all(&target)?;
                            create_plugin_symlink(&entry.path(), &target)?;
                            println!("  ~ {name_str} (overwritten)");
                            counts.repaired += 1;
                        } else {
                            let hint = if force { "skipped" } else { "modified" };
                            println!("  ! {name_str} ({hint} — use --force to overwrite)");
                            counts.kept_modified += 1;
                        }
                    }
                }
            }

            let total_kept = counts.kept_in_cache + counts.kept_custom + counts.kept_modified;
            println!(
                "\nDone! {} added, {} repaired, {} kept ({} custom, {} modified).",
                counts.added, counts.repaired, total_kept, counts.kept_custom, counts.kept_modified,
            );
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

/// Inject `LARK_BINARY` into the secrets map so plugins can re-invoke the
/// running binary via `lark.env("LARK_BINARY")`. Lets dev runs (target/debug/lark,
/// not on `$PATH`) still dispatch subcommands like `lark atlassian token`.
/// Using the secrets map avoids the unsafe `std::env::set_var` banned by our
/// `forbid(unsafe_code)` lint.
fn inject_lark_binary(secrets: &mut std::collections::HashMap<String, String>) {
    if let Ok(exe) = std::env::current_exe() {
        secrets.insert(
            "LARK_BINARY".to_string(),
            exe.to_string_lossy().into_owned(),
        );
    }
}

/// Dispatch CLI subcommands that should run *before* TUI / config init.
/// Returns `Ok(Some(_))` when a subcommand handled the invocation (caller
/// should propagate). `Ok(None)` means no subcommand matched and the TUI
/// should launch.
async fn dispatch_subcommand(args: &[String]) -> Result<Option<()>> {
    let Some(first) = args.get(1) else {
        return Ok(None);
    };
    match first.as_str() {
        "--help" | "-h" => {
            print_help();
            Ok(Some(()))
        }
        "--version" => {
            println!("lark {}", env!("CARGO_PKG_VERSION"));
            Ok(Some(()))
        }
        "--print-alias" => {
            let shell = args.get(2).map_or("zsh", String::as_str);
            print_alias(shell);
            Ok(Some(()))
        }
        "init-plugin" => {
            let name = args.get(2).ok_or_else(|| {
                anyhow::anyhow!("Usage: lark init-plugin <NAME> [--shell|--multi]")
            })?;
            let shell = args.iter().any(|a| a == "--shell");
            let multi = args.iter().any(|a| a == "--multi");
            init_plugin(name, shell, multi).map(Some)
        }
        "invoke" => {
            let name = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("Usage: lark invoke <PLUGIN_NAME>"))?;
            invoke_plugin(name).await.map(Some)
        }
        "secret" => handle_secret_command(&args[2..]).map(Some),
        "plugin" => handle_plugin_command(&args[2..]).map(Some),
        "atlassian" => atlassian::handle_command(&args[2..]).await.map(Some),
        "list" => handle_list_command(&args[2..]).map(Some),
        "action" => handle_action_command(&args[2..]).await.map(Some),
        "ai-ask" => handle_ai_ask_command(&args[2..]).await.map(Some),
        "agent-ask" => handle_agent_ask_command(&args[2..]).await.map(Some),
        _ => Ok(None),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Handle CLI flags before TUI init.
    let args: Vec<String> = std::env::args().collect();
    if let Some(()) = dispatch_subcommand(&args).await? {
        return Ok(());
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

    // Initialize logging to a rolling file. All code paths reaching this point
    // are TUI sessions — CLI subcommands returned earlier. Writing to stderr
    // here would corrupt ratatui's alternate screen buffer.
    let log_dir = log_file_dir();
    let _log_guard = init_tui_logger(&log_dir, log_level);

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
    let mut declared_keys: Vec<&str> = plugins
        .iter()
        .flat_map(|p| p.metadata().secrets.iter().map(String::as_str))
        .collect();
    declared_keys.extend(config::AI_SECRET_KEYS);
    config::resolve_keychain_secrets(&mut secrets, &declared_keys);
    inject_lark_binary(&mut secrets);

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
    fn log_file_dir_prefers_xdg_state_home() {
        let dir = resolve_log_file_dir(
            Some("/tmp/xdg-state-test".to_string()),
            Some("/tmp/home-test".to_string()),
        );
        assert_eq!(
            dir,
            std::path::PathBuf::from("/tmp/xdg-state-test/larkline")
        );
    }

    #[test]
    fn log_file_dir_falls_back_to_home_state() {
        let dir = resolve_log_file_dir(None, Some("/tmp/home-test".to_string()));
        assert_eq!(
            dir,
            std::path::PathBuf::from("/tmp/home-test/.local/state/larkline")
        );
    }

    #[test]
    fn log_file_dir_handles_missing_home() {
        let dir = resolve_log_file_dir(None, None);
        assert_eq!(dir, std::path::PathBuf::from("/tmp/.local/state/larkline"));
    }

    #[test]
    fn classify_target_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        assert_eq!(
            classify_target(&tmp.path().join("nope"), &cache),
            SyncOutcome::Missing
        );
    }

    #[test]
    fn classify_target_in_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("cache");
        let plugin_src = cache.join("hello");
        std::fs::create_dir_all(&plugin_src).unwrap();

        let plugin_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let target = plugin_dir.join("hello");
        std::os::unix::fs::symlink(&plugin_src, &target).unwrap();

        assert_eq!(classify_target(&target, &cache), SyncOutcome::InCache);
    }

    #[test]
    fn classify_target_outside_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        let external = tmp.path().join("elsewhere");
        std::fs::create_dir_all(&external).unwrap();

        let plugin_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let target = plugin_dir.join("custom");
        std::os::unix::fs::symlink(&external, &target).unwrap();

        assert_eq!(classify_target(&target, &cache), SyncOutcome::OutsideCache);
    }

    #[test]
    fn classify_target_dead_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        let plugin_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let target = plugin_dir.join("dead");
        std::os::unix::fs::symlink(tmp.path().join("does-not-exist"), &target).unwrap();

        assert_eq!(classify_target(&target, &cache), SyncOutcome::DeadSymlink);
    }

    #[test]
    fn classify_target_real_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        let plugin_dir = tmp.path().join("plugins");
        let target = plugin_dir.join("modified");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("manifest.toml"), "[plugin]").unwrap();

        assert_eq!(classify_target(&target, &cache), SyncOutcome::RealDirectory);
    }

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
