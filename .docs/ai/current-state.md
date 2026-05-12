# Current State

> Updated: 2026-05-12

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
| 3 — Calendar v2 plugin | 🔜 Next | 1.5w budget; rewrite `examples/plugins/calendar/` to structured items + helper subprocess + icalbuddy fallback |
| 4 — Mail plugin (osascript) | Pending | 4w budget — biggest unknown |
| 5 — AI Provider trait + 4 backends | Pending | Anthropic + OpenAI + OpenRouter + Ollama |
| 6 — AI single-shot plugin | Pending | |
| 7 — Tool registry + manifest schema | Pending | `agent_callable` + `destructive` |
| 8 — AI tool-use plugin + dry-run plan | Pending | Headline feature |
| 9 — Web search shortcuts + onboarding wizard | Pending | |
| 10 — QA pass + bug sweep + theme polish | Pending | |
| 11 — Beta + Medium draft + launch prep | Pending | |
| 12 — Tag v1.0 + Medium post + Show HN | Pending | |

## Active Branches

- `main` — at `bbbe3b2` (Release v0.15.0); pushed to origin
- `v1.0-prep` — branched off main; 5 Phase 2 commits + Phase 2.G handoff commit (pending). Local-only until v1.0 ships.

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

## Next

See the **Now / Next / Later** section in `.docs/ai/roadmap.md`. Phase 3 (Calendar v2 plugin) is queued.
