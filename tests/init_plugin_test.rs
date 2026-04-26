//! Integration tests for the `lark init-plugin` subcommand.

use std::process::Command;

/// Helper: run `lark init-plugin <args>` with `XDG_CONFIG_HOME` redirected to a tempdir.
fn run_init_plugin(xdg_home: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lark"))
        .env("XDG_CONFIG_HOME", xdg_home)
        .arg("init-plugin")
        .args(args)
        .output()
        .expect("failed to execute `lark init-plugin`")
}

#[test]
fn init_plugin_default_creates_lua_scaffold() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let output = run_init_plugin(tmp.path(), &["test-default"]);

    assert!(
        output.status.success(),
        "expected success, got {}: stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let plugin_dir = tmp.path().join("larkline/plugins/test-default");
    let manifest_path = plugin_dir.join("manifest.toml");
    let lua_path = plugin_dir.join("init.lua");

    assert!(manifest_path.exists(), "manifest.toml not found");
    assert!(lua_path.exists(), "init.lua not found");

    let manifest_content = std::fs::read_to_string(&manifest_path).expect("read manifest.toml");
    // Must be valid TOML.
    let _parsed: toml::Value =
        toml::from_str(&manifest_content).expect("manifest.toml did not parse as TOML");
    assert!(
        manifest_content.contains("name = \"test-default\""),
        "manifest missing plugin name"
    );

    let lua_content = std::fs::read_to_string(&lua_path).expect("read init.lua");
    assert!(!lua_content.is_empty(), "init.lua is empty");
    assert!(
        lua_content.contains("lark.register"),
        "init.lua missing lark.register"
    );
}

#[test]
fn init_plugin_shell_creates_shell_scaffold() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let output = run_init_plugin(tmp.path(), &["test-shell", "--shell"]);

    assert!(
        output.status.success(),
        "expected success, got {}: stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let plugin_dir = tmp.path().join("larkline/plugins/test-shell");
    let manifest_path = plugin_dir.join("manifest.toml");
    let sh_path = plugin_dir.join("run.sh");

    assert!(manifest_path.exists(), "manifest.toml not found");
    assert!(sh_path.exists(), "run.sh not found");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::metadata(&sh_path)
            .expect("stat run.sh")
            .permissions();
        assert!(
            perms.mode() & 0o111 != 0,
            "run.sh does not have executable bit set (mode={:#o})",
            perms.mode()
        );
    }

    let sh_content = std::fs::read_to_string(&sh_path).expect("read run.sh");
    assert!(
        sh_content.contains("#!/usr/bin/env bash"),
        "run.sh missing shebang"
    );
    assert!(sh_content.contains("jq -n"), "run.sh missing jq -n");
}

#[test]
fn init_plugin_multi_creates_two_command_scaffold() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let output = run_init_plugin(tmp.path(), &["test-multi", "--multi"]);

    assert!(
        output.status.success(),
        "expected success, got {}: stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let plugin_dir = tmp.path().join("larkline/plugins/test-multi");
    let manifest_path = plugin_dir.join("manifest.toml");
    let cmd1_path = plugin_dir.join("command_one.lua");
    let cmd2_path = plugin_dir.join("command_two.lua");

    assert!(manifest_path.exists(), "manifest.toml not found");
    assert!(cmd1_path.exists(), "command_one.lua not found");
    assert!(cmd2_path.exists(), "command_two.lua not found");

    let manifest_content = std::fs::read_to_string(&manifest_path).expect("read manifest.toml");
    // Must be valid TOML.
    let _parsed: toml::Value =
        toml::from_str(&manifest_content).expect("manifest.toml did not parse as TOML");
    assert!(
        manifest_content.contains("[[commands]]"),
        "manifest missing [[commands]] section"
    );
}

#[test]
fn init_plugin_refuses_to_overwrite_existing_dir() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // First invocation — must succeed.
    let first = run_init_plugin(tmp.path(), &["foo"]);
    assert!(
        first.status.success(),
        "first init-plugin failed: stderr={}",
        String::from_utf8_lossy(&first.stderr)
    );

    let lua_path = tmp.path().join("larkline/plugins/foo/init.lua");
    let original_content =
        std::fs::read_to_string(&lua_path).expect("read init.lua after first run");

    // Second invocation — must fail.
    let second = run_init_plugin(tmp.path(), &["foo"]);
    assert!(
        !second.status.success(),
        "second init-plugin should have failed but exited with {}",
        second.status
    );

    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(
        stderr.contains("already exists"),
        "expected 'already exists' in stderr, got: {stderr}"
    );

    // Original file must be unchanged.
    let content_after = std::fs::read_to_string(&lua_path).expect("read init.lua after second run");
    assert_eq!(
        original_content, content_after,
        "init.lua was mutated by the failed second invocation"
    );
}
