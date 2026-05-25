//! Agent hooks — extension points around tool dispatch.
//!
//! The harness consults hooks at well-defined points during a turn.
//! v1.0 ships exactly one hook surface (`before_tool_call`) and one
//! built-in implementation (`DefaultApprovalHook`). Future versions
//! (v1.1+) will expose plugin-registered hooks for custom approval,
//! rate limiting, content filtering, etc. — the trait is `pub` from
//! day one so the public surface is stable.
//!
//! Approval semantics (locked decision via harness-deck
//! `phase8-approval-granularity`): **all-or-nothing**. If any
//! destructive tool call is in the plan, the entire plan goes for
//! approval. Per-tool toggles deferred to v1.1.

use async_trait::async_trait;
use serde_json::Value as JsonValue;

/// One tool the model wants to call. Built from `ProviderEvent::ToolUse`
/// once the harness has resolved which plugin command backs it and
/// whether that command is destructive.
#[derive(Debug, Clone)]
pub struct PlannedCall {
    /// Provider-issued ID. Used to match the tool_result back to the
    /// tool_use in the next assistant message.
    pub id: String,
    /// `{plugin_group}__{command}` (or just `{command}` for
    /// single-command plugins). Matches `registry::tool_name_for`.
    pub tool_name: String,
    /// JSON-shaped args the model passed. v1.0 plugins ignore these
    /// (input schemas are `{}`); v1.1 will pass them as form values.
    pub args: JsonValue,
    /// Manifest `destructive` flag. Drives the approval-needed gate.
    pub destructive: bool,
}

/// The set of tools the model wants to call this iteration. A `plan`
/// is what the user approves or rejects as a whole.
#[derive(Debug, Clone)]
pub struct ToolCallPlan {
    /// All planned calls for this iteration, in the order the model
    /// emitted them.
    pub calls: Vec<PlannedCall>,
}

impl ToolCallPlan {
    /// True when at least one call in the plan is destructive. Drives
    /// the all-or-nothing approval gate.
    #[must_use]
    pub fn has_destructive(&self) -> bool {
        self.calls.iter().any(|c| c.destructive)
    }
}

/// Context passed to `before_tool_call` hooks. Borrows the plan so
/// hooks can inspect without taking ownership.
#[derive(Debug)]
pub struct BeforeToolCallCtx<'a> {
    /// The tool plan the harness is about to dispatch.
    pub plan: &'a ToolCallPlan,
}

/// What a hook returns from `before_tool_call`. Hooks chain
/// short-circuit: the first `Block` wins; downstream hooks don't run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockDecision {
    /// Proceed with dispatch.
    Allow,
    /// Reject the plan; carry a human-readable reason that surfaces in
    /// the next turn's user-visible message ("Tool plan rejected:
    /// {reason}"). The model gets the reason as context so it can
    /// adjust on the next turn.
    Block(String),
}

/// Extension point trait. Implementors register via
/// [`crate::agent::harness::AgentHarness::with_hook`].
///
/// `&self` so hooks are shareable / cheap to clone the harness around.
/// `async` so an approval-modal hook can await user input from the TUI.
#[async_trait]
pub trait AgentHook: Send + Sync {
    /// Called once per tool plan, *before* any tool in the plan
    /// dispatches. Return `Allow` to proceed or `Block` to reject the
    /// whole plan.
    ///
    /// The default impl allows everything — overrides typically gate
    /// destructive plans.
    async fn before_tool_call(&self, _ctx: &BeforeToolCallCtx<'_>) -> BlockDecision {
        BlockDecision::Allow
    }
}

/// Default hook installed on every harness: allows non-destructive
/// plans, blocks plans containing any destructive tool with a clear
/// message pointing the user at the 8.E approval surface (or a
/// custom hook).
///
/// Phase 8.E replaces this with `TuiApprovalHook` which actually
/// prompts the user via a modal. v1.1+ users could register their
/// own hook with different semantics.
#[derive(Debug, Default)]
pub struct DefaultApprovalHook;

#[async_trait]
impl AgentHook for DefaultApprovalHook {
    async fn before_tool_call(&self, ctx: &BeforeToolCallCtx<'_>) -> BlockDecision {
        if ctx.plan.has_destructive() {
            BlockDecision::Block(
                "Destructive tool call requires approval — install a TUI approval hook or a \
                 custom AgentHook implementation. Phase 8.E ships the default TUI surface."
                    .to_string(),
            )
        } else {
            BlockDecision::Allow
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(id: &str, destructive: bool) -> PlannedCall {
        PlannedCall {
            id: id.into(),
            tool_name: "mail__archive".into(),
            args: json!({}),
            destructive,
        }
    }

    #[test]
    fn has_destructive_detects_any_destructive_call() {
        let safe = ToolCallPlan {
            calls: vec![call("a", false), call("b", false)],
        };
        let mixed = ToolCallPlan {
            calls: vec![call("a", false), call("b", true)],
        };
        assert!(!safe.has_destructive());
        assert!(mixed.has_destructive());
    }

    #[tokio::test]
    async fn default_hook_allows_safe_plan() {
        let hook = DefaultApprovalHook;
        let plan = ToolCallPlan {
            calls: vec![call("a", false)],
        };
        let dec = hook
            .before_tool_call(&BeforeToolCallCtx { plan: &plan })
            .await;
        assert_eq!(dec, BlockDecision::Allow);
    }

    #[tokio::test]
    async fn default_hook_blocks_destructive_plan_with_reason() {
        let hook = DefaultApprovalHook;
        let plan = ToolCallPlan {
            calls: vec![call("a", true)],
        };
        match hook
            .before_tool_call(&BeforeToolCallCtx { plan: &plan })
            .await
        {
            BlockDecision::Block(reason) => assert!(reason.contains("approval")),
            other => panic!("expected Block, got {other:?}"),
        }
    }
}
