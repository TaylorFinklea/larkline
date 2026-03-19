//! Plugin execution engine — dispatches plugins as Tokio tasks and sends events back
//! to the app run loop via an `mpsc` channel.

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::plugin::traits::{Plugin, PluginError, PluginOutput};

/// Events sent from the engine to the app run loop.
#[derive(Debug)]
pub enum EngineEvent {
    /// A plugin has started executing.
    PluginStarted {
        /// Index into the engine's plugin list.
        /// Useful for multi-plugin dispatch in future phases.
        #[allow(dead_code)]
        plugin_index: usize,
    },
    /// A plugin has finished (successfully or with an error).
    PluginFinished {
        /// Index into the engine's plugin list.
        #[allow(dead_code)]
        plugin_index: usize,
        /// The execution result.
        result: Result<PluginOutput, PluginError>,
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

    /// Spawn plugin execution on a Tokio task. Returns immediately.
    ///
    /// Events are sent to the channel: `PluginStarted`, then `PluginFinished`.
    ///
    /// Uses an outer/inner task pattern so panics in the plugin are caught by the
    /// `JoinHandle` and converted to a `PluginError`, ensuring `PluginFinished` is
    /// always sent even when the plugin task panics.
    pub fn execute(&self, plugin_index: usize) {
        let plugin = Arc::clone(&self.plugins[plugin_index]);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(EngineEvent::PluginStarted { plugin_index }).await;
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
            category: None,
            keybinding: None,
            timeout: std::time::Duration::from_secs(5),
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
            EngineEvent::PluginStarted { plugin_index: 0 }
        ));

        let event2 = rx.recv().await.unwrap();
        assert!(matches!(
            event2,
            EngineEvent::PluginFinished {
                plugin_index: 0,
                result: Ok(_)
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
            EngineEvent::PluginStarted { plugin_index: 0 }
        ));

        let event2 = rx.recv().await.unwrap();
        assert!(matches!(
            event2,
            EngineEvent::PluginFinished {
                plugin_index: 0,
                result: Err(_)
            }
        ));
    }
}
