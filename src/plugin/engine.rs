//! Plugin execution engine — dispatches plugins as Tokio tasks and sends events back
//! to the app run loop via an `mpsc` channel.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::mpsc;

use crate::plugin::traits::{OutputItem, Plugin, PluginError, PluginOutput};

tokio::task_local! {
    /// Current invocation depth for recursion guarding in `lark.invoke()`.
    pub static INVOKE_DEPTH: u32;
    /// Full plugin list for inter-plugin invocation via `lark.invoke()`.
    pub static PLUGIN_LIST: Arc<Vec<Arc<dyn Plugin>>>;
    /// Secrets loaded from `~/.config/larkline/.env`.
    pub static SECRETS: Arc<std::collections::HashMap<String, String>>;
    /// Cancellation token observed by long-running plugin code. Lua
    /// plugins poll it via `lark.is_cancelled()`; Rust code can `await`
    /// `.cancelled()` directly. Outside the agent loop this stays
    /// un-cancelled forever — the existing TUI/CLI paths use a fresh
    /// `CancellationToken::new()` per execution.
    ///
    /// Design note (Phase 7): we use a task-local rather than a
    /// `Plugin::execute(&self, cancel)` trait parameter because it
    /// achieves the same outcome (Lua poll → early-exit) without
    /// touching every existing plugin impl. Departs from the literal
    /// "trait change" wording in ADR-008 in favor of the same outcome
    /// with one-tenth the surface area. See ADR-009.
    pub static CANCEL_TOKEN: tokio_util::sync::CancellationToken;
}

/// Indicates whether a plugin execution was triggered by the user or by prefetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionSource {
    /// The user explicitly selected this plugin.
    UserSelected,
    /// The plugin was executed in the background on startup (prefetch).
    Prefetch,
}

/// Identity shared by every event emitted for one engine dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionStamp {
    /// Registry snapshot against which the plugin index was resolved.
    pub registry_generation: u64,
    /// Process-wide monotonic dispatch identifier.
    pub execution_id: u64,
}

static NEXT_EXECUTION_ID: AtomicU64 = AtomicU64::new(1);

/// Events sent from the engine to the app run loop.
#[derive(Debug)]
pub enum EngineEvent {
    /// A plugin has started executing.
    PluginStarted {
        /// Index into the engine's plugin list.
        plugin_index: usize,
        /// Identity of this dispatch.
        stamp: ExecutionStamp,
        /// Whether this is a user-triggered or prefetch execution.
        source: ExecutionSource,
    },
    /// A plugin has finished (successfully or with an error).
    PluginFinished {
        /// Index into the engine's plugin list.
        plugin_index: usize,
        /// Identity of this dispatch.
        stamp: ExecutionStamp,
        /// The execution result.
        result: Result<PluginOutput, PluginError>,
        /// Whether this is a user-triggered or prefetch execution.
        source: ExecutionSource,
    },
    /// Incremental output from a streaming plugin.
    PartialOutput {
        /// Index into the engine's plugin list.
        plugin_index: usize,
        /// Identity of this dispatch.
        stamp: ExecutionStamp,
        /// Title (set only on the first partial).
        title: Option<String>,
        /// Items to append to the output.
        items: Vec<OutputItem>,
        /// Whether this is a user-triggered or prefetch execution.
        source: ExecutionSource,
    },
    /// Result of an `on_action` callback (action chaining).
    ActionResult {
        /// Index into the engine's plugin list.
        plugin_index: usize,
        /// Identity of this dispatch.
        stamp: ExecutionStamp,
        /// The updated output from the action callback.
        result: Result<PluginOutput, PluginError>,
    },
}

/// Maximum concurrent prefetch plugin executions.
///
/// Startup fires `execute_all()` which enqueues every prefetch-eligible plugin.
/// Without a cap, 40+ shell/HTTP/docker commands run in parallel and saturate
/// the system on slow machines. User-triggered executions bypass this limit.
const PREFETCH_CONCURRENCY: usize = 8;

/// Plugins slower than this threshold log at `info` for profiling.
///
/// Kept below the default `warn` level: slow plugins are routine (network
/// calls, docker, github), and any writer that reaches stderr during the TUI
/// session corrupts the screen. Users opt in via `RUST_LOG=info` or the
/// `logging.level` config.
const SLOW_PLUGIN_THRESHOLD: std::time::Duration = std::time::Duration::from_millis(500);

fn log_execution_time(name: &str, elapsed: std::time::Duration) {
    if elapsed >= SLOW_PLUGIN_THRESHOLD {
        tracing::info!(
            plugin = %name,
            elapsed_ms = elapsed.as_millis(),
            "slow plugin execution"
        );
    } else {
        tracing::debug!(
            plugin = %name,
            elapsed_ms = elapsed.as_millis(),
            "plugin executed"
        );
    }
}

/// Manages a set of plugins and dispatches them as async Tokio tasks.
pub struct PluginEngine {
    plugins: Vec<Arc<dyn Plugin>>,
    tx: mpsc::Sender<EngineEvent>,
    secrets: Arc<std::collections::HashMap<String, String>>,
    registry_generation: u64,
    /// Limits concurrent prefetch executions; user-selected runs bypass it.
    prefetch_sem: Arc<tokio::sync::Semaphore>,
}

impl PluginEngine {
    /// Create a new `PluginEngine` with the given plugins, event sender, and secrets.
    #[must_use]
    pub fn new(
        plugins: Vec<Arc<dyn Plugin>>,
        tx: mpsc::Sender<EngineEvent>,
        secrets: std::collections::HashMap<String, String>,
        registry_generation: u64,
    ) -> Self {
        Self {
            plugins,
            tx,
            secrets: Arc::new(secrets),
            registry_generation,
            prefetch_sem: Arc::new(tokio::sync::Semaphore::new(PREFETCH_CONCURRENCY)),
        }
    }

    fn next_execution_stamp(&self) -> ExecutionStamp {
        ExecutionStamp {
            registry_generation: self.registry_generation,
            execution_id: NEXT_EXECUTION_ID.fetch_add(1, Ordering::Relaxed),
        }
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
    #[must_use]
    pub fn execute(&self, plugin_index: usize) -> ExecutionStamp {
        self.execute_with_source(plugin_index, ExecutionSource::UserSelected)
    }

    /// Re-run a plugin in the BACKGROUND (prefetch source): the result updates
    /// the result cache only and must NOT take over the foreground view. Used
    /// by the widget/glance-strip auto-refresh ticks — `execute()` would emit
    /// `UserSelected` events that yank the main pane to the refreshed plugin.
    #[must_use]
    pub fn refresh(&self, plugin_index: usize) -> ExecutionStamp {
        self.execute_with_source(plugin_index, ExecutionSource::Prefetch)
    }

    /// Execute all prefetch-eligible plugins in the background.
    ///
    /// Called on startup and after refresh. Only runs plugins with `prefetch == true`.
    /// No-ops if called outside of a Tokio runtime context (e.g., from sync tests).
    #[must_use]
    pub fn execute_all(&self) -> Vec<(usize, ExecutionStamp)> {
        if tokio::runtime::Handle::try_current().is_err() {
            return Vec::new();
        }
        let mut executions = Vec::new();
        for i in 0..self.plugins.len() {
            let meta = self.plugins[i].metadata();
            if meta.prefetch || meta.widget || meta.status {
                let stamp = self.execute_with_source(i, ExecutionSource::Prefetch);
                executions.push((i, stamp));
            }
        }
        executions
    }

    /// Internal dispatch — routes to streaming or normal execution with the given source.
    fn execute_with_source(&self, plugin_index: usize, source: ExecutionSource) -> ExecutionStamp {
        let stamp = self.next_execution_stamp();
        let meta = self.plugins[plugin_index].metadata();
        if meta.streaming && meta.entry_path.is_some() {
            self.execute_streaming(plugin_index, source, stamp);
        } else {
            self.execute_normal(plugin_index, source, stamp);
        }
        stamp
    }

    /// Execute a plugin's `on_action` callback for action chaining.
    ///
    /// Spawns a Tokio task that calls `Plugin::execute_action()` and sends
    /// an `EngineEvent::ActionResult` when complete.
    #[must_use]
    pub fn execute_action(
        &self,
        plugin_index: usize,
        callback_id: String,
        context: String,
    ) -> ExecutionStamp {
        let stamp = self.next_execution_stamp();
        let plugin = Arc::clone(&self.plugins[plugin_index]);
        let all_plugins = Arc::new(self.plugins.clone());
        let secrets = self.secrets.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let handle = tokio::spawn(SECRETS.scope(
                secrets,
                PLUGIN_LIST.scope(
                    all_plugins,
                    INVOKE_DEPTH.scope(0, async move {
                        plugin.execute_action(&callback_id, &context).await
                    }),
                ),
            ));
            let result = match handle.await {
                Ok(r) => r,
                Err(join_err) => Err(PluginError::ExecutionFailed(format!(
                    "action task failed: {join_err}"
                ))),
            };
            let _ = tx
                .send(EngineEvent::ActionResult {
                    plugin_index,
                    stamp,
                    result,
                })
                .await;
        });
        stamp
    }

    /// Execute a plugin with form values. The plugin's `execute_with_form()` receives
    /// the collected values, which Lua/Script backends use to inject form context.
    #[must_use]
    pub fn execute_with_form(
        &self,
        plugin_index: usize,
        form_values: std::collections::HashMap<String, String>,
    ) -> ExecutionStamp {
        let stamp = self.next_execution_stamp();
        let plugin = Arc::clone(&self.plugins[plugin_index]);
        let all_plugins = Arc::new(self.plugins.clone());
        let secrets = self.secrets.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx
                .send(EngineEvent::PluginStarted {
                    plugin_index,
                    stamp,
                    source: ExecutionSource::UserSelected,
                })
                .await;
            let handle = tokio::spawn(SECRETS.scope(
                secrets,
                PLUGIN_LIST.scope(
                    all_plugins,
                    INVOKE_DEPTH.scope(
                        0,
                        async move { plugin.execute_with_form(form_values).await },
                    ),
                ),
            ));
            let result = match handle.await {
                Ok(r) => r,
                Err(join_err) => Err(PluginError::ExecutionFailed(format!(
                    "plugin task failed: {join_err}"
                ))),
            };
            let _ = tx
                .send(EngineEvent::PluginFinished {
                    plugin_index,
                    stamp,
                    result,
                    source: ExecutionSource::UserSelected,
                })
                .await;
        });
        stamp
    }

    /// Normal (non-streaming) execution — waits for plugin to complete, then sends result.
    ///
    /// Uses an outer/inner task pattern so panics in the plugin are caught by the
    /// `JoinHandle` and converted to a `PluginError`, ensuring `PluginFinished` is
    /// always sent even when the plugin task panics.
    fn execute_normal(&self, plugin_index: usize, source: ExecutionSource, stamp: ExecutionStamp) {
        let plugin = Arc::clone(&self.plugins[plugin_index]);
        let all_plugins = Arc::new(self.plugins.clone());
        let secrets = self.secrets.clone();
        let tx = self.tx.clone();
        let prefetch_sem = Arc::clone(&self.prefetch_sem);
        let plugin_name = plugin.metadata().name.clone();
        tokio::spawn(async move {
            // Rate-limit background prefetch. User-selected runs bypass the cap.
            let _permit = match source {
                ExecutionSource::Prefetch => prefetch_sem.acquire_owned().await.ok(),
                ExecutionSource::UserSelected => None,
            };
            let _ = tx
                .send(EngineEvent::PluginStarted {
                    plugin_index,
                    stamp,
                    source,
                })
                .await;
            let started = std::time::Instant::now();
            let handle = tokio::spawn(SECRETS.scope(
                secrets,
                PLUGIN_LIST.scope(
                    all_plugins,
                    INVOKE_DEPTH.scope(0, async move { plugin.execute().await }),
                ),
            ));
            let result = match handle.await {
                Ok(r) => r,
                Err(join_err) => Err(PluginError::ExecutionFailed(format!(
                    "plugin task failed: {join_err}"
                ))),
            };
            log_execution_time(&plugin_name, started.elapsed());
            let _ = tx
                .send(EngineEvent::PluginFinished {
                    plugin_index,
                    stamp,
                    result,
                    source,
                })
                .await;
            // _permit drops here, releasing the prefetch slot.
        });
    }

    /// Streaming execution — reads stdout line-by-line and sends partial output events.
    ///
    /// First line is parsed as `PluginOutput` (header + initial items).
    /// Subsequent lines are parsed as individual `OutputItem`.
    /// Invalid lines are skipped with a warning.
    #[allow(clippy::too_many_lines)]
    fn execute_streaming(
        &self,
        plugin_index: usize,
        source: ExecutionSource,
        stamp: ExecutionStamp,
    ) {
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
        let store_path =
            crate::plugin::store::store_path_for(&meta.name, meta.plugin_group.as_deref());
        let tx = self.tx.clone();
        let prefetch_sem = Arc::clone(&self.prefetch_sem);

        tokio::spawn(async move {
            // Rate-limit background prefetch. User-selected runs bypass the cap.
            let _permit = match source {
                ExecutionSource::Prefetch => prefetch_sem.acquire_owned().await.ok(),
                ExecutionSource::UserSelected => None,
            };
            let _ = tx
                .send(EngineEvent::PluginStarted {
                    plugin_index,
                    stamp,
                    source,
                })
                .await;
            let started = std::time::Instant::now();

            let result = tokio::time::timeout(timeout, async {
                use tokio::io::{AsyncBufReadExt, BufReader};
                use tokio::process::Command;

                let mut child = match Command::new(&entry_path)
                    .current_dir(&plugin_dir)
                    .env("LARK_STORE_PATH", &store_path)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    // Reap the child if this timeout future is dropped mid-stream.
                    .kill_on_drop(true)
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
                                        stamp,
                                        title: Some(output.title),
                                        items: output.items,
                                        source,
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
                                        stamp,
                                        title: None,
                                        items: vec![item],
                                        source,
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
            log_execution_time(&meta.name, started.elapsed());
            let _ = tx
                .send(EngineEvent::PluginFinished {
                    plugin_index,
                    stamp,
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
            secrets: vec![],
            settings_spec: vec![],
            widget: false,
            widget_refresh_secs: 0,
            status: false,
            status_refresh_secs: 0,
            mini_app: false,
            agent_callable: false,
            destructive: false,
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
        let engine = PluginEngine::new(
            vec![Arc::new(MockPlugin(test_metadata()))],
            tx,
            std::collections::HashMap::new(),
            0,
        );
        let stamp = engine.execute(0);

        let event1 = rx.recv().await.unwrap();
        assert!(matches!(
            event1,
            EngineEvent::PluginStarted {
                plugin_index: 0,
                stamp: event_stamp,
                source: ExecutionSource::UserSelected,
                ..
            } if event_stamp == stamp
        ));

        let event2 = rx.recv().await.unwrap();
        assert!(matches!(
            event2,
            EngineEvent::PluginFinished {
                plugin_index: 0,
                stamp: event_stamp,
                result: Ok(_),
                source: ExecutionSource::UserSelected,
                ..
            } if event_stamp == stamp
        ));
    }

    #[tokio::test]
    async fn dispatches_get_monotonic_execution_ids() {
        let (tx, _rx) = mpsc::channel(4);
        let engine = PluginEngine::new(
            vec![Arc::new(MockPlugin(test_metadata()))],
            tx,
            std::collections::HashMap::new(),
            7,
        );

        let first = engine.execute(0);
        let second = engine.execute(0);

        assert!(second.execution_id > first.execution_id);
    }

    #[tokio::test]
    async fn dispatch_stamp_carries_registry_generation() {
        let (tx, _rx) = mpsc::channel(2);
        let engine = PluginEngine::new(
            vec![Arc::new(MockPlugin(test_metadata()))],
            tx,
            std::collections::HashMap::new(),
            7,
        );

        let stamp = engine.execute(0);

        assert_eq!(stamp.registry_generation, 7);
    }

    #[tokio::test]
    async fn propagates_plugin_error() {
        let (tx, mut rx) = mpsc::channel(4);
        let engine = PluginEngine::new(
            vec![Arc::new(FailPlugin(test_metadata()))],
            tx,
            std::collections::HashMap::new(),
            0,
        );
        let _stamp = engine.execute(0);

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
        let engine = PluginEngine::new(
            vec![Arc::new(PanicPlugin(test_metadata()))],
            tx,
            std::collections::HashMap::new(),
            0,
        );
        let _stamp = engine.execute(0);

        let event1 = rx.recv().await.unwrap();
        assert!(matches!(
            event1,
            EngineEvent::PluginStarted {
                plugin_index: 0,
                source: ExecutionSource::UserSelected,
                ..
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
        let engine = PluginEngine::new(
            vec![Arc::new(MockPlugin(test_metadata()))],
            tx,
            std::collections::HashMap::new(),
            0,
        );
        let _executions = engine.execute_all();

        let event1 = rx.recv().await.unwrap();
        assert!(matches!(
            event1,
            EngineEvent::PluginStarted {
                plugin_index: 0,
                source: ExecutionSource::Prefetch,
                ..
            }
        ));

        let event2 = rx.recv().await.unwrap();
        assert!(matches!(
            event2,
            EngineEvent::PluginFinished {
                plugin_index: 0,
                result: Ok(_),
                source: ExecutionSource::Prefetch,
                ..
            }
        ));
    }
}
