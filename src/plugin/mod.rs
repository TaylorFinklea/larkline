//! Plugin system — traits, registry, and execution engine.
//!
//! The [`Plugin`] trait is the central abstraction. All plugin backends
//! (script-based for v0.1, Lua-based for v1.0) implement this trait.
//! The rest of the application only talks to the trait, never to backends directly.

pub mod engine;
pub mod lua;
pub mod registry;
pub mod script;
pub mod traits;

// Re-export the types most commonly needed by the rest of the application.
// Phase 2: types used by engine/script/registry sub-modules wired in Task 6.
#[allow(unused_imports)]
pub use traits::{
    ActionKind, ItemAction, OutputItem, Plugin, PluginError, PluginMetadata, PluginOutput,
};

use std::sync::Arc;

use registry::{DiscoveredPlugin, PluginKind};

/// Construct the correct [`Plugin`] backend for a discovered plugin.
#[must_use]
pub fn build_plugin(discovered: DiscoveredPlugin) -> Arc<dyn Plugin> {
    match discovered.kind {
        PluginKind::Script => Arc::new(script::ScriptPlugin::from_discovered(discovered)),
        PluginKind::Lua => Arc::new(lua::LuaPlugin::from_discovered(discovered)),
    }
}
