//! Integration tests for `examples/plugins/_shared/errors.lua` translators.
//!
//! Loads the canonical error helpers via mlua, then exercises `from_exit`
//! with stderr samples drawn from real plugins (gh, bw, atlassian, kubectl).
//! Each translator gets one positive case (matches and produces the expected
//! shape) and the suite ends with a negative case (no pattern → nil) so we
//! verify the fall-through contract callers depend on.

use std::path::PathBuf;

use mlua::{Lua, LuaOptions, StdLib, Table, Value};

fn errors_lua_source() -> String {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/plugins/_shared/errors.lua");
    std::fs::read_to_string(path).expect("read errors.lua")
}

/// Build a fresh sandbox-style VM. Mirrors `src/plugin/lua.rs::create_vm` minus
/// the `lark.*` host API — the helpers are pure Lua and don't need it.
fn fresh_vm() -> Lua {
    Lua::new_with(
        StdLib::COROUTINE | StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8,
        LuaOptions::default(),
    )
    .expect("create lua vm")
}

/// Load `errors.lua` and return the module table (the Lua `M` it returns).
fn load_module(lua: &Lua) -> Table {
    lua.load(errors_lua_source())
        .set_name("errors.lua")
        .call::<Table>(())
        .expect("load errors.lua")
}

fn assert_error_item(value: &Value, expected_label_substring: &str) -> Table {
    let table = match value {
        Value::Table(t) => t.clone(),
        other => panic!("from_exit returned non-table: {other:?}"),
    };
    let label: String = table.get("label").expect("label field");
    assert!(
        label.contains(expected_label_substring),
        "label {label:?} did not contain {expected_label_substring:?}"
    );
    let icon: String = table.get("icon").expect("icon field");
    assert_eq!(icon, "!", "error_item must use the `!` icon");
    table
}

#[test]
fn missing_cli_pattern_produces_install_help() {
    let lua = fresh_vm();
    let module = load_module(&lua);
    let from_exit: mlua::Function = module.get("from_exit").unwrap();

    let hints = lua.create_table().unwrap();
    hints.set("cli", "gh").unwrap();
    hints
        .set("install_url", "https://cli.github.com/manual/installation")
        .unwrap();

    let result: Value = from_exit
        .call(("bash: gh: command not found", hints))
        .unwrap();
    let item = assert_error_item(&result, "gh not found");
    let help_url: String = item.get("help_url").unwrap();
    assert_eq!(help_url, "https://cli.github.com/manual/installation");
    let detail: String = item.get("detail").unwrap();
    assert!(detail.contains("Install:"), "detail: {detail}");
}

#[test]
fn auth_failure_pattern_suggests_login_command() {
    let lua = fresh_vm();
    let module = load_module(&lua);
    let from_exit: mlua::Function = module.get("from_exit").unwrap();

    let hints = lua.create_table().unwrap();
    hints.set("service", "GitHub").unwrap();
    hints.set("login_command", "gh auth login").unwrap();

    let stderr = "gh: HTTP 401: Bad credentials (https://api.github.com/repos/foo/bar)";
    let result: Value = from_exit.call((stderr, hints)).unwrap();
    let item = assert_error_item(&result, "GitHub auth failed");
    let detail: String = item.get("detail").unwrap();
    assert!(detail.contains("gh auth login"), "detail: {detail}");
}

#[test]
fn rate_limit_pattern_extracts_retry_after() {
    let lua = fresh_vm();
    let module = load_module(&lua);
    let from_exit: mlua::Function = module.get("from_exit").unwrap();

    let hints = lua.create_table().unwrap();
    hints.set("service", "GitHub").unwrap();

    let stderr = "API rate limit exceeded\nRetry-After: 60\n";
    let result: Value = from_exit.call((stderr, hints)).unwrap();
    let item = assert_error_item(&result, "GitHub rate limited");
    let detail: String = item.get("detail").unwrap();
    assert!(
        detail.contains("60s"),
        "detail should mention 60s: {detail}"
    );
}

#[test]
fn rate_limit_without_retry_after_falls_back_to_generic_message() {
    let lua = fresh_vm();
    let module = load_module(&lua);
    let from_exit: mlua::Function = module.get("from_exit").unwrap();

    let stderr = "HTTP 429 Too Many Requests";
    let result: Value = from_exit.call(stderr).unwrap();
    let item = assert_error_item(&result, "rate limited");
    let detail: String = item.get("detail").unwrap();
    assert!(
        detail.contains("try again later"),
        "detail should be generic: {detail}"
    );
}

#[test]
fn network_unreachable_pattern_matches_dns_failure() {
    let lua = fresh_vm();
    let module = load_module(&lua);
    let from_exit: mlua::Function = module.get("from_exit").unwrap();

    let stderr = "curl: (6) Could not resolve host: api.example.com";
    let result: Value = from_exit.call(stderr).unwrap();
    let item = assert_error_item(&result, "Network unreachable");
    let detail: String = item.get("detail").unwrap();
    assert!(detail.contains("connection"), "detail: {detail}");
}

#[test]
fn unmatched_stderr_returns_nil() {
    let lua = fresh_vm();
    let module = load_module(&lua);
    let from_exit: mlua::Function = module.get("from_exit").unwrap();

    let result: Value = from_exit
        .call("Something unexpected went wrong with no known pattern")
        .unwrap();
    assert!(
        matches!(result, Value::Nil),
        "expected nil for unmatched stderr, got {result:?}"
    );
}

#[test]
fn error_item_passes_through_retry_action_and_help_url() {
    let lua = fresh_vm();
    let module = load_module(&lua);
    let error_item: mlua::Function = module.get("error_item").unwrap();

    let opts = lua.create_table().unwrap();
    opts.set("label", "Custom failure").unwrap();
    opts.set("detail", "with detail").unwrap();
    opts.set("help_url", "https://docs.example.com/troubleshoot")
        .unwrap();

    let retry = lua.create_table().unwrap();
    retry.set("label", "Retry").unwrap();
    retry.set("kind", "chain").unwrap();
    let args = lua.create_table().unwrap();
    args.set(1, "fetch_issues").unwrap();
    retry.set("args", args).unwrap();
    opts.set("retry_action", retry).unwrap();

    let item: Table = error_item.call(opts).unwrap();
    let label: String = item.get("label").unwrap();
    assert_eq!(label, "Custom failure");
    let icon: String = item.get("icon").unwrap();
    assert_eq!(icon, "!");
    let help_url: String = item.get("help_url").unwrap();
    assert_eq!(help_url, "https://docs.example.com/troubleshoot");
    let retry: Table = item.get("retry_action").unwrap();
    let retry_kind: String = retry.get("kind").unwrap();
    assert_eq!(retry_kind, "chain");
}
