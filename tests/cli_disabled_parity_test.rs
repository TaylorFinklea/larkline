//! Disabled-plugin parity for headless surfaces (mkv.26).
//!
//! A command disabled in the Plugin Manager must not be listed, invocable,
//! or agent-callable from any headless surface — the TUI's disabled filter
//! is a safety boundary, not a display preference.

use std::path::Path;
use std::process::Command;

fn write_plugin(plugins_dir: &Path, name: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = plugins_dir.join(name.to_lowercase().replace(' ', "-"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("manifest.toml"),
        format!(
            r#"
[plugin]
name = "{name}"
description = "parity test plugin"
version = "0.1.0"
author = "test"
icon = "P"
entry = "run.sh"
"#
        ),
    )
    .unwrap();
    // A working entry, so an invoke failure can only mean "filtered out" —
    // not "entry script missing".
    let entry = dir.join("run.sh");
    std::fs::write(&entry, "#!/bin/sh\necho '{\"title\":\"ran\"}'\n").unwrap();
    std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Isolated home: config points at a temp plugin dir; the plugin-manager
/// state disables "Beta Plugin". Returns the tempdir holding everything.
fn setup_isolated_home() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("tempdir");
    let plugins_dir = root.path().join("plugins");
    write_plugin(&plugins_dir, "Alpha Plugin");
    write_plugin(&plugins_dir, "Beta Plugin");

    let config_dir = root.path().join("config").join("larkline");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        format!(
            "[general]\nplugin_dirs = [{:?}]\n",
            plugins_dir.to_string_lossy()
        ),
    )
    .unwrap();

    let data_dir = root.path().join("data").join("larkline");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(
        data_dir.join("plugin-manager.json"),
        r#"{"disabled_plugins": ["Beta Plugin"]}"#,
    )
    .unwrap();

    root
}

fn lark(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lark"))
        .args(args)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .output()
        .expect("failed to run lark")
}

#[test]
fn disabled_plugin_is_not_listed() {
    let root = setup_isolated_home();

    let output = lark(root.path(), &["list"]);

    assert!(output.status.success(), "lark list failed");
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let entries: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).expect("lark list emits JSON");
    let names: Vec<&str> = entries
        .iter()
        .filter_map(|e| e.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(
        names.contains(&"Alpha Plugin"),
        "enabled plugin must be listed, got: {names:?}"
    );
    assert!(
        !names.contains(&"Beta Plugin"),
        "a plugin disabled in the Plugin Manager must not appear in lark list"
    );
}

#[test]
fn disabled_plugin_is_not_invocable() {
    let root = setup_isolated_home();

    let output = lark(root.path(), &["invoke", "Beta Plugin"]);

    assert!(
        !output.status.success(),
        "invoking a disabled plugin must fail, got stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}
