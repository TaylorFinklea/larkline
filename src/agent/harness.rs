//! Agent harness — phase state machine + turn loop + session persistence.
//!
//! Foundation shipped in 8.A; queues in 8.C; audit log in 8.D; tool
//! dispatch + hook trait in 8.B. The architecture (phase state machine,
//! turn snapshot, locked decisions) is documented in
//! [`.docs/ai/phases/v1.0-phase-8-agent-loop-spec.md`]. Prior art:
//! pi-mono's `packages/agent/src/agent-loop.ts`.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::agent::audit::AuditLog;
use crate::agent::hooks::{
    AgentHook, BeforeToolCallCtx, BlockDecision, DefaultApprovalHook, PlannedCall, ToolCallPlan,
};
use crate::agent::provider::{
    AskRequest, ContentBlock, Message, Provider, ProviderEvent, Role, StopReason, ToolDefinition,
};
use crate::agent::registry;
use crate::agent::session::{SessionEntry, SessionLog};
use crate::plugin::traits::Plugin;

/// Explicit phase state for the harness.
///
/// Structural operations (`prompt`, model swap, future `abort_session`)
/// require [`Idle`](AgentPhase::Idle) and transition synchronously before
/// any `await`. Tools-and-hooks operations (Phase 8.B+) get their own
/// queue endpoints rather than mutating phase directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentPhase {
    /// Accepting new prompts.
    Idle,
    /// Provider is generating a response; tools (when wired in 8.B) may
    /// be dispatching.
    Turn,
    /// 8.B: tool plan submitted to user; loop paused until
    /// `approve_plan()`. Variant exists from day one so the state
    /// machine doesn't churn when 8.B lands.
    AwaitingApproval,
    /// 8.A+: transient provider error; backing off before retry.
    /// Variant exists from day one; retry logic lands in 8.A.E.
    Retry,
}

/// Anthropic extended-thinking budget level. Scaffolded in `TurnSnapshot`
/// from day one (decision locked 2026-05-24 via harness-deck
/// `phase8-thinking-level`). Non-Anthropic providers ignore it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThinkingLevel {
    /// Extended thinking disabled (default).
    #[default]
    Off,
    /// Minimal budget — sketch reasoning only.
    Minimal,
    /// Low budget — short reasoning.
    Low,
    /// Medium budget — balanced.
    Medium,
    /// High budget — thorough.
    High,
}

/// Immutable view of harness state captured at turn start.
///
/// The harness has live "config" (mutable from settings UI); a snapshot
/// is created at the `Idle → Turn` transition and read from for the rest
/// of the turn. Settings UI writes apply to the *next* turn. Prevents
/// races between live config changes and in-flight provider requests.
#[derive(Debug, Clone)]
pub struct TurnSnapshot {
    /// Resolved request the provider sees. Includes the conversation
    /// history, system prompt, and (Phase 8.B+) the tool registry.
    pub request: AskRequest,
    /// Extended-thinking level for this turn. Read by Anthropic
    /// provider; no-op elsewhere.
    pub thinking_level: ThinkingLevel,
    /// Subset of tool names enabled this turn. `None` = all tools
    /// active. Phase 8.B uses this for dynamic per-turn filtering;
    /// 8.A always sets `None` (no tools).
    pub active_tool_filter: Option<Vec<String>>,
}

/// Live harness configuration — mutable from settings UI. Read into a
/// [`TurnSnapshot`] at turn start.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Top-level system prompt.
    pub system_prompt: Option<String>,
    /// Provider-specific model identifier.
    pub model: String,
    /// Hard cap on output tokens per request. `None` = provider default.
    pub max_tokens: Option<u32>,
    /// Extended-thinking budget.
    pub thinking_level: ThinkingLevel,
    /// Tool registry the model may call. Empty in Phase 8.A.
    pub tools: Vec<ToolDefinition>,
}

impl AgentConfig {
    /// Build a [`TurnSnapshot`] for the next turn, combining live config
    /// with the message history the harness owns.
    fn materialize(&self, messages: Vec<Message>) -> TurnSnapshot {
        TurnSnapshot {
            request: AskRequest {
                system: self.system_prompt.clone(),
                messages,
                tools: self.tools.clone(),
                model: self.model.clone(),
                max_tokens: self.max_tokens,
            },
            thinking_level: self.thinking_level,
            active_tool_filter: None,
        }
    }
}

/// Outcome of a single `prompt()` call.
#[derive(Debug)]
pub enum TurnOutcome {
    /// Turn ran to completion; harness is back to `Idle`.
    Completed {
        /// Reason the model stopped.
        stop_reason: StopReason,
        /// Token accounting if the provider reported it.
        usage: Option<(u32, u32)>,
        /// New entry IDs persisted to the session log for this turn.
        entry_ids: Vec<Uuid>,
    },
    /// Turn was aborted via [`AgentHarness::abort`].
    Aborted,
}

/// Errors from the harness. Wraps subsystem errors with `cause`-style
/// preservation via `thiserror`.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// Operation requires `Idle` (or another specific phase) and the
    /// harness was in a different phase. Records what was attempted so
    /// callers can give a useful error message.
    #[error("agent busy: cannot {attempted} while in {current_phase:?}")]
    Busy {
        /// The phase the harness was actually in.
        current_phase: AgentPhase,
        /// What the caller tried to do (e.g. "start a new prompt").
        attempted: &'static str,
    },
    /// Provider request failed (auth, network, malformed response).
    #[error("provider error: {0}")]
    Provider(#[from] crate::agent::error::ProviderError),
    /// Session log I/O failed.
    #[error("session error: {0}")]
    Session(#[from] crate::agent::session::SessionError),
}

/// Two message queues the harness consults around turn boundaries.
///
/// `steering` and `follow_up` are pi-mono's pattern (locked in ADR-008):
///
/// - **Steering**: mid-turn user input. Drained into the *next provider
///   call within the same turn* — so it only matters when a turn has
///   more than one provider call (i.e. tool cycles, Phase 8.B). 8.A
///   adds the queue + API so 8.B can consume it without changing the
///   public surface; the actual mid-turn poll lands in 8.B.
///
/// - **Follow-up**: queued continuation between turns. Drained at
///   turn-end; each follow-up message becomes the next turn's user
///   message. Survives `abort()`.
///
/// Wrapped in `Arc<Mutex<>>` so `steer()` and `follow_up()` can be
/// called from other tasks (e.g. the TUI input thread) while `prompt()`
/// holds `&mut self`.
#[derive(Debug, Default)]
struct MessageQueues {
    steering: VecDeque<Message>,
    follow_up: VecDeque<Message>,
}

/// One agent conversation — drives a provider, persists to a session log.
///
/// **Mostly single-task.** `prompt()` takes `&mut self`, so only one
/// turn runs at a time. The queue methods (`steer`, `follow_up`) take
/// `&self` and are safe to call from another task while a turn is in
/// flight — they just push onto the shared `Arc<Mutex<MessageQueues>>`.
pub struct AgentHarness {
    config: AgentConfig,
    phase: AgentPhase,
    provider: Box<dyn Provider>,
    session: SessionLog,
    /// In-memory conversation history; reflected in `session` on every
    /// `prompt()`. Reconstructed from the session log on `reopen`.
    messages: Vec<Message>,
    /// Handle to the in-flight provider task, if any. `abort()` calls
    /// `.abort()` on this handle to cancel a running turn.
    in_flight: Option<JoinHandle<Result<(), crate::agent::error::ProviderError>>>,
    /// Shared queues for steering + follow-up messages. See
    /// [`MessageQueues`] doc for semantics.
    queues: Arc<Mutex<MessageQueues>>,
    /// Optional audit-log sink for structured safe-metadata events.
    /// `None` in tests; `Some` in production once the TUI plugin (8.E)
    /// wires the path. See [`crate::agent::audit`] for the schema.
    audit: Option<AuditLog>,
    /// Tool-name → plugin lookup. Built by filtering on
    /// `agent_callable` and slugifying via [`registry::tool_name_for`].
    /// Empty until [`with_plugins`] is called.
    plugin_lookup: HashMap<String, Arc<dyn Plugin>>,
    /// Cancellation token observed by long-running plugin code via the
    /// `lark.is_cancelled()` host fn. Cloned into the per-execution
    /// `CANCEL_TOKEN` task_local before each tool dispatch.
    cancel: tokio_util::sync::CancellationToken,
    /// Agent hooks consulted around tool dispatch. Default:
    /// `[DefaultApprovalHook]` — non-destructive plans auto-dispatch,
    /// destructive plans are blocked until 8.E (or a user-registered
    /// hook) provides a real approval surface.
    hooks: Vec<Box<dyn AgentHook>>,
}

impl AgentHarness {
    /// Open a brand-new session: creates the session file with a UUID v7
    /// id, writes the header entry, returns the harness ready for the
    /// first `prompt()`.
    pub fn create_in(
        config: AgentConfig,
        provider: Box<dyn Provider>,
        sessions_dir: &Path,
    ) -> Result<Self, AgentError> {
        let session = SessionLog::create_in(sessions_dir)?;
        Ok(Self {
            config,
            phase: AgentPhase::Idle,
            provider,
            session,
            messages: Vec::new(),
            in_flight: None,
            queues: Arc::new(Mutex::new(MessageQueues::default())),
            audit: None,
            plugin_lookup: HashMap::new(),
            cancel: tokio_util::sync::CancellationToken::new(),
            hooks: vec![Box::new(DefaultApprovalHook)],
        })
    }

    /// Resume an existing session from disk. Replays the JSONL log to
    /// rebuild the in-memory message history.
    pub fn reopen(
        config: AgentConfig,
        provider: Box<dyn Provider>,
        session_path: &Path,
    ) -> Result<Self, AgentError> {
        let (session, entries) = SessionLog::reopen(session_path)?;
        let messages = entries_to_messages(&entries);
        Ok(Self {
            config,
            phase: AgentPhase::Idle,
            provider,
            session,
            messages,
            in_flight: None,
            queues: Arc::new(Mutex::new(MessageQueues::default())),
            audit: None,
            plugin_lookup: HashMap::new(),
            cancel: tokio_util::sync::CancellationToken::new(),
            hooks: vec![Box::new(DefaultApprovalHook)],
        })
    }

    /// Install the agent's tool registry. Filters `plugins` to those
    /// with `agent_callable = true`, builds the tool-name → plugin
    /// lookup, and replaces any previously-installed registry.
    ///
    /// Also populates `config.tools` so the provider sees the new
    /// schema on the next turn.
    #[must_use]
    pub fn with_plugins(mut self, plugins: &[Arc<dyn Plugin>]) -> Self {
        let callable: Vec<_> = plugins.iter().filter(|p| p.metadata().agent_callable).cloned().collect();
        self.config.tools = registry::build_tools(&callable);
        self.plugin_lookup = callable
            .into_iter()
            .map(|p| (registry::tool_name_for(p.metadata()), p))
            .collect();
        self
    }

    /// Register an agent hook. Hooks fire in registration order; the
    /// first `Block` short-circuits dispatch.
    ///
    /// The default [`DefaultApprovalHook`] is installed at construction
    /// time. To replace it (e.g. 8.E's TUI approval modal), the caller
    /// typically clears defaults via [`with_no_default_hooks`] first.
    #[must_use]
    pub fn with_hook(mut self, hook: Box<dyn AgentHook>) -> Self {
        self.hooks.push(hook);
        self
    }

    /// Drop the default `DefaultApprovalHook`. Used by 8.E when
    /// installing its TUI approval modal instead.
    #[must_use]
    pub fn with_no_default_hooks(mut self) -> Self {
        self.hooks.clear();
        self
    }

    /// Returns the `CancellationToken` this harness will inject into
    /// tool dispatches. Caller can keep it to fire from another task
    /// (e.g. on a Ctrl-C handler) and cancel any in-flight tool.
    #[must_use]
    pub fn cancel_token(&self) -> tokio_util::sync::CancellationToken {
        self.cancel.clone()
    }

    /// Attach an audit log to this harness. Subsequent `prompt()` calls
    /// emit safe-metadata spans (turn start/end + provider start/end)
    /// to the log. Replaces any previously-attached audit log.
    ///
    /// Builder-style — chain after `create_in` / `reopen` so wiring is
    /// declarative at the call site:
    /// ```ignore
    /// let h = AgentHarness::create_in(cfg, provider, &sessions)?
    ///     .with_audit(AuditLog::open(&audit_path)?);
    /// ```
    #[must_use]
    pub fn with_audit(mut self, audit: AuditLog) -> Self {
        self.audit = Some(audit);
        self
    }

    /// Current phase. Cheap; safe to call from any thread (it's just a
    /// `Copy` of an enum), but mutation goes through `&mut self`.
    #[must_use]
    pub fn phase(&self) -> AgentPhase {
        self.phase
    }

    /// Session ID (UUID v7, matches the filename stem).
    #[must_use]
    pub fn session_id(&self) -> Uuid {
        self.session.session_id()
    }

    /// Enqueue a steering message — drained into the next provider call
    /// within the same turn. Only meaningful for multi-call turns (tool
    /// cycles, Phase 8.B+); in 8.A's single-call text turns, steering
    /// has no in-turn injection point and the queue stays full until
    /// the caller migrates the message to follow-up or aborts.
    ///
    /// `&self` (not `&mut self`) so this can be called from another task
    /// while a turn is in flight — the typical use case is the TUI input
    /// thread routing keystrokes here while `prompt()` is awaiting the
    /// provider.
    pub fn steer(&self, msg: Message) {
        self.queues
            .lock()
            .expect("steering queue mutex poisoned")
            .steering
            .push_back(msg);
    }

    /// Enqueue a follow-up message — drained at the next turn boundary
    /// and used as the user message for an additional turn. Valid in any
    /// phase; survives `abort()`. The continuation pattern that makes
    /// queued user input "just work" across an aborted-and-restarted
    /// turn.
    pub fn follow_up(&self, msg: Message) {
        self.queues
            .lock()
            .expect("follow-up queue mutex poisoned")
            .follow_up
            .push_back(msg);
    }

    /// Snapshot of the current queue depths for debugging / status UIs.
    /// Returns `(steering_len, follow_up_len)`.
    #[must_use]
    pub fn queue_depths(&self) -> (usize, usize) {
        let q = self.queues.lock().expect("queue mutex poisoned");
        (q.steering.len(), q.follow_up.len())
    }

    /// Run one or more turns: starts with `initial_msg`, then drains
    /// the follow-up queue, running an additional turn per queued
    /// message until the queue is empty.
    ///
    /// Each turn appends a user message, calls the provider, drains the
    /// stream into `on_event`, then appends the assistant message and
    /// turn-end marker. Returns one [`TurnOutcome`] per turn run.
    ///
    /// **Phase 8.A/C:** still text-only. Tool calls don't dispatch (no
    /// registry yet — Phase 7); steering queue exists but the mid-turn
    /// inject point doesn't fire because single-call turns have nowhere
    /// to inject. Both gaps close in 8.B.
    ///
    /// `on_event` is the streaming callback — typically forwards
    /// `TextDelta` into the TUI's `EngineEvent::PartialOutput` pipeline.
    pub async fn prompt<F>(
        &mut self,
        initial_msg: Message,
        mut on_event: F,
    ) -> Result<Vec<TurnOutcome>, AgentError>
    where
        F: FnMut(&ProviderEvent),
    {
        self.require_phase(AgentPhase::Idle, "start a new prompt")?;

        // One trace_id per top-level prompt — all child turns + provider
        // requests share it so the audit log can reconstruct the causal
        // tree for this user interaction.
        let trace_id = Uuid::now_v7();
        let mut outcomes = Vec::new();
        let mut next_user_msg = Some(initial_msg);

        // Outer loop: run turns until both initial_msg and follow-up
        // queue are drained. Phase 8.B's tool cycles add inner-loop
        // iterations within each turn.
        loop {
            let user_msg = match next_user_msg.take() {
                Some(msg) => msg,
                None => match self.queues.lock().expect("queue mutex").follow_up.pop_front() {
                    Some(msg) => msg,
                    None => break,
                },
            };

            let outcome = self.run_single_turn(trace_id, user_msg, &mut on_event).await?;
            let was_aborted = matches!(outcome, TurnOutcome::Aborted);
            outcomes.push(outcome);
            if was_aborted {
                // Abort breaks the outer loop too; follow-up queue is
                // preserved per spec (next caller can resume).
                break;
            }
        }

        Ok(outcomes)
    }

    /// Run exactly one turn — pulled out of `prompt()` so the outer
    /// loop reads cleanly. Owns the phase transition for the turn.
    /// A turn may include multiple provider calls when tool cycles
    /// happen — each cycle is one iteration of the inner loop.
    //
    // Long because the inner loop weaves three orthogonal concerns:
    // provider streaming, tool dispatch with hook gating, and audit
    // spans. Splitting them out would force shared state through return
    // tuples or fat `&mut self` paths — both worse than the lines.
    #[allow(clippy::too_many_lines)]
    async fn run_single_turn<F>(
        &mut self,
        trace_id: Uuid,
        user_msg: Message,
        on_event: &mut F,
    ) -> Result<TurnOutcome, AgentError>
    where
        F: FnMut(&ProviderEvent),
    {
        self.phase = AgentPhase::Turn;

        let turn_started_at = std::time::Instant::now();
        let turn_span = self.audit.as_mut().and_then(|a| {
            a.turn_start(trace_id)
                .map_err(|e| tracing::warn!(error = %e, "audit turn_start failed"))
                .ok()
        });

        let mut entry_ids: Vec<Uuid> = Vec::new();
        let user_entry_id = self.session.append_user(user_msg.clone())?;
        entry_ids.push(user_entry_id);
        self.messages.push(user_msg);

        // Total token usage across all provider calls in this turn.
        let mut turn_input_tokens: u32 = 0;
        let mut turn_output_tokens: u32 = 0;
        let mut final_stop_reason = StopReason::EndTurn;

        for iteration in 0..MAX_TOOL_ITERATIONS {
            let snapshot = self.config.materialize(self.messages.clone());
            let model = snapshot.request.model.clone();
            let provider_name = self.provider.name();

            let provider_started_at = std::time::Instant::now();
            let provider_span = if let (Some(audit), Some(parent)) =
                (self.audit.as_mut(), turn_span)
            {
                audit
                    .provider_start(trace_id, parent, provider_name, &model)
                    .ok()
            } else {
                None
            };

            let (tx, mut rx) = mpsc::unbounded_channel::<ProviderEvent>();
            let mut acc = TurnAccumulator::default();
            let mut provider_error_kind: Option<&'static str> = None;

            // Scope `ask_fut` so its `&self.provider` borrow drops
            // before we hit any `self.audit.as_mut()` below — the
            // pinned Future is type-erased and the borrow checker
            // can't split disjoint-field borrows across it.
            let ask_result = {
                let ask_fut = self.provider.ask(snapshot.request, tx);
                tokio::pin!(ask_fut);
                loop {
                    tokio::select! {
                        ask_result = &mut ask_fut => {
                            if let Err(ref e) = ask_result {
                                provider_error_kind = Some(provider_error_kind_str(e));
                            }
                            while let Ok(event) = rx.try_recv() {
                                apply_event(event, on_event, &mut acc);
                            }
                            break ask_result;
                        }
                        maybe_event = rx.recv() => {
                            match maybe_event {
                                Some(event) => apply_event(event, on_event, &mut acc),
                                None => break Ok(()),
                            }
                        }
                    }
                }
            };

            if let (Some(audit), Some(parent), Some(span)) =
                (self.audit.as_mut(), turn_span, provider_span)
            {
                let (in_tok, out_tok) = acc.usage.unwrap_or((0, 0));
                let _ = audit.provider_end(
                    trace_id, parent, span,
                    u64::try_from(provider_started_at.elapsed().as_millis())
                        .unwrap_or(u64::MAX),
                    in_tok, out_tok,
                    provider_error_kind,
                );
            }
            ask_result?;

            if let Some((i, o)) = acc.usage {
                turn_input_tokens = turn_input_tokens.saturating_add(i);
                turn_output_tokens = turn_output_tokens.saturating_add(o);
            }
            let stop_reason = acc.stop_reason.clone().unwrap_or(StopReason::EndTurn);
            final_stop_reason = stop_reason.clone();

            // Build the assistant message from the iteration's accumulator.
            // Always includes any text + any tool_use blocks in arrival order.
            let assistant_content = acc.into_assistant_content();
            let needs_tool_dispatch =
                stop_reason == StopReason::ToolUse && has_tool_uses(&assistant_content);

            if !assistant_content.is_empty() {
                let asst_msg = Message {
                    role: Role::Assistant,
                    content: assistant_content.clone(),
                };
                let asst_entry_id = self.session.append_assistant(asst_msg.clone())?;
                entry_ids.push(asst_entry_id);
                self.messages.push(asst_msg);
            }

            if !needs_tool_dispatch {
                // Normal end of turn (or stop reason other than ToolUse).
                break;
            }

            // ---- Tool dispatch ----
            let plan = self.plan_from_content(&assistant_content);
            let block = self.run_before_tool_call_hooks(&plan).await;
            match block {
                BlockDecision::Allow => {
                    let result_blocks = self
                        .dispatch_plan(trace_id, turn_span, &plan, &mut entry_ids)
                        .await?;
                    // Feed tool results back as a user-role message.
                    let user_msg = Message {
                        role: Role::User,
                        content: result_blocks,
                    };
                    self.messages.push(user_msg);
                    // Loop back to provider with extended history.
                    if iteration + 1 == MAX_TOOL_ITERATIONS {
                        tracing::warn!(
                            "agent hit MAX_TOOL_ITERATIONS ({}); breaking turn",
                            MAX_TOOL_ITERATIONS
                        );
                    }
                }
                BlockDecision::Block(reason) => {
                    // Inject the rejection back so the model can react
                    // on the next turn (or end gracefully if it can't).
                    let rejection = format!(
                        "Tool plan rejected by hook: {reason}. Continuing without tools."
                    );
                    let rejection_msg = Message::user(rejection);
                    self.messages.push(rejection_msg);
                    final_stop_reason = StopReason::Other("hook_blocked".to_string());
                    break;
                }
            }
        }

        let turn_end_id = self.session.append_turn_end(
            final_stop_reason.clone(),
            turn_input_tokens,
            turn_output_tokens,
        )?;
        entry_ids.push(turn_end_id);

        if let (Some(audit), Some(span)) = (self.audit.as_mut(), turn_span) {
            let _ = audit.turn_end(
                trace_id,
                span,
                u64::try_from(turn_started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
                &final_stop_reason,
                turn_input_tokens,
                turn_output_tokens,
            );
        }

        self.phase = AgentPhase::Idle;
        Ok(TurnOutcome::Completed {
            stop_reason: final_stop_reason,
            usage: Some((turn_input_tokens, turn_output_tokens)),
            entry_ids,
        })
    }

    /// Build a [`ToolCallPlan`] from the tool_use content blocks in an
    /// assistant message. Looks up each tool's `destructive` flag from
    /// the plugin metadata so the hook decision-table has the info it
    /// needs.
    fn plan_from_content(&self, content: &[ContentBlock]) -> ToolCallPlan {
        let mut calls = Vec::new();
        for block in content {
            if let ContentBlock::ToolUse { id, name, input } = block {
                let destructive = self
                    .plugin_lookup
                    .get(name)
                    .is_some_and(|p| p.metadata().destructive);
                calls.push(PlannedCall {
                    id: id.clone(),
                    tool_name: name.clone(),
                    args: input.clone(),
                    destructive,
                });
            }
        }
        ToolCallPlan { calls }
    }

    /// Chain-run `before_tool_call` across registered hooks. First
    /// `Block` short-circuits.
    async fn run_before_tool_call_hooks(&self, plan: &ToolCallPlan) -> BlockDecision {
        for hook in &self.hooks {
            let ctx = BeforeToolCallCtx { plan };
            if let BlockDecision::Block(reason) = hook.before_tool_call(&ctx).await {
                return BlockDecision::Block(reason);
            }
        }
        BlockDecision::Allow
    }

    /// Dispatch every call in the plan in order; return the resulting
    /// `ToolResult` content blocks ready to attach to the next user
    /// message. Each dispatch emits an `agent.tool_call` audit span.
    /// Cancellation token is scoped per-call so `lark.is_cancelled()`
    /// inside the plugin's Lua observes it.
    async fn dispatch_plan(
        &mut self,
        trace_id: Uuid,
        turn_span: Option<Uuid>,
        plan: &ToolCallPlan,
        entry_ids: &mut Vec<Uuid>,
    ) -> Result<Vec<ContentBlock>, AgentError> {
        let mut blocks = Vec::with_capacity(plan.calls.len());
        for call in &plan.calls {
            let started_at = std::time::Instant::now();
            let tool_span: Option<()> = self.audit.as_mut().and_then(|audit| {
                audit
                    .write(&crate::agent::audit::AuditRecord {
                        ts_ms: now_ms(),
                        trace_id,
                        span_id: Uuid::now_v7(),
                        parent_span_id: turn_span,
                        name: "agent.tool_call",
                        kind: crate::agent::audit::AuditKind::Start,
                        metadata: serde_json::json!({
                            "tool": call.tool_name,
                            "destructive": call.destructive,
                        }),
                    })
                    .ok()
            });

            let (content, is_error) = match self.plugin_lookup.get(&call.tool_name) {
                Some(plugin) => {
                    let plugin = plugin.clone();
                    let cancel = self.cancel.clone();
                    let result = crate::plugin::engine::CANCEL_TOKEN
                        .scope(cancel, async move { plugin.execute().await })
                        .await;
                    match result {
                        Ok(output) => (render_plugin_output(&output), false),
                        Err(e) => (format!("Plugin error: {e}"), true),
                    }
                }
                None => (
                    format!("Unknown tool: {}", call.tool_name),
                    true,
                ),
            };

            // Persist the tool_result in the session log too.
            let result_id = Uuid::now_v7();
            self.session.append(&SessionEntry::ToolResult {
                id: result_id,
                parent_id: self.session.session_id(), // best-effort; session sets correct chain
                timestamp_ms: now_ms(),
                call_id: call.id.clone(),
                content: content.clone(),
                is_error,
            })?;
            entry_ids.push(result_id);

            if let (Some(audit), Some(())) = (self.audit.as_mut(), tool_span) {
                let duration_ms =
                    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
                let _ = audit.write(&crate::agent::audit::AuditRecord {
                    ts_ms: now_ms(),
                    trace_id,
                    span_id: Uuid::now_v7(),
                    parent_span_id: turn_span,
                    name: "agent.tool_call",
                    kind: crate::agent::audit::AuditKind::End,
                    metadata: serde_json::json!({
                        "tool": call.tool_name,
                        "duration_ms": duration_ms,
                        "status": if is_error { "error" } else { "ok" },
                    }),
                });
            }

            blocks.push(ContentBlock::ToolResult {
                tool_use_id: call.id.clone(),
                content,
                is_error,
            });
        }
        Ok(blocks)
    }

    /// Cancel the in-flight turn. Returns immediately; the running
    /// `prompt()` future observes the abort on its next event-loop tick
    /// and returns `TurnOutcome::Aborted`.
    ///
    /// **Queue semantics (Phase 8.C):** clears the steering queue
    /// (mid-turn intent is invalidated by the abort), preserves the
    /// follow-up queue (continuation intent survives — caller can
    /// resume from the next `prompt()`).
    ///
    /// No-op when no turn is in flight; safe to call from any phase.
    pub fn abort(&mut self) {
        if let Some(handle) = self.in_flight.take() {
            handle.abort();
        }
        self.queues
            .lock()
            .expect("queue mutex poisoned during abort")
            .steering
            .clear();
        // The phase transition back to Idle happens inside prompt() when
        // its drain loop sees the channel close. Follow-up is preserved.
    }

    fn require_phase(
        &self,
        required: AgentPhase,
        attempted: &'static str,
    ) -> Result<(), AgentError> {
        if self.phase == required {
            Ok(())
        } else {
            Err(AgentError::Busy {
                current_phase: self.phase,
                attempted,
            })
        }
    }
}

/// Stable string for an audit `error_kind` field. Avoids the
/// Debug-derived format drifting into the wire contract.
fn provider_error_kind_str(err: &crate::agent::error::ProviderError) -> &'static str {
    use crate::agent::error::ProviderError as E;
    match err {
        E::Auth(_) => "Auth",
        E::RateLimited(_) => "RateLimited",
        E::Api(_) => "Api",
        E::Network(_) => "Network",
        E::Malformed(_) => "Malformed",
        E::Config(_) => "Config",
    }
}

/// Accumulator for one provider call's worth of streaming events.
/// Replaces the loose `&mut` parameter list `apply_event` used in 8.A.
#[derive(Debug, Default)]
struct TurnAccumulator {
    /// Concatenated text deltas in arrival order.
    text: String,
    /// Complete tool_use blocks, in arrival order. Each becomes a
    /// `ContentBlock::ToolUse` on the assistant message.
    tool_uses: Vec<(String, String, serde_json::Value)>, // (id, name, args)
    /// Token usage if the provider reported it.
    usage: Option<(u32, u32)>,
    /// Stop reason from the terminal Done event.
    stop_reason: Option<StopReason>,
}

impl TurnAccumulator {
    /// Build the assistant message's content blocks: text first (if
    /// any), then tool_use blocks in arrival order. Anthropic
    /// expects this exact ordering (text-then-tool); OpenAI providers
    /// flatten internally.
    fn into_assistant_content(self) -> Vec<ContentBlock> {
        let mut content = Vec::new();
        if !self.text.is_empty() {
            content.push(ContentBlock::Text(self.text));
        }
        for (id, name, input) in self.tool_uses {
            content.push(ContentBlock::ToolUse { id, name, input });
        }
        content
    }
}

/// True when any content block is a `ToolUse` — drives the "should we
/// dispatch tools?" branch after the provider returns.
fn has_tool_uses(content: &[ContentBlock]) -> bool {
    content
        .iter()
        .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
}

/// Render a `PluginOutput` into the string form the model sees as the
/// tool_result content. For now, JSON-serialize the whole output —
/// gives the model the full structured payload. v1.x may render a
/// more model-friendly summary (especially for large outputs).
fn render_plugin_output(output: &crate::plugin::traits::PluginOutput) -> String {
    serde_json::to_string(output).unwrap_or_else(|e| format!("(unrenderable output: {e})"))
}

/// Wall-clock now in unix epoch milliseconds. Duplicated from
/// session.rs because session.rs's `now_ms` is module-private; not
/// worth a `pub(crate)` for a 5-line helper.
fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Max provider/tool iterations inside one turn before the loop breaks
/// with a warning. Prevents runaway agent loops. v1.1 may make this
/// configurable per-call.
const MAX_TOOL_ITERATIONS: usize = 32;

/// Apply one provider event: dispatch to the user callback, mutate the
/// accumulator. Free function so `tokio::select!` arms can call it
/// without re-borrowing `self`.
fn apply_event<F>(event: ProviderEvent, on_event: &mut F, acc: &mut TurnAccumulator)
where
    F: FnMut(&ProviderEvent),
{
    on_event(&event);
    match event {
        ProviderEvent::TextDelta(chunk) => acc.text.push_str(&chunk),
        ProviderEvent::ToolUse { id, name, args } => acc.tool_uses.push((id, name, args)),
        ProviderEvent::Usage {
            input_tokens,
            output_tokens,
        } => acc.usage = Some((input_tokens, output_tokens)),
        ProviderEvent::Done { stop_reason: sr } => acc.stop_reason = Some(sr),
    }
}

/// Replay a session entry list back into the conversation history the
/// harness keeps in memory. Used by [`AgentHarness::reopen`].
fn entries_to_messages(entries: &[SessionEntry]) -> Vec<Message> {
    let mut out = Vec::new();
    for entry in entries {
        match entry {
            SessionEntry::User { message, .. } | SessionEntry::Assistant { message, .. } => {
                out.push(message.clone());
            }
            SessionEntry::ToolResult {
                call_id, content, ..
            } => {
                // Tool results render back as user messages carrying the
                // ToolResult content block — matches how providers want
                // them in the next request.
                out.push(Message {
                    role: Role::User,
                    content: vec![crate::agent::provider::ContentBlock::ToolResult {
                        tool_use_id: call_id.clone(),
                        content: content.clone(),
                        is_error: false,
                    }],
                });
            }
            // Session/TurnEnd/Leaf don't carry conversation content.
            SessionEntry::Session { .. }
            | SessionEntry::TurnEnd { .. }
            | SessionEntry::Leaf { .. } => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::error::ProviderError;
    use crate::agent::provider::AskRequest;
    use async_trait::async_trait;
    use tempfile::tempdir;

    /// Test double that emits a scripted sequence of `ProviderEvent`s
    /// and returns success. Lets us drive `prompt()` without HTTP.
    #[derive(Debug)]
    struct ScriptedProvider {
        events: Vec<ProviderEvent>,
    }

    #[async_trait]
    impl Provider for ScriptedProvider {
        fn name(&self) -> &'static str {
            "scripted"
        }

        async fn ask(
            &self,
            _request: AskRequest,
            events: mpsc::UnboundedSender<ProviderEvent>,
        ) -> Result<(), ProviderError> {
            for ev in &self.events {
                let _ = events.send(ev.clone());
            }
            Ok(())
        }
    }

    fn default_config() -> AgentConfig {
        AgentConfig {
            system_prompt: None,
            model: "test-model".to_string(),
            max_tokens: None,
            thinking_level: ThinkingLevel::Off,
            tools: Vec::new(),
        }
    }

    #[test]
    fn snapshot_freezes_config_state() {
        let mut cfg = default_config();
        let snap1 = cfg.materialize(vec![Message::user("a")]);
        cfg.model = "different-model".to_string();
        let snap2 = cfg.materialize(vec![Message::user("a")]);
        assert_eq!(snap1.request.model, "test-model");
        assert_eq!(snap2.request.model, "different-model");
    }

    #[test]
    fn create_initializes_idle_with_session_id() {
        let dir = tempdir().unwrap();
        let provider = Box::new(ScriptedProvider { events: vec![] });
        let h = AgentHarness::create_in(default_config(), provider, dir.path()).unwrap();
        assert_eq!(h.phase(), AgentPhase::Idle);
        assert_eq!(h.session_id().get_version_num(), 7);
    }

    #[tokio::test]
    async fn prompt_drives_provider_and_persists_messages() {
        let dir = tempdir().unwrap();
        let provider = Box::new(ScriptedProvider {
            events: vec![
                ProviderEvent::TextDelta("hello ".to_string()),
                ProviderEvent::TextDelta("world".to_string()),
                ProviderEvent::Usage {
                    input_tokens: 5,
                    output_tokens: 2,
                },
                ProviderEvent::Done {
                    stop_reason: StopReason::EndTurn,
                },
            ],
        });
        let mut h = AgentHarness::create_in(default_config(), provider, dir.path()).unwrap();

        let mut deltas = String::new();
        let outcomes = h
            .prompt(Message::user("hi"), |ev| {
                if let ProviderEvent::TextDelta(s) = ev {
                    deltas.push_str(s);
                }
            })
            .await
            .unwrap();

        assert_eq!(h.phase(), AgentPhase::Idle);
        assert_eq!(deltas, "hello world");
        assert_eq!(outcomes.len(), 1, "single turn, no follow-ups");
        match &outcomes[0] {
            TurnOutcome::Completed {
                stop_reason,
                usage,
                entry_ids,
            } => {
                assert_eq!(*stop_reason, StopReason::EndTurn);
                assert_eq!(*usage, Some((5, 2)));
                // user + assistant + turn_end
                assert_eq!(entry_ids.len(), 3);
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn prompt_while_in_turn_returns_busy() {
        // Provoke `Busy` by manually setting the phase — Phase 8.A
        // doesn't actually have concurrent entry points yet, but the
        // gate must be correct for 8.C when steering arrives.
        let dir = tempdir().unwrap();
        let provider = Box::new(ScriptedProvider { events: vec![] });
        let mut h = AgentHarness::create_in(default_config(), provider, dir.path()).unwrap();
        h.phase = AgentPhase::Turn;
        let err = h
            .prompt(Message::user("hi"), |_| {})
            .await
            .unwrap_err();
        match err {
            AgentError::Busy {
                current_phase,
                attempted,
            } => {
                assert_eq!(current_phase, AgentPhase::Turn);
                assert_eq!(attempted, "start a new prompt");
            }
            other => panic!("expected Busy, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn reopen_restores_conversation_history() {
        let dir = tempdir().unwrap();
        let provider = Box::new(ScriptedProvider {
            events: vec![
                ProviderEvent::TextDelta("yes".to_string()),
                ProviderEvent::Done {
                    stop_reason: StopReason::EndTurn,
                },
            ],
        });
        let mut h = AgentHarness::create_in(default_config(), provider, dir.path()).unwrap();
        h.prompt(Message::user("ping?"), |_| {}).await.unwrap();
        let path = h.session.path().to_path_buf();
        drop(h);

        let provider2 = Box::new(ScriptedProvider { events: vec![] });
        let reopened =
            AgentHarness::reopen(default_config(), provider2, &path).unwrap();
        assert_eq!(reopened.phase(), AgentPhase::Idle);
        assert_eq!(reopened.messages.len(), 2, "user + assistant");
    }

    // ---- Phase 8.C — queue semantics ----------------------------------

    /// Build a provider that emits one `"k"` text delta + Done per call.
    /// Enables multi-turn tests without provider state.
    fn scripted_ok_provider() -> Box<dyn Provider> {
        Box::new(ScriptedProvider {
            events: vec![
                ProviderEvent::TextDelta("k".to_string()),
                ProviderEvent::Done {
                    stop_reason: StopReason::EndTurn,
                },
            ],
        })
    }

    #[test]
    fn steer_and_follow_up_are_independent_queues() {
        let dir = tempdir().unwrap();
        let h = AgentHarness::create_in(default_config(), scripted_ok_provider(), dir.path())
            .unwrap();
        h.steer(Message::user("s1"));
        h.steer(Message::user("s2"));
        h.follow_up(Message::user("f1"));
        let (steer_depth, follow_depth) = h.queue_depths();
        assert_eq!(steer_depth, 2);
        assert_eq!(follow_depth, 1);
    }

    #[tokio::test]
    async fn prompt_drains_follow_up_queue_into_extra_turns() {
        let dir = tempdir().unwrap();
        let mut h =
            AgentHarness::create_in(default_config(), scripted_ok_provider(), dir.path()).unwrap();
        h.follow_up(Message::user("part 2"));
        h.follow_up(Message::user("part 3"));

        let outcomes = h
            .prompt(Message::user("part 1"), |_| {})
            .await
            .unwrap();
        assert_eq!(outcomes.len(), 3, "initial + 2 follow-ups = 3 turns");
        assert_eq!(h.queue_depths(), (0, 0), "queues drained");
        // 3 turns × (user + assistant + turn_end) = 9 messages in
        // memory, 9 entries past the session header.
        assert_eq!(h.messages.len(), 6, "3 user + 3 assistant");
    }

    #[tokio::test]
    async fn abort_clears_steering_preserves_follow_up() {
        let dir = tempdir().unwrap();
        let mut h =
            AgentHarness::create_in(default_config(), scripted_ok_provider(), dir.path()).unwrap();
        h.steer(Message::user("mid-turn"));
        h.follow_up(Message::user("queued continuation"));
        assert_eq!(h.queue_depths(), (1, 1));

        h.abort();
        let (steer_after, follow_after) = h.queue_depths();
        assert_eq!(steer_after, 0, "steering cleared on abort");
        assert_eq!(follow_after, 1, "follow-up preserved on abort");
    }

    #[tokio::test]
    async fn audit_log_records_turn_and_provider_spans() {
        use crate::agent::audit::AuditLog;
        let dir = tempdir().unwrap();
        let audit_path = dir.path().join("audit.log");
        let audit = AuditLog::open(&audit_path).unwrap();
        let mut h = AgentHarness::create_in(default_config(), scripted_ok_provider(), dir.path())
            .unwrap()
            .with_audit(audit);

        h.prompt(Message::user("hi"), |_| {}).await.unwrap();

        let raw = std::fs::read_to_string(&audit_path).unwrap();
        let entries: Vec<serde_json::Value> = raw
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();

        // Expect: turn.start, provider.start, provider.end, turn.end
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0]["name"], "agent.turn");
        assert_eq!(entries[0]["kind"], "start");
        assert_eq!(entries[1]["name"], "ai.provider.request");
        assert_eq!(entries[2]["name"], "ai.provider.request");
        assert_eq!(entries[2]["kind"], "end");
        assert_eq!(entries[3]["name"], "agent.turn");
        assert_eq!(entries[3]["kind"], "end");

        // All four spans share the trace_id.
        let trace = &entries[0]["trace_id"];
        for e in &entries[1..] {
            assert_eq!(&e["trace_id"], trace, "trace_id propagates across spans");
        }
        // Provider span's parent is the turn span.
        assert_eq!(entries[1]["parent_span_id"], entries[0]["span_id"]);
    }

    #[tokio::test]
    async fn follow_up_enqueued_mid_turn_drives_next_turn() {
        // Models the real flow: user prompts; while the provider is
        // running, the TUI thread accepts another keystroke and calls
        // `follow_up()`. The outer loop should pick it up at turn end.
        let dir = tempdir().unwrap();
        let mut h =
            AgentHarness::create_in(default_config(), scripted_ok_provider(), dir.path()).unwrap();

        let queues_handle = Arc::clone(&h.queues);
        let pushed = std::cell::Cell::new(false);
        let outcomes = h
            .prompt(Message::user("first"), |ev| {
                // On the first TextDelta from the model, the "user" types
                // ONE follow-up. The Cell prevents re-push on subsequent
                // turns (the callback fires every turn's deltas too).
                if matches!(ev, ProviderEvent::TextDelta(_)) && !pushed.get() {
                    queues_handle
                        .lock()
                        .unwrap()
                        .follow_up
                        .push_back(Message::user("second"));
                    pushed.set(true);
                }
            })
            .await
            .unwrap();
        assert_eq!(outcomes.len(), 2);
        assert_eq!(h.queue_depths(), (0, 0));
    }

    // ---- Phase 8.B — tool dispatch -----------------------------------

    /// Provider that emits a different scripted sequence per call,
    /// so we can model a real tool cycle (call 1: ToolUse / call 2:
    /// EndTurn).
    #[derive(Debug)]
    struct ToolCycleProvider {
        // RefCell of remaining call scripts; popped front per call.
        calls: std::sync::Mutex<VecDeque<Vec<ProviderEvent>>>,
    }
    impl ToolCycleProvider {
        fn new(calls: Vec<Vec<ProviderEvent>>) -> Box<Self> {
            Box::new(Self {
                calls: std::sync::Mutex::new(calls.into()),
            })
        }
    }
    #[async_trait::async_trait]
    impl Provider for ToolCycleProvider {
        fn name(&self) -> &'static str {
            "tool-cycle"
        }
        async fn ask(
            &self,
            _request: AskRequest,
            events: mpsc::UnboundedSender<ProviderEvent>,
        ) -> Result<(), crate::agent::error::ProviderError> {
            let script = self
                .calls
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| vec![ProviderEvent::Done {
                    stop_reason: StopReason::EndTurn,
                }]);
            for ev in script {
                let _ = events.send(ev);
            }
            Ok(())
        }
    }

    /// Agent-callable stub plugin returning a constant output.
    #[derive(Debug)]
    struct AgentToolPlugin {
        meta: crate::plugin::traits::PluginMetadata,
        reply: String,
    }
    #[async_trait::async_trait]
    impl crate::plugin::traits::Plugin for AgentToolPlugin {
        fn metadata(&self) -> &crate::plugin::traits::PluginMetadata {
            &self.meta
        }
        async fn execute(
            &self,
        ) -> Result<
            crate::plugin::traits::PluginOutput,
            crate::plugin::traits::PluginError,
        > {
            Ok(crate::plugin::traits::PluginOutput {
                title: "tool".into(),
                items: vec![],
                raw_text: Some(self.reply.clone()),
                ..Default::default()
            })
        }
    }
    fn tool_plugin(
        name: &str,
        destructive: bool,
        reply: &str,
    ) -> Arc<dyn crate::plugin::traits::Plugin> {
        let mut meta = crate::plugin::traits::PluginMetadata {
            name: name.into(),
            description: format!("desc of {name}"),
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
            mini_app: false,
            agent_callable: true,
            destructive,
        };
        meta.plugin_group = None;
        Arc::new(AgentToolPlugin {
            meta,
            reply: reply.into(),
        }) as Arc<dyn crate::plugin::traits::Plugin>
    }

    #[tokio::test]
    async fn tool_call_cycle_dispatches_plugin_and_feeds_result_back() {
        // Call 1: model emits text + tool_use(calendar) with stop=ToolUse.
        // Call 2: model emits text "done" with stop=EndTurn.
        let provider = ToolCycleProvider::new(vec![
            vec![
                ProviderEvent::TextDelta("checking calendar".into()),
                ProviderEvent::ToolUse {
                    id: "call_1".into(),
                    name: "calendar".into(),
                    args: serde_json::json!({}),
                },
                ProviderEvent::Done {
                    stop_reason: StopReason::ToolUse,
                },
            ],
            vec![
                ProviderEvent::TextDelta("you have 3 events today".into()),
                ProviderEvent::Done {
                    stop_reason: StopReason::EndTurn,
                },
            ],
        ]);
        let dir = tempdir().unwrap();
        let plugins = vec![tool_plugin("calendar", false, "3 events scheduled")];
        let mut h = AgentHarness::create_in(default_config(), provider, dir.path())
            .unwrap()
            .with_plugins(&plugins);
        // DefaultApprovalHook blocks destructive only; this plugin is safe.

        let outcomes = h
            .prompt(Message::user("what's on my calendar"), |_| {})
            .await
            .unwrap();
        assert_eq!(outcomes.len(), 1, "still one turn — multi-call cycle inside");

        // Message history: user, assistant(text+tool_use), user(tool_result),
        // assistant(text). 4 messages.
        assert_eq!(h.messages.len(), 4);

        // Last assistant message includes the post-tool text.
        let last = h.messages.last().unwrap();
        assert_eq!(last.role, Role::Assistant);
        assert!(matches!(&last.content[0],
            ContentBlock::Text(t) if t.contains("you have 3 events today")));

        // Second message (assistant after first provider call) has a
        // tool_use block — the agent loop persisted it.
        match &h.messages[1].content[1] {
            ContentBlock::ToolUse { name, .. } => assert_eq!(name, "calendar"),
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn destructive_tool_blocked_by_default_hook() {
        let provider = ToolCycleProvider::new(vec![vec![
            ProviderEvent::ToolUse {
                id: "call_1".into(),
                name: "delete_email".into(),
                args: serde_json::json!({}),
            },
            ProviderEvent::Done {
                stop_reason: StopReason::ToolUse,
            },
        ]]);
        let dir = tempdir().unwrap();
        // Destructive plugin — default hook should block.
        let plugins = vec![tool_plugin("delete_email", true, "deleted")];
        let mut h = AgentHarness::create_in(default_config(), provider, dir.path())
            .unwrap()
            .with_plugins(&plugins);

        let outcomes = h
            .prompt(Message::user("delete that"), |_| {})
            .await
            .unwrap();
        // Turn finishes (hook block ends the loop), with hook_blocked stop reason.
        assert_eq!(outcomes.len(), 1);
        match &outcomes[0] {
            TurnOutcome::Completed { stop_reason, .. } => {
                match stop_reason {
                    StopReason::Other(s) => assert_eq!(s, "hook_blocked"),
                    other => panic!("expected hook_blocked, got {other:?}"),
                }
            }
            other => panic!("expected Completed, got {other:?}"),
        }
        // The hook-rejection message is appended as a user message.
        let last = h.messages.last().unwrap();
        assert_eq!(last.role, Role::User);
        assert!(matches!(&last.content[0],
            ContentBlock::Text(t) if t.contains("rejected by hook")));
    }
}
