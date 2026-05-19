# Current State

> Updated: 2026-05-18

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
| 6 — AI single-shot plugin | 🔜 Next | builds on agent::build_provider from Phase 5 |
| 7 — Tool registry + manifest schema | Pending | `agent_callable` + `destructive` |
| 8 — AI tool-use plugin + dry-run plan | Pending | Headline feature |
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

## Next

See the **Now / Next / Later** section in `.docs/ai/roadmap.md`. Phase 6 (AI single-shot plugin — `examples/plugins/ai/ask.lua` surfacing `agent::build_provider` in the TUI) is queued. Taylor is still smoke-testing Phase 4.5 (Mail compose, Mail mutating actions, mobile-width thresholds) and Phase 5 (real-API provider calls) in parallel.
