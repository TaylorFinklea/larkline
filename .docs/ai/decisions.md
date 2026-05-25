# Architecture Decision Records

## ADR-001: Flat list replaces grouped sections (2026-03-27)

**Context:** Unified list used `GroupHeader` rows as visual separators for multi-command plugins. This added complexity to navigation (non-selectable rows, cursor skipping) and didn't match Raycast's flat-list feel.

**Decision:** Remove `GroupHeader` variant entirely. Show plugin name as a dimmed inline badge on each command row. All rows are selectable.

**Consequences:** Simplified `UnifiedRow` enum, navigation logic, and rendering. `is_selectable()` now always returns `true`.

## ADR-002: Quickkey exact-match bypasses fuzzy scoring (2026-03-27)

**Context:** Quickkeys (e.g. `ca` for Calendar) were displayed as `[ca]` badges but had no search priority. Typing `ca` ranked "Caffeinate" above Calendar because of fuzzy scoring.

**Decision:** In the search loop, exact quickkey matches get `u32::MAX` score before nucleo runs, pinning them to position 0.

**Consequences:** Quickkeys work as Raycast-style aliases. Fuzzy scoring still applies to all non-exact matches.

## ADR-003: Search always resets cursor to top (2026-03-27)

**Context:** Selection preservation logic (designed for browse-mode refreshes) was fighting search reranking, causing the cursor to jump to the bottom when a quickkey match pinned a result to the top.

**Decision:** Non-empty query always sets `unified_selected = 0`. Selection preservation only applies to empty-query (browse) mode.

## ADR-004: ViewOutput transitions enter Normal mode (2026-03-27)

**Context:** Opening a result (Enter) or returning from one (h/Back) left `vim_mode` in Insert, causing j/k to type into the search field instead of navigating.

**Decision:** `open_plugin()` and Back-to-Unified both set `VimMode::Normal`. User re-enters Insert explicitly with `i` or `/`.

## ADR-005: Configurable sidebar ratio (2026-03-27)

**Context:** Sidebar width was fixed at 28/72 for both browse and ViewOutput modes. Taylor wanted a wider sidebar in browse mode and the ability to tune it.

**Decision:** New `sidebar_ratio` config setting (default 50, range 20-80) controls browse-with-preview width. ViewOutput always uses 28% regardless. Clamped at init time.

## ADR-006: Background update checker with daily cache (2026-04-03)

**Context:** Users install larkline via Homebrew or Cargo but have no way to know when a new version is available. Widget feature was invisible to v0.3.1 users.

**Decision:** Check GitHub releases API once per day in a background tokio task. Cache result to `~/.local/share/larkline/update-check.json`. Detect install method from binary path (Homebrew prefix vs `.cargo/bin`). Show upgrade hint in status bar.

**Consequences:** Zero-latency on cached hits (read JSON on startup). 5-second timeout on API calls, fully non-blocking via oneshot channel. Users see actionable upgrade command specific to their install method.

## ADR-007: Widget picker overlay for discoverability (2026-04-03)

**Context:** Widget management was buried in keybindings (K/W/H/L/D) that only the developer knew about. Taylor (the creator) couldn't figure out how to add/remove widgets.

**Decision:** Add a centered popup overlay (like theme picker) showing all widget-eligible commands with [x]/[ ] checkboxes. Triggered by `A` in Normal mode. Space toggles. Persists to existing `plugin-manager.json` disabled_widgets list. Also added contextual status bar hints: `K widgets` / `W show widgets` / `A add/remove`.

**Consequences:** Widget management is now discoverable from the status bar without reading docs. Reuses existing PluginManagerConfig persistence — no new state file.

## ADR-008: Phase 6 AI plugin shells out to `lark ai-ask` CLI; Phase 8 architecture borrowed from pi-mono (2026-05-23)

**Context:** Phase 6 (AI single-shot plugin) needed a way to expose `agent::Provider::ask` to Lua plugins. Two options: (a) add a `lark.ai_ask(prompt)` Lua host fn that runs the provider in-process, or (b) ship a `lark ai-ask` CLI subcommand and have the Lua plugin shell out to it via `lark.exec_io`. Additionally, after studying earendil-works/pi (the only mature open-source agent harness with overlapping scope), we wanted to lock in Phase 8 architecture before writing code.

**Decision:**

1. **Phase 6 plugin shells out to the CLI.** No in-process `lark.ai_ask` host fn. One code path for provider/key/streaming.
2. **`lark ai-ask` CLI is the canonical single-shot entry point.** Mirrors pi-mono's `pi -p "..."` pattern. Streams plain text to stdout, usage to stderr. Usable standalone outside the TUI.
3. **Phase 8 architecture spec written *before* Phase 6/7 code lands.** Locks in: explicit `AgentPhase` state machine, per-turn `TurnSnapshot` (config changes apply NEXT turn), two-queue model (steering + follow-up), `AgentHook` trait with built-in `DryRunApprovalHook` (the "dry-run plan preview" IS a hook, not a special case), append-only JSONL session log at `$XDG_STATE_HOME/larkline/sessions/<id>.jsonl`, separate audit log capturing only safe metadata by default.

**Consequences:**

- **Phase 6 plugin is ~50 lines of Lua** with two clear TODO blocks for UX choices; CLI is ~116 LOC of Rust. Total surface is minimal.
- **Streaming UX inside the TUI plugin is deferred to Phase 6.5** (the CLI streams, but the plugin blocks on `lark.exec_io`). Acceptable for v1.0 — power users get streaming via the terminal CLI.
- **Phase 8 won't churn structure.** The spec is the source of truth. Sub-phases 8.A through 8.F map directly to spec sections.
- **In-process `lark.ai_ask` host fn is deferred to Phase 8** when the agent loop needs fine-grained event control that subprocess JSON-line streaming can't give. Likely added during 8.E (TUI plugin).
- **Pi-mono's `Skill` construct is explicitly NOT adopted** — Larkline's plugin manifest + commands already fill that role; Pi needed Skills because it lacked a plugin registry. Avoids a parallel system.
- **Linear sessions only in v1.0** — tree branching / `/fork` / `/clone` (pi has them) deferred to v1.1 despite the `parent_id` field being stored from day one.
- **Compaction deferred to v1.1.** Long sessions in v1.0 fail with a token-limit error surfaced cleanly. Pi's compaction algorithm is the v1.1 implementation target.

**Open questions resolved 2026-05-24** (via harness-deck `20260523-pi-mono-study`):

1. **CancellationToken plumbing → Phase 7.** Phase 7 lands the breaking-but-additive trait change to `Plugin::execute()`. Every impl adds the param, ignored if unused. Agent must be able to abort slow tools. **Implementation deviated — see ADR-009.**
2. **`thinking_level` scaffolded in `TurnSnapshot` from day one.** Anthropic provider reads it; other providers no-op. Avoids restructure when Anthropic extended thinking ships.
3. **Approval UX: all-or-nothing for v1.0.** One Enter approves the full plan; per-tool toggles deferred to v1.1. Matches pi-mono.
4. **Session IDs are UUID v7.** `features = ["v7"]` on the `uuid` dep. Time-ordered → newest-first session listing is free.

## ADR-009: Task-local CANCEL_TOKEN instead of `Plugin::execute(&self, cancel)` trait change (2026-05-25)

**Context:** ADR-008 captured the locked decision that Phase 7 would land cancellation via a breaking-but-additive trait parameter on `Plugin::execute()`. Implementing it revealed the surface cost: ~10 files touched (LuaPlugin, ScriptPlugin, MockPlugin, FailPlugin, PanicPlugin, StubPlugin, AgentToolPlugin) + every call site (engine, actions, main, tests). The user's vote in `phase8-cancellation` was binary ("land in Phase 7" vs "defer to v1.1") — they wanted cancellation to work, not the specific implementation surface.

**Decision:** Ship cancellation via a `CANCEL_TOKEN` `tokio::task_local!` in `src/plugin/engine.rs`, matching the existing `SECRETS` / `PLUGIN_LIST` / `INVOKE_DEPTH` pattern. The harness scopes a `CancellationToken` per tool dispatch; Lua plugins poll via a new `lark.is_cancelled()` host fn.

**Consequences:**

- **Same outcome.** Lua plugins can early-exit when the agent aborts a tool. The user-visible behavior matches what ADR-008 promised.
- **1/10th the surface area.** Two files touched (`engine.rs` adds the task_local, `lua.rs` adds the host fn). No plugin impls churn. No call sites change.
- **Pattern-consistent.** Matches how Larkline already plumbs ambient context through plugin invocation. New maintainers don't have to learn two patterns.
- **Deviation from the literal text of ADR-008.** Reasonable judgment call given user intent; documented here so the deviation is auditable.
- **Limitation:** Plugins outside Lua (shell scripts, future Python plugins) can't observe the token. v1.x can add cancellation via SIGINT to subprocess plugins if demand surfaces.

The trait-change route remains available if v1.x wants explicit per-call cancellation observability that task-local can't provide (e.g. for async-await cancellation propagation through plugin code). For v1.0 the polling primitive suffices.
