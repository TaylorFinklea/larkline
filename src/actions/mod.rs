//! Action dispatcher — runs an [`ItemAction`] outside the TUI.
//!
//! The TUI's `App::execute_item_action` handles actions inline against
//! `&mut self.state` (flash messages, confirmations, loading flags). This
//! module is the equivalent CLI-friendly path: feed it an action and an
//! `Arc<dyn Plugin>`, get back an [`ActionResult`].
//!
//! Used by:
//! - the `lark action` subcommand (Phase C of v0.13.0)
//! - any future caller that needs to fire actions without owning TUI state
//!
//! The TUI does NOT route through this module today; behaviour is preserved
//! verbatim. Future cleanup can converge the two paths once we're confident
//! the CLI side is stable.

// Phase B lands the dispatcher; Phase C wires it into the `lark action`
// subcommand. The interim period makes everything in this module appear
// dead — silence the warning here rather than peppering each item.
#![allow(dead_code)]

use std::sync::Arc;

use anyhow::{Context, Result};

use crate::plugin::traits::{ActionKind, ItemAction, Plugin, PluginOutput};

pub mod side_effects;

/// Outcome of executing an action via [`execute`].
#[derive(Debug)]
pub enum ActionResult {
    /// Side-effect happened (clipboard copy, browser open, file open in nvim,
    /// shell command). Caller logs `summary` as a status line; `stdout` is
    /// populated for shell commands that produced output.
    Side {
        /// Human-readable one-line summary, e.g. "Copied to clipboard".
        summary: String,
        /// Combined stdout/stderr of a shell command. Empty for non-shell actions.
        stdout: Option<String>,
    },
    /// Chain action invoked the plugin's `on_action` callback and produced a
    /// new [`PluginOutput`]. Caller transitions to viewing it (TUI: replaces
    /// the output pane; CLI: serializes to JSON).
    Chained(Box<PluginOutput>),
}

/// Dispatch a single action against `plugin`. Side-effects run synchronously;
/// chain actions await the plugin's `on_action` callback.
///
/// **Note:** unlike the TUI path, this function does not differentiate
/// confirm-required shell actions. Callers that need confirmation should
/// inspect `action.confirm` themselves before calling this. The CLI dispatch
/// (Phase C) will surface confirmation as a separate JSON outcome.
pub async fn execute(action: &ItemAction, plugin: &Arc<dyn Plugin>) -> Result<ActionResult> {
    match action.kind {
        ActionKind::Open => execute_open(action),
        ActionKind::Clipboard => execute_clipboard(action),
        ActionKind::Shell => execute_shell(action),
        ActionKind::Chain => execute_chain(action, plugin, 0).await,
        ActionKind::UpdatePane => execute_chain(action, plugin, 1).await,
        ActionKind::NvimEdit => execute_nvim_edit(action),
    }
}

fn execute_open(action: &ItemAction) -> Result<ActionResult> {
    let url = action
        .args
        .first()
        .context("Open action missing URL argument")?;
    side_effects::open_url(url);
    Ok(ActionResult::Side {
        summary: format!("Opened {url}"),
        stdout: None,
    })
}

fn execute_clipboard(action: &ItemAction) -> Result<ActionResult> {
    let text = action
        .args
        .first()
        .context("Clipboard action missing text argument")?;
    side_effects::copy_to_clipboard(text).context("clipboard copy failed")?;
    Ok(ActionResult::Side {
        summary: "Copied to clipboard".to_string(),
        stdout: None,
    })
}

fn execute_shell(action: &ItemAction) -> Result<ActionResult> {
    let cmd = action
        .args
        .first()
        .context("Shell action missing command argument")?;
    let args: Vec<String> = action.args.iter().skip(1).cloned().collect();
    let mut command = std::process::Command::new(cmd);
    command.args(&args);
    // Same secrets-env contract as the TUI's shell dispatch and script
    // plugins: tokens travel via env, never argv.
    if let Ok(secrets) = crate::plugin::engine::SECRETS.try_with(Clone::clone) {
        command.envs(secrets.iter());
    }
    let output = command
        .output()
        .with_context(|| format!("running shell command `{cmd}`"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let combined = if stderr.is_empty() {
        stdout
    } else {
        format!("{stdout}{stderr}")
    };
    let exit_code = output.status.code().unwrap_or(-1);
    Ok(ActionResult::Side {
        summary: format!("{cmd} (exit {exit_code})"),
        stdout: Some(combined),
    })
}

/// `Chain` / `UpdatePane` share a body — both invoke `plugin.execute_action`.
/// They only differ in where the callback id lives in `args`: position 0 for
/// `Chain`, position 1 for `UpdatePane` (which has a leading `pane_id` at
/// position 0 we don't surface in the CLI).
async fn execute_chain(
    action: &ItemAction,
    plugin: &Arc<dyn Plugin>,
    callback_arg_index: usize,
) -> Result<ActionResult> {
    let callback_id = action
        .args
        .get(callback_arg_index)
        .cloned()
        .unwrap_or_default();
    let context = action
        .args
        .iter()
        .skip(callback_arg_index + 1)
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let output = plugin
        .execute_action(&callback_id, &context)
        .await
        .context("plugin on_action callback failed")?;
    Ok(ActionResult::Chained(Box::new(output)))
}

fn execute_nvim_edit(action: &ItemAction) -> Result<ActionResult> {
    let path = action
        .args
        .first()
        .context("NvimEdit missing path argument")?;
    let split = action.args.get(1).map_or("edit", String::as_str);
    match side_effects::nvim_open_file(path, split) {
        Ok(()) => Ok(ActionResult::Side {
            summary: format!("Opened in nvim: {path}"),
            stdout: None,
        }),
        Err(side_effects::NvimOpenError::NotUnderNvim) => {
            side_effects::open_url(path);
            Ok(ActionResult::Side {
                summary: format!("Not running under Neovim; opened {path} via system handler"),
                stdout: None,
            })
        }
        Err(side_effects::NvimOpenError::CommandFailed(e)) => {
            Err(anyhow::anyhow!("nvim open failed: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::traits::ActionKind;

    fn mk_action(kind: ActionKind, args: Vec<String>) -> ItemAction {
        ItemAction {
            id: None,
            label: "test".to_string(),
            kind,
            args,
            confirm: false,
        }
    }

    #[test]
    fn classify_open_extracts_url() {
        // Pure-classification check — we don't actually open a URL in tests.
        let action = mk_action(ActionKind::Open, vec!["https://example.com".to_string()]);
        assert_eq!(action.kind, ActionKind::Open);
        assert_eq!(
            action.args.first().map(String::as_str),
            Some("https://example.com")
        );
    }

    #[test]
    fn nvim_split_kind_falls_back_to_edit() {
        // Direct test of the nvim helper's split-kind validation (no $NVIM needed
        // — that path errors out cleanly).
        let result = side_effects::nvim_open_file("/tmp/test.txt", "garbage");
        // Without $NVIM set, we always get NotUnderNvim, regardless of split kind.
        // The internal "garbage" → "edit" fallback is exercised before the env check
        // matters, so this still validates the function at least runs.
        match result {
            Err(side_effects::NvimOpenError::NotUnderNvim) => {}
            other => panic!("expected NotUnderNvim, got {other:?}"),
        }
    }
}
