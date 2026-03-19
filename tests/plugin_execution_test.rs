//! End-to-end integration tests for plugin discovery and execution.

use std::path::PathBuf;
use std::sync::Arc;

use larkline::plugin::{Plugin, registry, script::ScriptPlugin};

#[tokio::test]
async fn end_to_end_hello_world_execution() {
    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/plugins");
    let discovered = registry::scan(&[examples_dir]).expect("scan failed");

    let hello = discovered
        .into_iter()
        .find(|d| d.metadata.name == "Hello World")
        .expect("hello-world plugin not found");

    let plugin: Arc<dyn Plugin> = Arc::new(ScriptPlugin::from_discovered(hello));
    let output = plugin.execute().await.expect("execution failed");

    assert_eq!(output.title, "Hello from Larkline!");
    assert_eq!(output.items.len(), 2);
    assert_eq!(output.items[0].label, "Hello, World!");
    assert!(!output.items[1].actions.is_empty());
}

#[tokio::test]
async fn scan_with_empty_plugin_dir_returns_empty() {
    let nonexistent = PathBuf::from("/tmp/larkline-test-nonexistent");
    let plugins = registry::scan(&[nonexistent]).expect("scan should not fail");
    assert!(plugins.is_empty());
}
