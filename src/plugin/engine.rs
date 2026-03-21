//! Plugin execution engine — dispatches plugins as Tokio tasks and sends events back
//! to the app run loop via an `mpsc` channel.

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::plugin::traits::{OutputItem, Plugin, PluginError, PluginOutput};

/// Indicates whether a plugin execution was triggered by the user or by prefetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionSource {
    /// The user explicitly selected this plugin.
    UserSelected,
    /// The plugin was executed in the background on startup (prefetch).
    Prefetch,
}

/// Events sent from the engine to the app run loop.
#[derive(Debug)]
pub enum EngineEvent {
    /// A plugin has started executing.
    PluginStarted {
        /// Index into the engine's plugin list.
        plugin_index: usize,
        /// Whether this is a user-triggered or prefetch execution.
        source: ExecutionSource,
    },
    /// A plugin has finished (successfully or with an error).
    PluginFinished {
        /// Index into the engine's plugin list.
        plugin_index: usize,
        /// The execution result.
        result: Result<PluginOutput, PluginError>,
        /// Whether this is a user-triggered or prefetch execution.
        source: ExecutionSource,
    },
    /// Incremental output from a streaming plugin.
    PartialOutput {
        /// Index into the engine's plugin list.
        plugin_index: usize,
        /// Title (set only on the first partial).
        title: Option<String>,
        /// Items to append to the output.
        items: Vec<OutputItem>,
        /// Whether this is a user-triggered or prefetch execution.
        source: ExecutionSource,
    },
}

/// Manages a set of plugins and dispatches them as async Tokio tasks.
pub struct PluginEngine {
    plugins: Vec<Arc<dyn Plugin>>,
    tx: mpsc::Sender<EngineEvent>,
}

impl PluginEngine {
    /// Create a new `PluginEngine` with the given plugins and event sender.
    #[must_use]
    pub fn new(plugins: Vec<Arc<dyn Plugin>>, tx: mpsc::Sender<EngineEvent>) -> Self {
        Self { plugins, tx }
    }

    /// Returns the number of plugins in this engine.
    #[allow(dead_code)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Returns true if the engine has no plugins.
    #[allow(dead_code)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Spawn plugin execution on a Tokio task (user-selected). Returns immediately.
    ///
    /// Dispatches to streaming or normal mode based on plugin metadata.
    pub fn execute(&self, plugin_index: usize) {
        self.execute_with_source(plugin_index, ExecutionSource::UserSelected);
    }

    /// Execute all prefetch-eligible plugins in the background.
    ///
    /// Called on startup and after refresh. Only runs plugins with `prefetch == true`.
    /// No-ops if called outside of a Tokio runtime context (e.g., from sync tests).
    pub fn execute_all(&self) {
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        for i in 0..self.plugins.len() {
            if self.plugins[i].metadata().prefetch {
                self.execute_with_source(i, ExecutionSource::Prefetch);
            }
        }
    }

    /// Internal dispatch — routes to streaming or normal execution with the given source.
    fn execute_with_source(&self, plugin_index: usize, source: ExecutionSource) {
        let meta = self.plugins[plugin_index].metadata();
        if meta.streaming && meta.entry_path.is_some() {
            self.execute_streaming(plugin_index, source);
        } else {
            self.execute_normal(plugin_index, source);
        }
    }

    /// Normal (non-streaming) execution — waits for plugin to complete, then sends result.
    ///
    /// Uses an outer/inner task pattern so panics in the plugin are caught by the
    /// `JoinHandle` and converted to a `PluginError`, ensuring `PluginFinished` is
    /// always sent even when the plugin task panics.
    fn execute_normal(&self, plugin_index: usize, source: ExecutionSource) {
        let plugin = Arc::clone(&self.plugins[plugin_index]);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx
                .send(EngineEvent::PluginStarted {
                    plugin_index,
                    source: source.clone(),
                })
                .await;
            let handle = tokio::spawn(async move { plugin.execute().await });
            let result = match handle.await {
                Ok(r) => r,
                Err(join_err) => Err(PluginError::ExecutionFailed(format!(
                    "plugin task failed: {join_err}"
                ))),
            };
            let _ = tx
                .send(EngineEvent::PluginFinished {
                    plugin_index,
                    result,
                    source,
                })
                .await;
        });
    }

    /// Streaming execution — reads stdout line-by-line and sends partial output events.
    ///
    /// First line is parsed as `PluginOutput` (header + initial items).
    /// Subsequent lines are parsed as individual `OutputItem`.
    /// Invalid lines are skipped with a warning.
    #[allow(clippy::too_many_lines)]
    fn execute_streaming(&self, plugin_index: usize, source: ExecutionSource) {
        let meta = self.plugins[plugin_index].metadata().clone();
        let entry_path = meta
            .entry_path
            .clone()
            .expect("checked in execute_with_source()");
        let plugin_dir = entry_path.parent().map_or_else(
            || std::path::PathBuf::from("."),
            std::path::Path::to_path_buf,
        );
        let timeout = meta.timeout;
        let tx = self.tx.clone();

        tokio::spawn(async move {
            let _ = tx
                .send(EngineEvent::PluginStarted {
                    plugin_index,
                    source: source.clone(),
                })
                .await;

            let result = tokio::time::timeout(timeout, async {
                use tokio::io::{AsyncBufReadExt, BufReader};
                use tokio::process::Command;

                let mut child = match Command::new(&entry_path)
                    .current_dir(&plugin_dir)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    Ok(c) => c,
                    Err(e) => {
                        return Err(PluginError::ExecutionFailed(format!(
                            "failed to spawn streaming plugin: {e}"
                        )));
                    }
                };

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
                                let _ = tx
                                    .send(EngineEvent::PartialOutput {
                                        plugin_index,
                                        title: Some(output.title),
                                        items: output.items,
                                        source: source.clone(),
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
                        match serde_json::from_str::<OutputItem>(&line) {
                            Ok(item) => {
                                let _ = tx
                                    .send(EngineEvent::PartialOutput {
                                        plugin_index,
                                        title: None,
                                        items: vec![item],
                                        source: source.clone(),
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

                // Wait for the child to exit.
                let _ = child.wait().await;
                Ok(PluginOutput::default())
            })
            .await;

            let finished_result = match result {
                Ok(r) => r,
                Err(_) => Err(PluginError::Timeout(timeout)),
            };
            let _ = tx
                .send(EngineEvent::PluginFinished {
                    plugin_index,
                    result: finished_result,
                    source,
                })
                .await;
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::traits::{PluginMetadata, PluginOutput};

    fn test_metadata() -> PluginMetadata {
        PluginMetadata {
            name: "test".into(),
            description: "test".into(),
            version: "0.1.0".into(),
            author: "test".into(),
            icon: "T".into(),
            icon_nerd: None,
            category: None,
            keybinding: None,
            timeout: std::time::Duration::from_secs(5),
            streaming: false,
            entry_path: None,
            prefetch: true,
            plugin_group: None,
            quickkey: None,
            cache: true,
        }
    }

    struct MockPlugin(PluginMetadata);

    #[async_trait::async_trait]
    impl Plugin for MockPlugin {
        fn metadata(&self) -> &PluginMetadata {
            &self.0
        }
        async fn execute(&self) -> Result<PluginOutput, PluginError> {
            Ok(PluginOutput {
                title: "mock".into(),
                ..Default::default()
            })
        }
    }

    struct FailPlugin(PluginMetadata);

    #[async_trait::async_trait]
    impl Plugin for FailPlugin {
        fn metadata(&self) -> &PluginMetadata {
            &self.0
        }
        async fn execute(&self) -> Result<PluginOutput, PluginError> {
            Err(PluginError::ExecutionFailed("boom".into()))
        }
    }

    #[tokio::test]
    async fn sends_started_then_finished_events() {
        let (tx, mut rx) = mpsc::channel(4);
        let engine = PluginEngine::new(vec![Arc::new(MockPlugin(test_metadata()))], tx);
        engine.execute(0);

        let event1 = rx.recv().await.unwrap();
        assert!(matches!(
            event1,
            EngineEvent::PluginStarted {
                plugin_index: 0,
                source: ExecutionSource::UserSelected
            }
        ));

        let event2 = rx.recv().await.unwrap();
        assert!(matches!(
            event2,
            EngineEvent::PluginFinished {
                plugin_index: 0,
                result: Ok(_),
                source: ExecutionSource::UserSelected
            }
        ));
    }

    #[tokio::test]
    async fn propagates_plugin_error() {
        let (tx, mut rx) = mpsc::channel(4);
        let engine = PluginEngine::new(vec![Arc::new(FailPlugin(test_metadata()))], tx);
        engine.execute(0);

        let _ = rx.recv().await; // PluginStarted
        let event = rx.recv().await.unwrap();
        assert!(matches!(
            event,
            EngineEvent::PluginFinished { result: Err(_), .. }
        ));
    }

    struct PanicPlugin(PluginMetadata);

    #[async_trait::async_trait]
    impl Plugin for PanicPlugin {
        fn metadata(&self) -> &PluginMetadata {
            &self.0
        }
        async fn execute(&self) -> Result<PluginOutput, PluginError> {
            panic!("plugin panicked!")
        }
    }

    #[tokio::test]
    async fn panic_in_plugin_sends_finished_with_error() {
        let (tx, mut rx) = mpsc::channel(4);
        let engine = PluginEngine::new(vec![Arc::new(PanicPlugin(test_metadata()))], tx);
        engine.execute(0);

        let event1 = rx.recv().await.unwrap();
        assert!(matches!(
            event1,
            EngineEvent::PluginStarted {
                plugin_index: 0,
                source: ExecutionSource::UserSelected
            }
        ));

        let event2 = rx.recv().await.unwrap();
        assert!(matches!(
            event2,
            EngineEvent::PluginFinished {
                plugin_index: 0,
                result: Err(_),
                ..
            }
        ));
    }

    #[tokio::test]
    async fn execute_all_sends_prefetch_source() {
        let (tx, mut rx) = mpsc::channel(8);
        let engine = PluginEngine::new(vec![Arc::new(MockPlugin(test_metadata()))], tx);
        engine.execute_all();

        let event1 = rx.recv().await.unwrap();
        assert!(matches!(
            event1,
            EngineEvent::PluginStarted {
                plugin_index: 0,
                source: ExecutionSource::Prefetch
            }
        ));

        let event2 = rx.recv().await.unwrap();
        assert!(matches!(
            event2,
            EngineEvent::PluginFinished {
                plugin_index: 0,
                result: Ok(_),
                source: ExecutionSource::Prefetch
            }
        ));
    }
}
