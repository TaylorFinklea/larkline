# Current State

> Updated: 2026-05-28

## Bug sweep + fixes (2026-05-28) — advances Phase 10

Ran an exhaustive 13-dimension multi-agent review of the whole codebase
(each finding adversarially verified) → **48 confirmed defects** (15 high /
19 medium / 14 low; 5 false positives correctly refuted). Fixed **all of
them** across 11 commits on `v1.0-prep` (`c5d5757`..`0f1260e`, on top of
`68a5080`). ~25 regression tests added. Build green: **62 lib + 247 bin +
integration pass**, `clippy --bin lark -D warnings` clean, `fmt --check`
clean. Global binary reinstalled. **Nothing pushed.**

Headline fixes (full detail in harness-deck report `20260528-bug-sweep-fixes`):
- **Panics** (HIGH): empty-form Tab divide-by-zero, lone-quote `.env` slice,
  multibyte hex/clipboard byte-slices, widget-width underflow, NaN http timeout.
- **Agent harness**: phase now resets to Idle on error (was bricked in Turn
  forever); **cancellation wired end-to-end** (abort fires token → select
  loop observes → `TurnOutcome::Aborted`; token reset around prompt; dead
  `in_flight` removed); panic-isolated tool dispatch; hook-blocked plans emit
  paired `tool_result`s (was orphaning tool_use → 400 on every later turn).
- **Providers**: Anthropic `input_tokens` now captured (was always 0);
  OpenAI-Chat drains tool_calls on any finish_reason (OpenRouter/Ollama).
- **Plugin engine**: `kill_on_drop` on all 4 spawns; `exec_io` pipe-buffer
  deadlock fixed (concurrent stdin/stdout); store size-check consistency +
  process-wide shared store registry (fixes `lark.invoke` lost-update).
- **Name collisions**: registry dedups colliding tool names (was 400/misroute)
  + **`Group:Command` addressing** for `lark invoke`/`lark.invoke` so shadowed
  commands are reachable (e.g. `Mail:Inbox`); empty-slug fallback.
- **Config**: section-by-section merge (one bad field no longer nukes the
  whole config); `save_theme_preset` won't wipe a malformed-but-recoverable
  file; no secrets to world-writable `/tmp`.
- **Atlassian**: OAuth callback + reqwest timeouts (was hang-forever); accept
  loop skips stray connections; token cache 0600 re-asserted on existing file.
- **Session**: reopen tolerates a torn trailing line (crash recovery); honest
  durability comments.
- **CLI/TUI**: `lark plugin remove` path-traversal blocked; AI subcommands
  surface config warnings; `ai-ask` tolerates broken pipe; ActionResult
  output-search desync, widget-focus nav, power-menu Enter, mini-app scroll.

Follow-ups done (commit `c11c031`):
- **NvimEdit `|` ex-command injection** — now opens via `nvim --remote`/
  `--remote-tab` (literal argv filename, no ex parsing); split/vsplit via a
  separate path-free command.
- **5 pre-existing `--all-targets` test-code lints** cleared; `cargo clippy
  --all-targets -- -D warnings` is now green.

Open / deferred (documented, NOT bugs left unfixed):
- Session forward-compat `#[serde(other)]` skip left as v1.x (needs
  cross-cutting `SessionEntry` match churn; doc corrected to not over-claim).
- Real-provider smoke of the agent cancellation path still wants Taylor's pass
  (no production caller of abort()/cancel_token() exists until the TUI agent
  loop, Phase 8.E).

## Next Milestone — v1.0 Agent Palette

Planned 2026-05-09. ~21 weeks horizon. Headline thesis: **a command palette
where the AI uses your plugins as tools**. Full plan in
[`v1.0-plan.md`](./v1.0-plan.md); roadmap entry in [`roadmap.md`](./roadmap.md)
under "v1.0 — Agent Palette".

## Phase status

| Phase | Status | Notes |
|---|---|---|
| 1 — Tag v0.13/v0.14/v0.15 | ✅ Done (2026-05-10) | v0.14.0 + v0.15.0 tagged + on Homebrew tap |
| 2 — macOS Swift helper (EventKit) | ✅ Done (2026-05-12) | 5 commits on `v1.0-prep`. Sub-phase 2.E dropped (no programmatic RSVP). Report at [`phases/v1.0-phase-2-macos-helper-report.md`](./phases/v1.0-phase-2-macos-helper-report.md) |
| 3 — Calendar v2 plugin | ✅ Done (2026-05-12) | 4 commits on `v1.0-prep`. Added `lark.exec_io` host fn for stdin piping. Report at [`phases/v1.0-phase-3-calendar-v2-report.md`](./phases/v1.0-phase-3-calendar-v2-report.md) |
| 4 — Mail plugin (osascript) | ✅ Done (2026-05-12) | 2 commits on `v1.0-prep`. 4 of 5 sub-phases shipped; 4.E (mailbox switcher chain) deferred to v1.x. Smoke runbook needs Taylor's pass on mutating actions ([`phases/v1.0-phase-4-mail-smoke-runbook.md`](./phases/v1.0-phase-4-mail-smoke-runbook.md)). Report at [`phases/v1.0-phase-4-mail-report.md`](./phases/v1.0-phase-4-mail-report.md) |
| 4.5 — Mail UX polish + TUI per-row actions + mobile layout | ✅ Done (2026-05-17) | 10 commits on `v1.0-prep`. Dogfood-driven: HTML body rendering (w3m/pandoc), inline image preview (chafa), Inbox perf fix (30s→7s), View body chain action, Space power menu "This item" category, `LayoutProfile` for narrow terminals, `lark.plugin_dir` host fn. Report at [`phases/v1.0-phase-4.5-mail-polish-tui-mobile-report.md`](./phases/v1.0-phase-4.5-mail-polish-tui-mobile-report.md) |
| 5 — AI Provider trait + 4 backends | ✅ Done (2026-05-18) | 6 commits on `v1.0-prep`. Provider trait + AskRequest/Message/ToolDefinition/ProviderEvent shared types, Anthropic Messages, OpenAI Responses, OpenRouter + Ollama (shared Chat Completions transport), `agent::build_provider` factory, `[ai]` config section, 38 new unit tests. Real-API smoke pending Taylor's runbook pass. Report at [`phases/v1.0-phase-5-ai-provider-report.md`](./phases/v1.0-phase-5-ai-provider-report.md); smoke runbook at [`phases/v1.0-phase-5-ai-provider-smoke-runbook.md`](./phases/v1.0-phase-5-ai-provider-smoke-runbook.md) |
| 6 — AI single-shot plugin | 🟡 Code done (pending smoke) | `lark ai-ask` CLI + `examples/plugins/ai/{manifest,ask.lua}`. Phase report + smoke runbook at [`phases/v1.0-phase-6-report.md`](./phases/v1.0-phase-6-report.md). Pending: real-provider smoke pass + commit |
| 7 — Tool registry + manifest schema | 🟡 Code done (pending smoke) | `agent_callable` + `destructive` manifest fields + `crate::agent::registry` builder + CANCEL_TOKEN task-local + `lark.is_cancelled()` host fn. Cancellation via task-local instead of trait change — see **ADR-009**. Report: [`phases/v1.0-phase-7-report.md`](./phases/v1.0-phase-7-report.md) |
| 8 — AI tool-use plugin + dry-run plan | 🟡 All sub-phases shipped (pending smoke) | Headline feature. **All six sub-phases (8.A–8.F) complete.** ~1940 LOC of agent code, 26 new tests, 220 total passing. `lark agent-ask` CLI + `examples/plugins/ai/agent.lua` + safe-by-default destructive blocking. Report: [`phases/v1.0-phase-8-report.md`](./phases/v1.0-phase-8-report.md). Smoke runbook: [`phases/v1.0-phase-8-smoke-runbook.md`](./phases/v1.0-phase-8-smoke-runbook.md) |
| 9 — Web search shortcuts + onboarding wizard | Pending | |
| 10 — QA pass + bug sweep + theme polish | Pending | |
| 11 — Beta + Medium draft + launch prep | Pending | |
| 12 — Tag v1.0 + Medium post + Show HN | Pending | |

## Active Branches

- `main` — at `bbbe3b2` (Release v0.15.0); pushed to origin
- `v1.0-prep` — branched off main; 5 Phase 2 + 4 Phase 3 + 2 Phase 4 + Phase 4.G handoff + 10 Phase 4.5 = ~23 commits. Local-only until v1.0 ships.

## Phase 2 outcome — macOS helper

5 commits on `v1.0-prep`:

| Sub-phase | Commit | Summary |
|---|---|---|
| 2.A | `973f922` | Swift package skeleton; hello-JSON; 70KB binary |
| 2.B | `8e547e7` | stdin/stdout JSON-line protocol; `version` + `ping` commands |
| 2.C | `8dce1b0` | `list_calendars` via EventKit (TCC permission gate handled) |
| 2.D | `aab4904` | `events_for_range` with meeting URL extraction (Teams/Zoom/Meet/Webex regex) |
| 2.F | `be9a788` | CI universal-binary build (`lipo` arm64+x86_64), ad-hoc codesign, Homebrew formula install |

Dropped during execution:
- **Sub-phase 2.E (`respond_to_invite`)** — `EKParticipant.participantStatus` is read-only on iOS/macOS. Cal v2 will shell to `/usr/bin/open ical://event/<id>` for RSVP instead.

Architecture summary in [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md) "macOS Helper (v1.0+)" section.

## Current Version

`Cargo.toml` at `0.15.0` (tagged). Next public tag: `v1.0.0` after all 12 phases land. No intermediate tags between v0.15.0 and v1.0.

## Validation (Phase 2 baseline)

- `swift build -c release` — clean, 70KB binary
- Cross-arch: `swift build --arch arm64` + `--arch x86_64` + `lipo -create` — universal Mach-O verified locally
- Protocol smoke: 6-request batch (version, ping, unknown, malformed, empty, args-extra) — all expected responses
- EventKit smoke: `list_calendars` → 15 calendars; `events_for_range` 14-day → 9 events with attendees/dates/sources

`cargo test` / `cargo clippy` / `cargo fmt --check` baselines from v0.15.0 still pass (no Rust changes in Phase 2).

## Pre-Phase-3 Gates

Smoke runbook for Phase 2: [`phases/v1.0-phase-2-macos-helper-smoke-runbook.md`](./phases/v1.0-phase-2-macos-helper-smoke-runbook.md). Three sections: protocol (no TCC), EventKit (TCC required), universal binary (only when verifying CI release artifacts).

## Phase 4.5 outcome — Mail UX polish, TUI per-row actions, mobile layout

10 commits on `v1.0-prep`, all reactive to Taylor's Mail plugin
dogfooding. Three threads:

| Thread | Headline | Commits |
|---|---|---|
| Mail UX | View body via w3m/pandoc, View images via chafa, Inbox listing perf (30s timeout → 7s) | `8163196` `c33e110` `b830b12` `7de4985` `a999041` `6450ea0` |
| TUI per-row actions | Space power menu "This item" category with digit shortcuts; `[SPC] actions` hint; palette nav bug fix | `eae7532` `8e48396` `f6dde38` |
| Responsive layout | `LayoutProfile` (Phone / Narrow / Medium / Wide) auto-detected from terminal width; `:layout <profile>` override | `7ba697e` |

New host-side surface:
- `lark.plugin_dir` global (absolute path to plugin source dir; enables sibling helper scripts like `mail_render.py`)
- `AppState.layout_profile_override` field
- `Action::RunFocusedItemAt(usize)` variant

Phase 4.5 report: [`phases/v1.0-phase-4.5-mail-polish-tui-mobile-report.md`](./phases/v1.0-phase-4.5-mail-polish-tui-mobile-report.md).

## Validation (Phase 4.5 baseline)

- `cargo test --bin lark` → **147 passed** (+4 from Phase 4 baseline: palette nav, RunFocusedItemAt, power-menu construction, LayoutProfile)
- `cargo test --lib` → 58 passed
- `cargo clippy --bin lark -- -D warnings` → clean
- `luac -p` on modified plugins → clean
- End-to-end smoke (Taylor's machine): Inbox listing on real ~200-msg iCloud inbox ≈ 7s; View body via w3m renders cleanly; View images downloads + chafa-renders 11 remote images; `:layout phone` confirmed list-only on 200-col terminal

## Phase 5 outcome — AI Provider trait + 4 backends

6 commits on `v1.0-prep`. The agent layer foundation that Phases 6-8
build on:

| Sub-phase | Commit | Summary |
|---|---|---|
| 5.A+B | `fdbab95` | `src/agent/` module scaffold (Provider trait + ProviderEvent/Message/ToolDefinition types + ProviderError) and `[ai]` config section with Keychain wiring (AI_SECRET_KEYS const, AiConfig struct) |
| 5.C | `88cdbf7` | AnthropicProvider — Messages API, SSE streaming, input_json_delta buffering, prompt caching on last tool |
| 5.D | `b94e512` | OpenAiResponsesProvider — Responses API (the newer endpoint), function_call_arguments.done emits ToolUse without buffering |
| 5.E | `dff5187` | OpenAiChatProvider — Chat Completions transport shared by OpenRouter + Ollama, factory methods for each, tool-call accumulation across `data:` chunks |
| 5.factory | `70ce391` | `agent::build_provider` reads `[ai] provider = "..."` + Keychain secrets and returns Box<dyn Provider>; Debug supertrait on Provider |

New host-side surface:

* `agent::Provider` trait + `Provider::ask` async method streaming `ProviderEvent` over `mpsc::UnboundedSender`
* `agent::build_provider(ai_config, secrets) -> Box<dyn Provider>` factory
* `config::AiConfig` + `config::AiProviderName` + `config::AI_SECRET_KEYS`
* `[ai]` section in default config template

No TUI surface yet — Phase 6 wires the AI plugin.

## Validation (Phase 5 baseline)

* `cargo test --bin lark` → **188 passed** (+38 from Phase 4.5: 9 Anthropic + 12 OpenAI Responses + 13 OpenAI Chat + 4 factory)
* `cargo test --lib` → 58 passed
* `cargo clippy --bin lark -- -D warnings` → clean
* Real-API end-to-end smoke pending Taylor's runbook pass ([phases/v1.0-phase-5-ai-provider-smoke-runbook.md](./phases/v1.0-phase-5-ai-provider-smoke-runbook.md))

New deps: `reqwest "stream"` feature + `futures-util` (default-feature-less).

## Phase 6 outcome — AI single-shot (code done 2026-05-24)

Uncommitted on `v1.0-prep`. Report: [`phases/v1.0-phase-6-report.md`](./phases/v1.0-phase-6-report.md). Smoke runbook: [`phases/v1.0-phase-6-smoke-runbook.md`](./phases/v1.0-phase-6-smoke-runbook.md).

- **CLI subcommand** `lark ai-ask [--system X] [--model Y] [--max-tokens N] <PROMPT>` — streams ProviderEvent::TextDelta to stdout as plain text; usage stats to stderr. Mirrors pi-mono's `pi -p`. `src/main.rs:handle_ai_ask_command` (~116 LOC).
- **AI plugin** at `examples/plugins/ai/{manifest.toml, ask.lua}` (~140 Lua LOC). Form: prompt + optional system override. Renders response in preview pane with copyable text + token-usage summary row. Error path uses ❌ icon + `help_url` → `docs/AI_INTEGRATION.md`; friendly labels for missing-key / rate-limit cases.
- **Decision (ADR-008): no in-process `lark.ai_ask` host fn.** Plugin shells out to CLI for a single code path. In-process host fn defers to Phase 6.5 when streaming UX inside the TUI lands.

**Pending before Phase 6 marks ✅:** real-provider smoke pass per runbook, then commit.

## Phase 8 progress — sub-phases shipped ahead of schedule

Uncommitted on `v1.0-prep`. 8.B/8.E/8.F still pending (8.B blocked on Phase 7).

| Sub-phase | Surface | LOC | Status |
|---|---|---|---|
| **8.A** | `src/agent/session.rs` (JSONL append-only log, UUID v7 IDs) + `src/agent/harness.rs` (`AgentPhase` state machine, `TurnSnapshot`, `ThinkingLevel`, `AgentConfig`, `AgentHarness::create_in/reopen/prompt/abort`) | ~820 | ✅ Code done |
| **8.C** | `MessageQueues` in `harness.rs` — `Arc<Mutex<>>`-wrapped steering + follow-up queues; `steer()/follow_up()/queue_depths()` methods; `prompt()` drains follow-ups across multiple turns; `abort()` clears steering, preserves follow-up | ~150 | ✅ Code done |
| **8.D** | `src/agent/audit.rs` — safe-metadata-only JSONL audit log; `AuditRecord` schema (`trace_id` / `span_id` / `parent_span_id`); typed helpers (`turn_start/end`, `provider_start/end`); harness emits spans automatically when `with_audit()` is set | ~290 | ✅ Code done |

**Validation:** `cargo test --bin lark` → **210 passed** (+22 from Phase 5 baseline). `cargo clippy --bin lark -- -D warnings` → clean.

## Phase 7 + 8 outcome — Agent palette complete (2026-05-25)

All code for v1.0's headline feature shipped on `v1.0-prep`
(uncommitted). End-to-end flow works:

- `lark agent-ask "<prompt>"` builds an `AgentHarness`, dispatches
  tool cycles, streams to stdout, persists session + audit log.
- `examples/plugins/ai/agent.lua` shells out to the CLI from the TUI.
- Three-layer safety: (1) `agent_callable` opt-in per plugin, (2)
  `destructive` flag per command, (3) `DefaultApprovalHook` blocks
  destructive plans unless `--yes`.

Validation: **220 tests pass** (+10 from Phase 6 baseline of 210), clippy
clean, all 40+ existing manifests still parse, Lua syntax check on
ask.lua + agent.lua clean.

**Pending before tagging Phase 8 ✅:** real-provider smoke pass per
[`phases/v1.0-phase-8-smoke-runbook.md`](./phases/v1.0-phase-8-smoke-runbook.md),
then commit.

## Next

See the **Now / Next / Later** section in [`roadmap.md`](./roadmap.md).
With Phases 1–8 in the bank, remaining v1.0 work is:

- **Phase 9** — Web search shortcuts plugin + onboarding wizard.
- **Phase 10** — QA pass + bug sweep + theme polish.
- **Phase 11** — Beta + Medium draft + launch prep.
- **Phase 12** — Tag v1.0 + Medium post + Show HN.

The big architectural lifts are done.
