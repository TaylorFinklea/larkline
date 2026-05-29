//! Plugin system — traits, registry, and execution engine.
//!
//! The [`Plugin`] trait is the central abstraction. All plugin backends
//! (script-based for v0.1, Lua-based for v1.0) implement this trait.
//! The rest of the application only talks to the trait, never to backends directly.

pub mod engine;
pub mod lua;
pub mod registry;
pub mod script;
pub mod store;
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

/// Resolve a plugin by display name for name-based addressing (`lark invoke`,
/// `lark.invoke`).
///
/// Supports a qualified `"Group:Command"` form so the same command name in
/// different plugin groups can be reached unambiguously (e.g. `Mail:Inbox`
/// vs `Harness Deck:Inbox`). A bare name that matches exactly one plugin
/// resolves directly; a bare name matching several returns an error listing
/// the qualified alternatives instead of silently picking the first (which
/// left every shadowed command unreachable). Returns a ready-to-display
/// error string so both the CLI (anyhow) and Lua (`LuaError`) callers can wrap
/// it uniformly.
pub fn resolve_plugin<'a>(
    plugins: &'a [Arc<dyn Plugin>],
    query: &str,
) -> Result<&'a Arc<dyn Plugin>, String> {
    // Qualified "Group:Command": disambiguate across plugin groups.
    if let Some((group, cmd)) = query.split_once(':') {
        let (group, cmd) = (group.trim(), cmd.trim());
        return plugins
            .iter()
            .find(|p| {
                let m = p.metadata();
                m.name == cmd && m.plugin_group.as_deref() == Some(group)
            })
            .ok_or_else(|| format!("plugin not found: no command {cmd:?} in group {group:?}"));
    }

    let matches: Vec<&'a Arc<dyn Plugin>> = plugins
        .iter()
        .filter(|p| p.metadata().name == query)
        .collect();
    match matches.as_slice() {
        [] => Err(format!("plugin not found: {query}")),
        [only] => Ok(only),
        many => {
            let mut alts: Vec<String> = many
                .iter()
                .map(|p| {
                    let m = p.metadata();
                    m.plugin_group
                        .as_deref()
                        .map_or_else(|| m.name.clone(), |g| format!("{g}:{}", m.name))
                })
                .collect();
            alts.sort();
            Err(format!(
                "ambiguous plugin name {query:?}; matches {} — qualify it as Group:Command",
                alts.join(", ")
            ))
        }
    }
}

#[cfg(test)]
mod resolve_tests {
    use super::*;
    use crate::plugin::traits::{PluginError, PluginMetadata, PluginOutput};
    use async_trait::async_trait;
    use std::time::Duration;

    #[derive(Debug)]
    struct Stub(PluginMetadata);
    #[async_trait]
    impl Plugin for Stub {
        fn metadata(&self) -> &PluginMetadata {
            &self.0
        }
        async fn execute(&self) -> Result<PluginOutput, PluginError> {
            Ok(PluginOutput::default())
        }
    }

    fn meta(name: &str, group: Option<&str>) -> PluginMetadata {
        PluginMetadata {
            name: name.into(),
            description: String::new(),
            version: "0".into(),
            author: String::new(),
            icon: "x".into(),
            icon_nerd: None,
            category: None,
            keybinding: None,
            timeout: Duration::from_secs(1),
            streaming: false,
            entry_path: None,
            prefetch: false,
            plugin_group: group.map(String::from),
            quickkey: None,
            cache: false,
            secrets: vec![],
            settings_spec: vec![],
            widget: false,
            widget_refresh_secs: 0,
            mini_app: false,
            agent_callable: false,
            destructive: false,
        }
    }

    fn plugins(metas: Vec<PluginMetadata>) -> Vec<Arc<dyn Plugin>> {
        metas
            .into_iter()
            .map(|m| Arc::new(Stub(m)) as Arc<dyn Plugin>)
            .collect()
    }

    #[test]
    fn unique_name_resolves() {
        let ps = plugins(vec![meta("Weather", None)]);
        assert!(resolve_plugin(&ps, "Weather").is_ok());
        assert!(resolve_plugin(&ps, "Nope").is_err());
    }

    #[test]
    fn ambiguous_bare_name_errors_with_qualified_alternatives() {
        let ps = plugins(vec![
            meta("Inbox", Some("Mail")),
            meta("Inbox", Some("Harness Deck")),
        ]);
        let Err(err) = resolve_plugin(&ps, "Inbox") else {
            panic!("expected ambiguous error");
        };
        assert!(err.contains("ambiguous"));
        assert!(err.contains("Mail:Inbox"));
        assert!(err.contains("Harness Deck:Inbox"));
    }

    #[test]
    fn group_qualified_form_disambiguates() {
        let ps = plugins(vec![
            meta("Inbox", Some("Mail")),
            meta("Inbox", Some("Harness Deck")),
        ]);
        let resolved = resolve_plugin(&ps, "Mail:Inbox").unwrap();
        assert_eq!(resolved.metadata().plugin_group.as_deref(), Some("Mail"));
    }
}
