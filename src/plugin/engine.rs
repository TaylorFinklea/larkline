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
    /// Marks plugin tasks whose panics are caught and converted to
    /// [`PluginError`], so the process-level hook leaves the active TUI alone.
    pub static PLUGIN_PANIC_ISOLATED: ();
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

#[derive(Debug, Clone, Copy)]
enum Invocation {
    Normal,
    Streaming,
    FormSubmission,
    ActionCallback,
}

impl Invocation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Streaming => "streaming",
            Self::FormSubmission => "form_submission",
            Self::ActionCallback => "action_callback",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct PluginTiming<'a> {
    plugin: &'a str,
    invocation: &'static str,
    source: &'static str,
    execution_id: u64,
    elapsed_ms: u128,
}

impl<'a> PluginTiming<'a> {
    fn new(
        plugin: &'a str,
        invocation: Invocation,
        source: ExecutionSource,
        stamp: ExecutionStamp,
        elapsed: std::time::Duration,
    ) -> Self {
        let source = match source {
            ExecutionSource::UserSelected => "user_selected",
            ExecutionSource::Prefetch => "prefetch",
        };
        Self {
            plugin,
            invocation: invocation.as_str(),
            source,
            execution_id: stamp.execution_id,
            elapsed_ms: elapsed.as_millis(),
        }
    }

    fn is_slow(&self) -> bool {
        self.elapsed_ms >= SLOW_PLUGIN_THRESHOLD.as_millis()
    }
}

fn log_execution_time(
    name: &str,
    invocation: Invocation,
    source: ExecutionSource,
    stamp: ExecutionStamp,
    elapsed: std::time::Duration,
) {
    let timing = PluginTiming::new(name, invocation, source, stamp, elapsed);
    if timing.is_slow() {
        tracing::info!(
            plugin = %timing.plugin,
            invocation = timing.invocation,
            source = timing.source,
            execution_id = timing.execution_id,
            elapsed_ms = timing.elapsed_ms,
            "slow plugin execution"
        );
    } else {
        tracing::debug!(
            plugin = %timing.plugin,
            invocation = timing.invocation,
            source = timing.source,
            execution_id = timing.execution_id,
            elapsed_ms = timing.elapsed_ms,
            "plugin executed"
        );
    }
}

fn log_normal_execution_time(
    name: &str,
    source: ExecutionSource,
    stamp: ExecutionStamp,
    elapsed: std::time::Duration,
) {
    log_execution_time(name, Invocation::Normal, source, stamp, elapsed);
}

fn log_streaming_execution_time(
    name: &str,
    source: ExecutionSource,
    stamp: ExecutionStamp,
    elapsed: std::time::Duration,
) {
    log_execution_time(name, Invocation::Streaming, source, stamp, elapsed);
}

fn log_form_execution_time(name: &str, stamp: ExecutionStamp, elapsed: std::time::Duration) {
    log_execution_time(
        name,
        Invocation::FormSubmission,
        ExecutionSource::UserSelected,
        stamp,
        elapsed,
    );
}

fn log_action_execution_time(name: &str, stamp: ExecutionStamp, elapsed: std::time::Duration) {
    log_execution_time(
        name,
        Invocation::ActionCallback,
        ExecutionSource::UserSelected,
        stamp,
        elapsed,
    );
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
        // Streaming routes through Plugin::execute_streaming — backends
        // without native streaming (Lua, native Rust) fall back to their
        // normal execute() via the trait default, in the VM/in-process.
        if meta.streaming {
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
        let plugin_name = plugin.metadata().name.clone();
        let all_plugins = Arc::new(self.plugins.clone());
        let secrets = self.secrets.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let started = std::time::Instant::now();
            let handle = tokio::spawn(PLUGIN_PANIC_ISOLATED.scope(
                (),
                SECRETS.scope(
                    secrets,
                    PLUGIN_LIST.scope(
                        all_plugins,
                        INVOKE_DEPTH.scope(0, async move {
                            plugin.execute_action(&callback_id, &context).await
                        }),
                    ),
                ),
            ));
            let result = match handle.await {
                Ok(r) => r,
                Err(join_err) => Err(PluginError::ExecutionFailed(format!(
                    "action task failed: {join_err}"
                ))),
            };
            log_action_execution_time(&plugin_name, stamp, started.elapsed());
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
        let plugin_name = plugin.metadata().name.clone();
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
            let started = std::time::Instant::now();
            let handle = tokio::spawn(PLUGIN_PANIC_ISOLATED.scope(
                (),
                SECRETS.scope(
                    secrets,
                    PLUGIN_LIST.scope(
                        all_plugins,
                        INVOKE_DEPTH.scope(0, async move {
                            plugin.execute_with_form(form_values).await
                        }),
                    ),
                ),
            ));
            let result = match handle.await {
                Ok(r) => r,
                Err(join_err) => Err(PluginError::ExecutionFailed(format!(
                    "plugin task failed: {join_err}"
                ))),
            };
            log_form_execution_time(&plugin_name, stamp, started.elapsed());
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
            let handle = tokio::spawn(PLUGIN_PANIC_ISOLATED.scope(
                (),
                SECRETS.scope(
                    secrets,
                    PLUGIN_LIST.scope(
                        all_plugins,
                        INVOKE_DEPTH.scope(0, async move { plugin.execute().await }),
                    ),
                ),
            ));
            let result = match handle.await {
                Ok(r) => r,
                Err(join_err) => Err(PluginError::ExecutionFailed(format!(
                    "plugin task failed: {join_err}"
                ))),
            };
            log_normal_execution_time(&plugin_name, source, stamp, started.elapsed());
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

    /// Streaming execution — runs [`Plugin::execute_streaming`] inside the
    /// same task-local scaffolding as [`execute_normal`](Self::execute_normal)
    /// (secrets, plugin list, invoke depth, panic isolation) and forwards
    /// its chunks as [`EngineEvent::PartialOutput`]s. `PluginFinished` is
    /// sent only after every chunk has been forwarded, so the app's
    /// accumulated streamed output is complete when the finish lands.
    fn execute_streaming(
        &self,
        plugin_index: usize,
        source: ExecutionSource,
        stamp: ExecutionStamp,
    ) {
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

            let (chunk_tx, mut chunk_rx) = mpsc::channel::<crate::plugin::traits::StreamChunk>(32);
            let forwarder = {
                let tx = tx.clone();
                tokio::spawn(async move {
                    while let Some(chunk) = chunk_rx.recv().await {
                        let _ = tx
                            .send(EngineEvent::PartialOutput {
                                plugin_index,
                                stamp,
                                title: chunk.title,
                                items: chunk.items,
                                source,
                            })
                            .await;
                    }
                })
            };

            let handle = tokio::spawn(
                PLUGIN_PANIC_ISOLATED.scope(
                    (),
                    SECRETS.scope(
                        secrets,
                        PLUGIN_LIST.scope(
                            all_plugins,
                            INVOKE_DEPTH
                                .scope(0, async move { plugin.execute_streaming(chunk_tx).await }),
                        ),
                    ),
                ),
            );
            let result = match handle.await {
                Ok(r) => r,
                Err(join_err) => Err(PluginError::ExecutionFailed(format!(
                    "plugin task failed: {join_err}"
                ))),
            };
            // The plugin future owns the only chunk sender; it is dropped by
            // now (returned or panicked), so the forwarder drains and ends.
            let _ = forwarder.await;
            log_streaming_execution_time(&plugin_name, source, stamp, started.elapsed());
            let _ = tx
                .send(EngineEvent::PluginFinished {
                    plugin_index,
                    stamp,
                    result,
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

    #[test]
    fn plugin_timing_is_slow_at_five_hundred_milliseconds() {
        let timing = PluginTiming::new(
            "test",
            Invocation::Normal,
            ExecutionSource::UserSelected,
            ExecutionStamp {
                registry_generation: 3,
                execution_id: 42,
            },
            std::time::Duration::from_millis(500),
        );

        assert!(timing.is_slow());
    }

    #[test]
    fn plugin_timing_is_normal_below_five_hundred_milliseconds() {
        let timing = PluginTiming::new(
            "test",
            Invocation::Normal,
            ExecutionSource::UserSelected,
            ExecutionStamp {
                registry_generation: 3,
                execution_id: 42,
            },
            std::time::Duration::from_millis(499),
        );

        assert!(!timing.is_slow());
    }

    #[test]
    fn plugin_timing_prepares_required_metadata() {
        let timing = PluginTiming::new(
            "test",
            Invocation::FormSubmission,
            ExecutionSource::UserSelected,
            ExecutionStamp {
                registry_generation: 3,
                execution_id: 42,
            },
            std::time::Duration::from_millis(17),
        );

        assert_eq!(
            timing,
            PluginTiming {
                plugin: "test",
                invocation: "form_submission",
                source: "user_selected",
                execution_id: 42,
                elapsed_ms: 17,
            }
        );
    }

    fn stamp(execution_id: u64) -> ExecutionStamp {
        ExecutionStamp {
            registry_generation: 3,
            execution_id,
        }
    }

    #[test]
    fn emitted_timing_normal_dispatch_has_exact_debug_contract() {
        let event = crate::test_tracing::capture_event(|| {
            log_normal_execution_time(
                "normal-plugin",
                ExecutionSource::Prefetch,
                stamp(41),
                std::time::Duration::from_millis(499),
            );
        });

        assert_eq!(
            event,
            crate::test_tracing::CapturedEvent::new(
                tracing::Level::DEBUG,
                [
                    ("message", "plugin executed"),
                    ("plugin", "normal-plugin"),
                    ("invocation", "normal"),
                    ("source", "prefetch"),
                    ("execution_id", "41"),
                    ("elapsed_ms", "499"),
                ],
            )
        );
    }

    #[test]
    fn emitted_timing_streaming_dispatch_has_exact_info_contract() {
        let event = crate::test_tracing::capture_event(|| {
            log_streaming_execution_time(
                "streaming-plugin",
                ExecutionSource::UserSelected,
                stamp(42),
                std::time::Duration::from_millis(500),
            );
        });

        assert_eq!(
            event,
            crate::test_tracing::CapturedEvent::new(
                tracing::Level::INFO,
                [
                    ("message", "slow plugin execution"),
                    ("plugin", "streaming-plugin"),
                    ("invocation", "streaming"),
                    ("source", "user_selected"),
                    ("execution_id", "42"),
                    ("elapsed_ms", "500"),
                ],
            )
        );
    }

    #[test]
    fn emitted_timing_form_dispatch_has_exact_debug_contract() {
        let event = crate::test_tracing::capture_event(|| {
            log_form_execution_time(
                "form-plugin",
                stamp(43),
                std::time::Duration::from_millis(17),
            );
        });

        assert_eq!(
            event,
            crate::test_tracing::CapturedEvent::new(
                tracing::Level::DEBUG,
                [
                    ("message", "plugin executed"),
                    ("plugin", "form-plugin"),
                    ("invocation", "form_submission"),
                    ("source", "user_selected"),
                    ("execution_id", "43"),
                    ("elapsed_ms", "17"),
                ],
            )
        );
    }

    #[test]
    fn emitted_timing_action_dispatch_has_exact_info_contract() {
        let event = crate::test_tracing::capture_event(|| {
            log_action_execution_time(
                "action-plugin",
                stamp(44),
                std::time::Duration::from_millis(700),
            );
        });

        assert_eq!(
            event,
            crate::test_tracing::CapturedEvent::new(
                tracing::Level::INFO,
                [
                    ("message", "slow plugin execution"),
                    ("plugin", "action-plugin"),
                    ("invocation", "action_callback"),
                    ("source", "user_selected"),
                    ("execution_id", "44"),
                    ("elapsed_ms", "700"),
                ],
            )
        );
    }

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

    /// A plugin with native streaming: emits a header chunk and one item
    /// chunk through the channel, then finishes.
    struct StreamingMockPlugin(PluginMetadata);

    #[async_trait::async_trait]
    impl Plugin for StreamingMockPlugin {
        fn metadata(&self) -> &PluginMetadata {
            &self.0
        }
        async fn execute(&self) -> Result<PluginOutput, PluginError> {
            Err(PluginError::ExecutionFailed(
                "streaming plugin must run via execute_streaming".into(),
            ))
        }
        async fn execute_streaming(
            &self,
            partial: mpsc::Sender<crate::plugin::traits::StreamChunk>,
        ) -> Result<PluginOutput, PluginError> {
            let _ = partial
                .send(crate::plugin::traits::StreamChunk {
                    title: Some("stream".into()),
                    items: vec![OutputItem {
                        label: "first".into(),
                        ..Default::default()
                    }],
                })
                .await;
            let _ = partial
                .send(crate::plugin::traits::StreamChunk {
                    title: None,
                    items: vec![OutputItem {
                        label: "second".into(),
                        ..Default::default()
                    }],
                })
                .await;
            Ok(PluginOutput::default())
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

    /// Build a real `ScriptPlugin` with `streaming = true` from a tempdir
    /// fixture. The tempdir is leaked to keep the script alive (test only).
    fn streaming_script_plugin(name: &str, script: &str) -> crate::plugin::script::ScriptPlugin {
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
        let discovered = crate::plugin::registry::parse_manifest(&plugin_dir)
            .unwrap()
            .remove(0);
        std::mem::forget(dir);
        crate::plugin::script::ScriptPlugin::from_discovered(discovered)
    }

    /// Drain events until `PluginFinished` arrives, returning its result and
    /// any `PartialOutput` payloads seen on the way.
    async fn collect_run(
        rx: &mut mpsc::Receiver<EngineEvent>,
    ) -> (
        Vec<(Option<String>, Vec<OutputItem>)>,
        Result<PluginOutput, PluginError>,
    ) {
        let mut partials = Vec::new();
        loop {
            match rx.recv().await.expect("engine event") {
                EngineEvent::PluginFinished { result, .. } => return (partials, result),
                EngineEvent::PartialOutput { title, items, .. } => partials.push((title, items)),
                EngineEvent::PluginStarted { .. } | EngineEvent::ActionResult { .. } => {}
            }
        }
    }

    #[tokio::test]
    async fn streaming_flag_without_native_streaming_runs_through_the_trait() {
        // A streaming-marked plugin whose backend has no native streaming
        // (e.g. a Lua plugin) must run via the Plugin trait — NOT be exec'd
        // as an OS process from its entry_path.
        let mut meta = test_metadata();
        meta.streaming = true;
        meta.entry_path = Some(std::path::PathBuf::from("/nonexistent/init.lua"));
        let (tx, mut rx) = mpsc::channel(8);
        let engine = PluginEngine::new(
            vec![Arc::new(MockPlugin(meta))],
            tx,
            std::collections::HashMap::new(),
            0,
        );
        let _stamp = engine.execute(0);

        let (_, result) = collect_run(&mut rx).await;
        assert_eq!(
            result
                .expect("must run the trait's execute, not exec init.lua")
                .title,
            "mock"
        );
    }

    #[tokio::test]
    async fn native_streaming_chunks_forward_as_partial_output() {
        let mut meta = test_metadata();
        meta.streaming = true;
        let (tx, mut rx) = mpsc::channel(8);
        let engine = PluginEngine::new(
            vec![Arc::new(StreamingMockPlugin(meta))],
            tx,
            std::collections::HashMap::new(),
            0,
        );
        let _stamp = engine.execute(0);

        let (partials, result) = collect_run(&mut rx).await;
        assert!(result.is_ok());
        assert_eq!(
            partials.len(),
            2,
            "both chunks must arrive as PartialOutput"
        );
        assert_eq!(partials[0].0.as_deref(), Some("stream"));
        assert_eq!(partials[0].1[0].label, "first");
        assert_eq!(partials[1].1[0].label, "second");
    }

    #[tokio::test]
    async fn failing_streaming_script_surfaces_error_not_empty_success() {
        let plugin = streaming_script_plugin(
            "stream-fail",
            "#!/bin/sh\necho '{\"title\":\"s\",\"items\":[]}'\necho 'boom' >&2\nexit 3\n",
        );
        let (tx, mut rx) = mpsc::channel(8);
        let engine = PluginEngine::new(
            vec![Arc::new(plugin)],
            tx,
            std::collections::HashMap::new(),
            0,
        );
        let _stamp = engine.execute(0);

        let (_, result) = collect_run(&mut rx).await;
        let error = result.expect_err("non-zero exit must fail, not report empty success");
        let message = error.to_string();
        assert!(
            message.contains("exit code") && message.contains("boom"),
            "exit status and stderr must be surfaced, got: {message}"
        );
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
