//! Integration test for the `lark action` subcommand.
//!
//! This is the action-execution contract consumed by `lark.nvim` — the
//! Telescope source feeds an `ItemAction` JSON in, parses one of three
//! `outcome` shapes back. Every outcome variant has a Telescope-side
//! handler; breakage on either side breaks the picker.

use std::process::Command;

use serde_json::Value;

fn run_lark_action(args: &[&str]) -> (String, String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_lark"))
        .arg("action")
        .args(args)
        .output()
        .expect("failed to execute `lark action`");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn lark_action_clipboard_emits_side_outcome() {
    let (stdout, stderr, code) = run_lark_action(&[
        "API Reference",
        "--action-json",
        r#"{"label":"copy","kind":"clipboard","args":["lark action test"]}"#,
    ]);

    assert_eq!(code, 0, "lark action exited with {code}\nstderr: {stderr}");

    let parsed: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("not JSON: {err}\nstdout: {stdout}"));

    assert_eq!(
        parsed["outcome"].as_str(),
        Some("side"),
        "expected side outcome, got: {parsed}"
    );
    assert_eq!(
        parsed["summary"].as_str(),
        Some("Copied to clipboard"),
        "summary mismatch"
    );
}

#[test]
fn lark_action_confirm_required_shell_returns_needs_confirmation() {
    let (stdout, stderr, code) = run_lark_action(&[
        "API Reference",
        "--action-json",
        r#"{"label":"echo greet","kind":"shell","args":["echo","hi"],"confirm":true}"#,
    ]);
    assert_eq!(code, 0, "exited {code}\nstderr: {stderr}");

    let parsed: Value = serde_json::from_str(&stdout).expect("not JSON");
    assert_eq!(parsed["outcome"].as_str(), Some("needs_confirmation"));
    assert_eq!(parsed["command"].as_str(), Some("echo"));
    assert_eq!(parsed["args"][0].as_str(), Some("hi"));
}

#[test]
fn lark_action_unconfirmed_shell_runs_and_captures_output() {
    let (stdout, stderr, code) = run_lark_action(&[
        "API Reference",
        "--action-json",
        r#"{"label":"echo","kind":"shell","args":["echo","captured"]}"#,
    ]);
    assert_eq!(code, 0, "exited {code}\nstderr: {stderr}");

    let parsed: Value = serde_json::from_str(&stdout).expect("not JSON");
    assert_eq!(parsed["outcome"].as_str(), Some("side"));
    assert!(
        parsed["summary"]
            .as_str()
            .is_some_and(|s| s.contains("exit 0")),
        "expected exit 0 in summary, got: {parsed}"
    );
    assert_eq!(parsed["stdout"].as_str(), Some("captured\n"));
}

#[test]
fn lark_action_unknown_plugin_exits_nonzero() {
    let (_stdout, stderr, code) = run_lark_action(&[
        "no-such-plugin-zzz",
        "--action-json",
        r#"{"label":"x","kind":"clipboard","args":["irrelevant"]}"#,
    ]);
    assert_ne!(code, 0, "expected non-zero exit");
    assert!(stderr.contains("plugin not found"), "stderr: {stderr}");
}
