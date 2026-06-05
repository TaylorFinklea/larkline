//! First-run / `lark setup` onboarding wizard.
//!
//! A terminal CLI flow (no TUI dependency) that guides a new user through
//! picking a theme, choosing an AI provider + entering its API key. Every
//! choice persists through the same config + Keychain plumbing the rest of the
//! app uses (`config::save_theme_preset`, `config::save_ai_provider`, the
//! `security` Keychain pattern from `lark secret set`), so a future in-TUI
//! onboarding mode can reuse this logic wholesale.

use std::io::{IsTerminal, Write};

use crate::config::{self, AiProviderName};

/// Explicit `lark setup` — always run the full wizard. Errors if not a TTY.
pub fn run_setup() -> anyhow::Result<()> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("`lark setup` needs an interactive terminal.");
    }
    println!("\n🐦 larkline setup\n");
    run_wizard()
}

/// First-run hook (config.toml was just created). Greets, offers to set up,
/// and runs the wizard if the user opts in. Non-interactive or declined just
/// points at `lark setup` and returns so the TUI still launches.
pub fn first_run() -> anyhow::Result<()> {
    if !std::io::stdin().is_terminal() {
        return Ok(());
    }
    println!("\n🐦 Welcome to larkline!\n");
    if prompt_yes_no("Set up your theme + AI provider now?", true)? {
        run_wizard()?;
    } else {
        println!("\nNo problem — run `lark setup` anytime.\n");
    }
    Ok(())
}

fn run_wizard() -> anyhow::Result<()> {
    pick_theme()?;
    pick_provider()?;
    println!("\n✅ All set. Tip: run `lark plugin sync` to install the bundled plugins.\n");
    Ok(())
}

fn pick_theme() -> anyhow::Result<()> {
    println!("Pick a theme:");
    for (i, (_, label)) in config::PRESET_NAMES.iter().enumerate() {
        println!("  {}) {label}", i + 1);
    }
    let choice = prompt_choice("Theme", config::PRESET_NAMES.len(), 1)?;
    let (id, label) = config::PRESET_NAMES[choice - 1];
    config::save_theme_preset(id)?;
    println!("  → theme set to {label}\n");
    Ok(())
}

fn pick_provider() -> anyhow::Result<()> {
    const PROVIDERS: [AiProviderName; 4] = [
        AiProviderName::Anthropic,
        AiProviderName::Openai,
        AiProviderName::Openrouter,
        AiProviderName::Ollama,
    ];
    println!("Pick an AI provider:");
    for (i, p) in PROVIDERS.iter().enumerate() {
        let auth = if p.api_key_env().is_some() {
            "needs an API key"
        } else {
            "no API key (local)"
        };
        println!("  {}) {} — default model {}, {auth}", i + 1, p.as_str(), p.default_model());
    }
    let choice = prompt_choice("Provider", PROVIDERS.len(), 1)?;
    let provider = PROVIDERS[choice - 1];

    // Persist provider; leave model empty so the provider's default_model() applies.
    config::save_ai_provider(provider.as_str(), "")?;
    println!(
        "  → provider set to {} (default model {})",
        provider.as_str(),
        provider.default_model()
    );

    match provider.api_key_env() {
        Some(key_env) => {
            if prompt_yes_no(&format!("Enter your {key_env} now?"), true)? {
                store_api_key(key_env)?;
            } else {
                println!("  → skipped — set it later with `lark secret set {key_env}`");
            }
        }
        None => println!("  → Ollama needs no API key (make sure the local server is running)"),
    }
    println!();
    Ok(())
}

/// Persist an API key: macOS → Keychain (mirrors `lark secret set`), other
/// platforms → a pointer to the `.env` file. Never written to config.toml.
fn store_api_key(key: &str) -> anyhow::Result<()> {
    eprint!("Enter value for {key} (input hidden): ");
    let value = rpassword::read_password()?;
    if value.is_empty() {
        println!("  → empty, nothing saved");
        return Ok(());
    }
    if cfg!(target_os = "macos") {
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
            println!("  → saved {key} to the macOS Keychain");
        } else {
            println!("  → could not save to Keychain (exit {status}); try `lark secret set {key}`");
        }
    } else {
        println!(
            "  → add `{key}=<value>` to {} (it loads first in the secret chain)",
            config::env_path().display()
        );
    }
    Ok(())
}

/// Prompt for a 1-based menu choice; empty input returns `default`. Loops on a
/// bad number, returns `default` on EOF (`read_line` yields empty).
fn prompt_choice(label: &str, count: usize, default: usize) -> anyhow::Result<usize> {
    loop {
        print!("{label} [1-{count}, default {default}]: ");
        std::io::stdout().flush()?;
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input)? == 0 {
            return Ok(default); // EOF
        }
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(default);
        }
        match trimmed.parse::<usize>() {
            Ok(v) if (1..=count).contains(&v) => return Ok(v),
            _ => println!("  please enter a number from 1 to {count}"),
        }
    }
}

/// Yes/no prompt; empty input / EOF returns `default_yes`.
fn prompt_yes_no(question: &str, default_yes: bool) -> anyhow::Result<bool> {
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    print!("{question} {hint}: ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    if std::io::stdin().read_line(&mut input)? == 0 {
        return Ok(default_yes); // EOF
    }
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(default_yes);
    }
    Ok(trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes"))
}
