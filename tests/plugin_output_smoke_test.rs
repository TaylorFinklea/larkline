//! Output schema smoke tests for pure plugins (no auth, no network, no state).
//!
//! # Plugin inclusion decisions
//!
//! ## Included
//!
//! - **Emoji** (`init.lua`): pure in-memory list, no exec/store/network
//! - **Hello World (Lua)** (`init.lua`): calls `lark.exec("hostname")`, universally available
//! - **Timezones** (`init.lua`): calls `lark.exec("env", ...)` with `date`; universally available
//! - **Quicklinks / Links** (`links.lua`): `lark.store.get` tolerates nil; returns placeholder item
//! - **Calculator** (`init.lua`): without `form_values` returns a form; `lark.exec("which", ...)`
//!   used only to set placeholder text, harmless if qalc is absent
//! - **Base64 Encode** (`b64encode.lua`): without `form_values` returns a form; pure transform
//!
//! ## Skipped
//!
//! - encode-decode URL/JWT commands: same form-return pattern as Base64 Encode; already covered
//! - All remaining plugins: require network, auth tokens, env vars, or filesystem state

use std::path::PathBuf;
use std::sync::Arc;

use larkline::plugin::{Plugin, build_plugin, registry};

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/plugins")
}

/// Find the first discovered plugin whose display name matches `name`.
fn find_plugin(name: &str) -> Arc<dyn Plugin> {
    let discovered = registry::scan(&[examples_dir()]).expect("scan failed");
    let meta = discovered
        .into_iter()
        .find(|d| d.metadata.name == name)
        .unwrap_or_else(|| panic!("plugin '{name}' not found in examples/plugins"));
    build_plugin(meta)
}

// ---------------------------------------------------------------------------
// Emoji
// ---------------------------------------------------------------------------

#[tokio::test]
async fn plugin_emoji_returns_well_formed_output() {
    let plugin = find_plugin("Emoji");
    let output = plugin.execute().await.expect("execution failed");

    assert!(!output.title.is_empty(), "title was empty");
    assert!(
        !output.items.is_empty(),
        "expected emoji items, got empty list"
    );
    for item in &output.items {
        assert!(!item.label.is_empty(), "item has empty label");
    }
}

// ---------------------------------------------------------------------------
// Hello World (Lua)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn plugin_hello_world_lua_returns_well_formed_output() {
    let plugin = find_plugin("Hello World (Lua)");
    let output = plugin.execute().await.expect("execution failed");

    assert!(!output.title.is_empty(), "title was empty");
    assert!(
        !output.items.is_empty(),
        "expected items from Hello World (Lua)"
    );
    for item in &output.items {
        assert!(!item.label.is_empty(), "item has empty label");
    }
}

// ---------------------------------------------------------------------------
// Timezones
// ---------------------------------------------------------------------------

#[tokio::test]
async fn plugin_timezones_returns_well_formed_output() {
    let plugin = find_plugin("Timezones");
    let output = plugin.execute().await.expect("execution failed");

    assert!(!output.title.is_empty(), "title was empty");
    assert!(
        !output.items.is_empty(),
        "expected timezone items, got empty list"
    );
    for item in &output.items {
        assert!(!item.label.is_empty(), "item has empty label");
    }
}

// ---------------------------------------------------------------------------
// Quicklinks / Links
// ---------------------------------------------------------------------------

#[tokio::test]
async fn plugin_quicklinks_links_returns_well_formed_output() {
    // Uses lark.store.get("links") or "[]" -- empty store returns a placeholder item.
    let plugin = find_plugin("Links");
    let output = plugin.execute().await.expect("execution failed");

    assert!(!output.title.is_empty(), "title was empty");
    for item in &output.items {
        assert!(!item.label.is_empty(), "item has empty label");
    }
}

// ---------------------------------------------------------------------------
// Calculator -- returns a form when no form_values are set
// ---------------------------------------------------------------------------

#[tokio::test]
async fn plugin_calculator_returns_well_formed_output() {
    // Without form_values the plugin returns a form (no items).
    // We verify the shape is still a valid PluginOutput with a non-empty title.
    let plugin = find_plugin("Calculator");
    let output = plugin.execute().await.expect("execution failed");

    assert!(!output.title.is_empty(), "title was empty");
    // items may be empty when a form is returned -- that is still well-formed.
    for item in &output.items {
        assert!(!item.label.is_empty(), "item has empty label");
    }
}

// ---------------------------------------------------------------------------
// Base64 Encode -- returns a form when no form_values are set
// ---------------------------------------------------------------------------

#[tokio::test]
async fn plugin_base64_encode_returns_well_formed_output() {
    // Without form_values the plugin returns a form spec; items will be empty.
    let plugin = find_plugin("Base64 Encode");
    let output = plugin.execute().await.expect("execution failed");

    assert!(!output.title.is_empty(), "title was empty");
    for item in &output.items {
        assert!(!item.label.is_empty(), "item has empty label");
    }
}
