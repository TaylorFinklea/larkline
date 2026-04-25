//! Integration test for the `lark list --json` subcommand.
//!
//! This is the headless contract consumed by `lark.nvim` (Telescope source) and
//! any other automation. The shape and the stdout-cleanliness guarantee are
//! both load-bearing — break either one and downstream tools break.

use std::process::Command;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields are checked via `serde::Deserialize` errors, not direct reads.
struct ListEntry {
    name: String,
    description: String,
    icon: String,
    icon_nerd: Option<String>,
    category: Option<String>,
    plugin_group: Option<String>,
    quickkey: Option<String>,
    keybinding: Option<String>,
    kind: String,
    author: String,
    version: String,
    secrets: Vec<String>,
    has_settings: bool,
    is_widget: bool,
    is_mini_app: bool,
    streaming: bool,
}

#[test]
fn lark_list_emits_clean_json_array() {
    let output = Command::new(env!("CARGO_BIN_EXE_lark"))
        .arg("list")
        .output()
        .expect("failed to execute `lark list`");

    assert!(
        output.status.success(),
        "lark list exited with {}: stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout was not UTF-8");

    // Stdout must be parseable JSON — no tracing output, no log lines, no banners.
    // This is the load-bearing part: lark.nvim runs `vim.json.decode(stdout)`.
    let entries: Vec<ListEntry> = serde_json::from_str(&stdout).unwrap_or_else(|err| {
        panic!("lark list output not valid JSON: {err}\n--- stdout ---\n{stdout}")
    });

    // The user's plugin dir may or may not have plugins, but the dev workspace
    // ships ~40 in examples/plugins. We accept zero (CI sandbox) but reject
    // anything that's not a well-typed array.
    for entry in &entries {
        assert!(!entry.name.is_empty(), "entry has empty name");
        assert!(
            entry.kind == "Lua" || entry.kind == "Script",
            "unexpected kind: {}",
            entry.kind
        );
    }
}
