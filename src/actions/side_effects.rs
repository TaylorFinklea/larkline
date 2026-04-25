//! Side-effect primitives shared by the TUI's action handler and the CLI's
//! [`crate::actions::execute`] dispatcher.
//!
//! These were originally private helpers in `app.rs`; moved here so the CLI
//! `lark action` subcommand (Phase C of v0.13.0) can reuse them without
//! depending on TUI types.

use anyhow::Result;

/// Open a URL via the platform default handler (`open` on macOS, `xdg-open`
/// elsewhere). Errors are logged but not propagated — opening a URL is a
/// best-effort side-effect.
pub fn open_url(url: &str) {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(cmd).arg(url).spawn() {
        tracing::warn!(error = %e, url = url, "failed to open URL");
    }
}

/// Copy `text` to the system clipboard via `arboard`.
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_text(text)?;
    tracing::info!("copied to clipboard");
    Ok(())
}

/// Error opening a file in the parent Neovim instance.
#[derive(Debug)]
pub enum NvimOpenError {
    /// `$NVIM` env var is not set — the caller is not running as a child of nvim.
    NotUnderNvim,
    /// The nvim command spawned but exited non-zero, or could not be spawned.
    CommandFailed(String),
}

impl std::fmt::Display for NvimOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotUnderNvim => f.write_str("not running under Neovim"),
            Self::CommandFailed(e) => write!(f, "nvim command failed: {e}"),
        }
    }
}

/// Open a file in the parent Neovim via `nvim --server $NVIM --remote-send`.
///
/// `split` is one of `edit`, `split`, `vsplit`, `tabedit`. Any other value
/// falls back to `edit`.
pub fn nvim_open_file(path: &str, split: &str) -> Result<(), NvimOpenError> {
    let Ok(socket) = std::env::var("NVIM") else {
        return Err(NvimOpenError::NotUnderNvim);
    };
    let cmd_verb = match split {
        "split" | "vsplit" | "tabedit" => split,
        _ => "edit",
    };
    // Build an ex sequence: <Esc>:<verb> <path><CR>. Escape backslashes and
    // double quotes in the path; nvim's remote-send parses the string as
    // key notation, so "<" and ">" embedded in the path would be misread.
    let escaped = path
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('<', "\\<");
    let keys = format!("<Esc>:{cmd_verb} {escaped}<CR>");

    let output = std::process::Command::new("nvim")
        .args(["--server", &socket, "--remote-send", &keys])
        .output()
        .map_err(|e| NvimOpenError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(NvimOpenError::CommandFailed(if stderr.is_empty() {
            format!("exit status {}", output.status)
        } else {
            stderr
        }));
    }
    Ok(())
}
