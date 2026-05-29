//! Tool registry — derives `Vec<ToolDefinition>` from discovered plugins.
//!
//! The agent loop (Phase 8.B) takes this list and hands it to the
//! provider in [`AskRequest::tools`]. Each agent-callable command in
//! every manifest becomes one tool the model can call.
//!
//! Tool naming convention is `{plugin_id}__{command_id}` so the
//! dispatcher in Phase 8.B can route the call back to the right
//! plugin + command by splitting on `"__"`.
//!
//! Input schema for v1.0 is `{}` (no arguments) — the agent calls
//! plugins zero-arg and the plugin reads any state it needs from
//! `lark.store` or the conversation context. v1.x will derive a
//! schema from `settings_spec` so the model can pass form values.
//!
//! Decision: ADR-008 + ADR-009 (see `.docs/ai/decisions.md`). Prior
//! art: pi-mono's `pi-agent-core/src/agent.ts` `AgentTool` type.

use crate::agent::provider::ToolDefinition;
use crate::plugin::traits::Plugin;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

/// Build the agent tool registry AND the dispatch lookup from a single
/// deduplicated pass, so the tools advertised to the provider and the
/// dispatcher's routing table always agree 1:1.
///
/// Only plugins with `agent_callable = true` are included. Two plugins
/// whose names slug to the same tool id (`tool_name_for` is not injective
/// over arbitrary names) would otherwise (a) make the provider reject the
/// request — Anthropic/OpenAI 400 on duplicate tool names — and (b) make
/// the HashMap lookup silently keep only one, routing the model's call to
/// whichever survived. We keep the first and skip later colliders with a
/// warning so neither hazard can occur.
#[must_use]
pub fn build_registry(
    plugins: &[Arc<dyn Plugin>],
) -> (Vec<ToolDefinition>, HashMap<String, Arc<dyn Plugin>>) {
    let mut tools = Vec::new();
    let mut lookup: HashMap<String, Arc<dyn Plugin>> = HashMap::new();
    for p in plugins.iter().filter(|p| p.metadata().agent_callable) {
        let meta = p.metadata();
        let name = tool_name_for(meta);
        if lookup.contains_key(&name) {
            tracing::warn!(
                tool = %name,
                plugin = %meta.name,
                "duplicate agent tool name; skipping the colliding plugin (rename it or give it a distinct plugin_group)"
            );
            continue;
        }
        tools.push(tool_definition_for(meta, &name));
        lookup.insert(name, Arc::clone(p));
    }
    (tools, lookup)
}

/// Build just the [`ToolDefinition`] list (the provider's view). Shares the
/// collision-deduplicated pass with [`build_registry`].
#[must_use]
pub fn build_tools(plugins: &[Arc<dyn Plugin>]) -> Vec<ToolDefinition> {
    build_registry(plugins).0
}

/// One [`ToolDefinition`] for a plugin under the given (already-computed)
/// tool name. Description is prefixed with `[destructive]` when the command
/// sets `destructive = true`; input schema is the empty object for v1.0.
fn tool_definition_for(meta: &crate::plugin::traits::PluginMetadata, name: &str) -> ToolDefinition {
    let description = if meta.destructive {
        format!("[destructive] {}", meta.description)
    } else {
        meta.description.clone()
    };
    ToolDefinition {
        name: name.to_string(),
        description,
        input_schema: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
    }
}

/// Construct the `{plugin_group}__{name}` (or just slugified `{name}`)
/// tool identifier for a given plugin's metadata. Slugification matches
/// what the dispatcher uses to route tool calls back — lowercase ASCII
/// alphanumerics + underscores; everything else becomes `_`.
///
/// Falls back to a deterministic hash-derived id when slugification yields
/// nothing usable (a name with no ASCII alphanumerics, e.g. emoji/CJK-only),
/// so the tool always has a non-empty, provider-acceptable name.
#[must_use]
pub fn tool_name_for(meta: &crate::plugin::traits::PluginMetadata) -> String {
    let cmd = slugify(&meta.name);
    let name = match &meta.plugin_group {
        Some(group) => format!("{}__{cmd}", slugify(group)),
        None => cmd,
    };
    if name.chars().any(|c| c.is_ascii_alphanumeric()) {
        name
    } else {
        fallback_tool_name(meta)
    }
}

/// Deterministic, per-plugin-distinct tool id for names that slug to empty.
fn fallback_tool_name(meta: &crate::plugin::traits::PluginMetadata) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    meta.name.hash(&mut h);
    if let Some(group) = &meta.plugin_group {
        group.hash(&mut h);
    }
    format!("tool_{:x}", h.finish())
}

fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_underscore = false;
    for ch in s.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };
        if mapped == '_' && prev_underscore {
            continue;
        }
        prev_underscore = mapped == '_';
        out.push(mapped);
    }
    out.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::traits::{Plugin, PluginError, PluginMetadata, PluginOutput};
    use async_trait::async_trait;
    use std::time::Duration;

    #[derive(Debug)]
    struct StubPlugin(PluginMetadata);

    #[async_trait]
    impl Plugin for StubPlugin {
        fn metadata(&self) -> &PluginMetadata {
            &self.0
        }
        async fn execute(&self) -> Result<PluginOutput, PluginError> {
            Ok(PluginOutput::default())
        }
    }

    fn meta(name: &str, group: Option<&str>, agent: bool, destructive: bool) -> PluginMetadata {
        PluginMetadata {
            name: name.into(),
            description: format!("desc of {name}"),
            version: "0.1.0".into(),
            author: "test".into(),
            icon: "T".into(),
            icon_nerd: None,
            category: None,
            keybinding: None,
            timeout: Duration::from_secs(5),
            streaming: false,
            entry_path: None,
            prefetch: true,
            plugin_group: group.map(String::from),
            quickkey: None,
            cache: true,
            secrets: vec![],
            settings_spec: vec![],
            widget: false,
            widget_refresh_secs: 0,
            mini_app: false,
            agent_callable: agent,
            destructive,
        }
    }

    fn plugins(metas: Vec<PluginMetadata>) -> Vec<Arc<dyn Plugin>> {
        metas
            .into_iter()
            .map(|m| Arc::new(StubPlugin(m)) as Arc<dyn Plugin>)
            .collect()
    }

    #[test]
    fn build_tools_excludes_non_agent_callable_plugins() {
        let ps = plugins(vec![
            meta("Inbox", Some("Mail"), true, false),
            meta("Send", Some("Mail"), false, false), // not callable
            meta("List Containers", Some("Docker"), true, false),
        ]);
        let tools = build_tools(&ps);
        assert_eq!(tools.len(), 2);
        assert!(tools.iter().any(|t| t.name == "mail__inbox"));
        assert!(tools.iter().any(|t| t.name == "docker__list_containers"));
        assert!(tools.iter().all(|t| !t.name.contains("send")));
    }

    #[test]
    fn destructive_commands_get_marker_prefix_in_description() {
        let ps = plugins(vec![meta("Archive", Some("Mail"), true, true)]);
        let tools = build_tools(&ps);
        assert!(
            tools[0].description.starts_with("[destructive]"),
            "destructive tools must announce themselves: {}",
            tools[0].description
        );
    }

    #[test]
    fn single_command_plugin_omits_group_prefix() {
        let ps = plugins(vec![meta("Weather", None, true, false)]);
        let tools = build_tools(&ps);
        assert_eq!(tools[0].name, "weather");
    }

    #[test]
    fn slugify_collapses_punctuation_and_lowercases() {
        assert_eq!(slugify("My Plugin"), "my_plugin");
        assert_eq!(slugify("Hello-World!"), "hello_world");
        assert_eq!(slugify("AI/ML"), "ai_ml");
        assert_eq!(slugify("__leading__"), "leading");
    }

    #[test]
    fn input_schema_is_empty_object_v1() {
        let ps = plugins(vec![meta("X", None, true, false)]);
        let tools = build_tools(&ps);
        assert_eq!(tools[0].input_schema["type"], "object");
        assert_eq!(tools[0].input_schema["additionalProperties"], false);
    }

    #[test]
    fn colliding_tool_names_are_deduplicated_consistently() {
        // "Git-Hub" and "Git Hub" both slug to "git_hub" — the provider must
        // not receive two tools with the same name, and the lookup must keep
        // exactly one (consistent with the advertised list).
        let ps = plugins(vec![
            meta("Git-Hub", None, true, false),
            meta("Git Hub", None, true, false),
        ]);
        let (tools, lookup) = build_registry(&ps);
        assert_eq!(tools.len(), 1, "no duplicate tool name advertised");
        assert_eq!(lookup.len(), 1, "lookup agrees 1:1 with advertised tools");
        assert!(lookup.contains_key("git_hub"));
    }

    #[test]
    fn empty_slug_name_gets_nonempty_fallback() {
        // An emoji/CJK-only name slugs to "" — must not produce an empty
        // (provider-rejected) tool name.
        let ps = plugins(vec![meta("🚀", None, true, false)]);
        let (tools, lookup) = build_registry(&ps);
        assert_eq!(tools.len(), 1);
        assert!(!tools[0].name.is_empty());
        assert!(lookup.contains_key(&tools[0].name));
    }
}
