//! Script plugin backend — executes a shell script and parses its output.

use std::path::PathBuf;

use crate::plugin::registry::DiscoveredPlugin;
use crate::plugin::traits::{Plugin, PluginError, PluginMetadata, PluginOutput};

/// A plugin that runs an external script and captures its stdout.
///
/// JSON output is parsed into [`PluginOutput`]. Non-JSON output falls back to
/// [`PluginOutput::raw_text`].
pub struct ScriptPlugin {
    metadata: PluginMetadata,
    entry_path: PathBuf,
    plugin_dir: PathBuf,
}

impl ScriptPlugin {
    /// Create a `ScriptPlugin` from a [`DiscoveredPlugin`].
    #[must_use]
    pub fn from_discovered(discovered: DiscoveredPlugin) -> Self {
        let entry_path = discovered.plugin_dir.join(&discovered.entry);
        Self {
            metadata: discovered.metadata,
            entry_path,
            plugin_dir: discovered.plugin_dir,
        }
    }
}

#[async_trait::async_trait]
impl Plugin for ScriptPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn execute(&self) -> Result<PluginOutput, PluginError> {
        let result = tokio::time::timeout(
            self.metadata.timeout,
            tokio::process::Command::new(&self.entry_path)
                .current_dir(&self.plugin_dir)
                .output(),
        )
        .await
        .map_err(|_| PluginError::Timeout(self.metadata.timeout))?
        .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            return Err(PluginError::ExecutionFailed(format!(
                "exit code {}: {}",
                result.status,
                stderr.trim_end()
            )));
        }

        let stdout = String::from_utf8_lossy(&result.stdout);

        // Try JSON first; fall back to raw text.
        match serde_json::from_str::<PluginOutput>(&stdout) {
            Ok(output) => Ok(output),
            Err(_) => Ok(PluginOutput {
                title: self.metadata.name.clone(),
                items: Vec::new(),
                raw_text: Some(stdout.into_owned()),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::registry::parse_manifest;
    use std::path::PathBuf;

    fn fixture_plugin(name: &str) -> ScriptPlugin {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/plugins")
            .join(name);
        let discovered = parse_manifest(&dir).unwrap();
        ScriptPlugin::from_discovered(discovered)
    }

    fn example_plugin(name: &str) -> ScriptPlugin {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples/plugins")
            .join(name);
        let discovered = parse_manifest(&dir).unwrap();
        ScriptPlugin::from_discovered(discovered)
    }

    #[tokio::test]
    async fn executes_hello_world_plugin() {
        let plugin = example_plugin("hello-world");
        let output = plugin.execute().await.expect("execution failed");
        assert_eq!(output.title, "Hello from Larkline!");
        assert!(!output.items.is_empty());
    }

    #[tokio::test]
    async fn falls_back_to_raw_text_for_non_json() {
        let plugin = fixture_plugin("plain-text");
        let output = plugin.execute().await.expect("execution failed");
        assert!(output.raw_text.is_some());
        assert!(output.raw_text.unwrap().contains("Just plain text"));
    }

    #[tokio::test]
    async fn enforces_timeout() {
        let plugin = fixture_plugin("slow");
        let result = plugin.execute().await;
        assert!(
            matches!(result, Err(PluginError::Timeout(_))),
            "expected Timeout error, got: {result:?}"
        );
    }
}
