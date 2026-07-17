//! Script plugin backend — executes a shell script and parses its output.

use std::path::PathBuf;

use crate::plugin::registry::DiscoveredPlugin;
use crate::plugin::traits::{Plugin, PluginError, PluginMetadata, PluginOutput, StreamChunk};

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
    pub fn from_discovered(mut discovered: DiscoveredPlugin) -> Self {
        let entry_path = discovered.plugin_dir.join(&discovered.entry);
        discovered.metadata.entry_path = Some(entry_path.clone());
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
        self.execute_inner(None).await
    }

    async fn execute_with_form(
        &self,
        form_values: std::collections::HashMap<String, String>,
    ) -> Result<PluginOutput, PluginError> {
        self.execute_inner(Some(form_values)).await
    }

    /// Streaming execution: newline-delimited JSON on stdout. The first line
    /// is a `PluginOutput` header (title + initial items), each subsequent
    /// line an `OutputItem`. Invalid lines are skipped with a warning.
    async fn execute_streaming(
        &self,
        partial: tokio::sync::mpsc::Sender<StreamChunk>,
    ) -> Result<PluginOutput, PluginError> {
        use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, BufReader};

        if !self.entry_path.exists() {
            return Err(PluginError::ExecutionFailed(format!(
                "entry script not found: {}",
                self.entry_path.display()
            )));
        }

        let store_path = crate::plugin::store::store_path_for(
            &self.metadata.name,
            self.metadata.plugin_group.as_deref(),
        );

        let mut cmd = tokio::process::Command::new(&self.entry_path);
        cmd.current_dir(&self.plugin_dir)
            .env("LARK_STORE_PATH", &store_path)
            // The TUI runs the terminal in raw mode — the child must not
            // inherit (and wedge on) its stdin.
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            // Captured and surfaced on failure — a nulled stderr made a
            // failing streaming plugin report empty success.
            .stderr(std::process::Stdio::piped())
            // Reap the child if the timeout future below is dropped, so a
            // hung script is not orphaned past its timeout.
            .kill_on_drop(true);

        // Inject secrets from .env as environment variables.
        if let Ok(secrets) = crate::plugin::engine::SECRETS.try_with(Clone::clone) {
            for (key, value) in secrets.iter() {
                cmd.env(key, value);
            }
        }

        tokio::time::timeout(self.metadata.timeout, async {
            let mut child = cmd
                .spawn()
                .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

            // Drain stderr concurrently so a chatty child can't fill the
            // pipe buffer and deadlock against the stdout line loop.
            let mut stderr = child.stderr.take().expect("stderr was piped");
            let stderr_task = tokio::spawn(async move {
                let mut buf = Vec::new();
                let _ = stderr.read_to_end(&mut buf).await;
                buf
            });

            let stdout = child.stdout.take().expect("stdout was piped");
            let mut lines = BufReader::new(stdout).lines();
            let mut is_first = true;
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                if is_first {
                    is_first = false;
                    // First line: parse as PluginOutput header.
                    match serde_json::from_str::<PluginOutput>(&line) {
                        Ok(output) => {
                            let _ = partial
                                .send(StreamChunk {
                                    title: Some(output.title),
                                    items: output.items,
                                })
                                .await;
                        }
                        Err(e) => {
                            tracing::warn!(
                                line = %line,
                                error = %e,
                                "streaming: invalid header line, skipping"
                            );
                        }
                    }
                } else {
                    // Subsequent lines: parse as OutputItem.
                    match serde_json::from_str::<crate::plugin::traits::OutputItem>(&line) {
                        Ok(item) => {
                            let _ = partial
                                .send(StreamChunk {
                                    title: None,
                                    items: vec![item],
                                })
                                .await;
                        }
                        Err(e) => {
                            tracing::warn!(
                                line = %line,
                                error = %e,
                                "streaming: invalid item line, skipping"
                            );
                        }
                    }
                }
            }

            let status = child
                .wait()
                .await
                .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
            if !status.success() {
                let stderr = stderr_task.await.unwrap_or_default();
                let stderr = String::from_utf8_lossy(&stderr);
                return Err(PluginError::ExecutionFailed(format!(
                    "exit code {}: {}",
                    status,
                    stderr.trim_end()
                )));
            }

            // The app accumulates the streamed chunks; nothing to repeat here.
            Ok(PluginOutput::default())
        })
        .await
        .map_err(|_| PluginError::Timeout(self.metadata.timeout))?
    }
}

impl ScriptPlugin {
    /// Shared execution logic for `execute()` and `execute_with_form()`.
    async fn execute_inner(
        &self,
        form_values: Option<std::collections::HashMap<String, String>>,
    ) -> Result<PluginOutput, PluginError> {
        if !self.entry_path.exists() {
            return Err(PluginError::ExecutionFailed(format!(
                "entry script not found: {}",
                self.entry_path.display()
            )));
        }

        let store_path = crate::plugin::store::store_path_for(
            &self.metadata.name,
            self.metadata.plugin_group.as_deref(),
        );

        let mut cmd = tokio::process::Command::new(&self.entry_path);
        cmd.current_dir(&self.plugin_dir)
            .env("LARK_STORE_PATH", &store_path)
            // Reap the child if the timeout future below is dropped, so a hung
            // script is not orphaned past its timeout.
            .kill_on_drop(true);

        // Inject secrets from .env as environment variables.
        if let Ok(secrets) = crate::plugin::engine::SECRETS.try_with(Clone::clone) {
            for (key, value) in secrets.iter() {
                cmd.env(key, value);
            }
        }

        if let Some(values) = form_values {
            let form_json = serde_json::to_string(&values).unwrap_or_else(|_| "{}".to_string());
            cmd.env("LARK_FORM_JSON", &form_json);
        }

        let result = tokio::time::timeout(self.metadata.timeout, cmd.output())
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
                raw_text: Some(stdout.into_owned()),
                ..Default::default()
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

    fn streaming_plugin_from_script(name: &str, script: &str) -> ScriptPlugin {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().expect("tempdir");
        let plugin_dir = dir.path().join(name);
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let entry = plugin_dir.join("run.sh");
        std::fs::write(&entry, script).unwrap();
        std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(
            plugin_dir.join("manifest.toml"),
            format!(
                r#"
[plugin]
name = "{name}"
description = "test"
version = "0.1.0"
author = "test"
icon = "T"
entry = "run.sh"
timeout_seconds = 5
streaming = true
"#
            ),
        )
        .unwrap();
        let discovered = parse_manifest(&plugin_dir).unwrap().remove(0);
        std::mem::forget(dir);
        ScriptPlugin::from_discovered(discovered)
    }

    #[tokio::test]
    async fn streaming_script_emits_chunks_line_by_line() {
        let plugin = streaming_plugin_from_script(
            "stream-ok",
            concat!(
                "#!/bin/sh\n",
                "echo '{\"title\":\"stream\",\"items\":[{\"label\":\"first\"}]}'\n",
                "echo '{\"label\":\"second\"}'\n",
                "echo '{\"label\":\"third\"}'\n",
            ),
        );
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);

        let exec = tokio::spawn(async move { plugin.execute_streaming(tx).await });
        let mut chunks = Vec::new();
        while let Some(chunk) = rx.recv().await {
            chunks.push(chunk);
        }
        let result = exec.await.expect("task").expect("streaming run succeeds");

        assert_eq!(chunks.len(), 3, "header + two item chunks");
        assert_eq!(chunks[0].title.as_deref(), Some("stream"));
        assert_eq!(chunks[0].items[0].label, "first");
        assert_eq!(chunks[1].items[0].label, "second");
        assert_eq!(chunks[2].items[0].label, "third");
        // Final result carries no duplicate items — the app accumulates the
        // streamed output from the chunks.
        assert!(result.items.is_empty());
    }

    fn fixture_plugin(name: &str) -> ScriptPlugin {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/plugins")
            .join(name);
        let discovered = parse_manifest(&dir).unwrap().remove(0);
        ScriptPlugin::from_discovered(discovered)
    }

    fn example_plugin(name: &str) -> ScriptPlugin {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples/plugins")
            .join(name);
        let discovered = parse_manifest(&dir).unwrap().remove(0);
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

    #[tokio::test]
    async fn returns_error_for_missing_entry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugin_dir = dir.path().join("no-entry");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("manifest.toml"),
            r#"
[plugin]
name = "No Entry"
description = "test"
version = "0.1.0"
author = "test"
icon = "T"
entry = "nonexistent.sh"
"#,
        )
        .unwrap();

        // parse_manifest no longer checks entry existence — succeeds here.
        let discovered = parse_manifest(&plugin_dir).unwrap().remove(0);
        let plugin = ScriptPlugin::from_discovered(discovered);

        // But execute() should catch the missing entry and return an error.
        let result = plugin.execute().await;
        assert!(
            matches!(result, Err(PluginError::ExecutionFailed(_))),
            "expected ExecutionFailed error, got: {result:?}"
        );
    }
}
