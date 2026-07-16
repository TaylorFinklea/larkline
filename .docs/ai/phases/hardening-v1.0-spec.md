# v1.0 Hardening Program — Spec

> Status: active. Backlog: beads epic `larkline-mkv` (31 child items). Decision record: `decisions.md` ADR-012. Branch: `v1.0-prep`.
> Authored 2026-07-15 from an architecture map (8 subsystem readers) + a 94-agent bug bash (59 confirmed findings, 22/22 architecture claims confirmed) + two GPT-5.6-Sol (max) adversarial passes.

## Goal

Make larkline feel **solid and performant** before v1.0 launch. Felt symptoms: input/render lag, plugin/widget slowness. Approach: fix the ~8 structural *generators* (see ADR-012), not more instances — prior sweeps (48 fixes, then 22) fixed instances while generators kept producing new ones.

## Constraints (unchanged)

- Terminal only; sub-100ms startup; TUI reads state, never owns it; the `Plugin` trait is the intended sole backend/engine interface (currently violated by the streaming path — see W2.4).
- Solo maintainer + AI coding agents. v1.0 launch must stay viable.

## The plan (6 milestones = beads `larkline-mkv.1`..`.32`)

### Week 1 — Instrument + amplification + crash containment
Make the lag measurable and stop the aborts. `mkv.1` instrumentation (lands first — every later perf fix is a measured before/after), `mkv.2` ANSI parse cache, `mkv.3` scoped markdown invalidation, `mkv.4` preserve Ready on refresh (no Loading flash), `mkv.5` single-flight refresh + seed startup timestamps (kills the double-dispatch), `mkv.6` remove `panic="abort"` + terminal-restoring panic hook (P0), `mkv.7` fix the four confirmed panic sites.

### Week 2 — Stable identity + execution model (the structural gate)
`mkv.9` registry generation + execution IDs on the existing index, reject stale completions (P0, fixes the foreground-hijack class). `mkv.10` single cache-state owner + uniform SWR (P0, fixes streaming-default-wipe). `mkv.11` in-flight dedup + route streaming through the Plugin trait. Full stable plugin_id/command_id migration (`mkv.8`) is deferred to v1.1 and is not a launch dependency.

### Week 3 — UI-task isolation (never block render/input)
`mkv.12` async background-command path for shell/nvim/editor (gated on `.9`). `mkv.13` async RefreshPlugins + plugin-manager scans/keychain (gated on `.11`). `mkv.14` in-memory history + secret-availability cache. `mkv.15` defer/parallelize startup keychain behind first paint.

### Week 4 — Persistence + store discipline + state invariants
`mkv.16` atomic-write helper across all persistence + stop fail-open wipes. `mkv.17` unify store discipline + single-flight OAuth refresh. `mkv.18` derived-state selectors + focused-surface enum (ends the dominant desync class). `mkv.19` theme coherence.

### Week 5 — Agent minimum launch hardening
`mkv.20` provider stream deadlines. `mkv.21` safe pre-output retry/backoff. `mkv.22` OpenAI terminal-event + max_tokens handling. `mkv.23` agent destructive-approval + audit gaps. `mkv.24` honest tool-arg contract (real args or documented single-shot) + agent-ask `--` terminator. `mkv.25` manifest secret-scope audit → enforce (two-step).

### Week 6 — Launch-surface coherence + rehearsal
`mkv.26` disabled-plugin parity across headless surfaces. `mkv.27` pin plugin sync to a compatible release + atomic sync (supply-chain). `mkv.28` version the lark.nvim wire contract + hermetic cli tests. `mkv.29` onboarding correctness. `mkv.30` packaging/release gates. `mkv.31` plugin fix-it batch (HA/Bitwarden/Docker/k8s/Atlassian/clipboard + confirm:true). `mkv.32` full v1.0 launch rehearsal + Taylor-gated QA (go/no-go).

## Sequencing gates (do not reorder)
- `mkv.9` **before** `mkv.12`/`.13` (async dispatch) — moving work async before execution identity exists creates a new late-completion focus-steal class.
- `mkv.8` remains the long-term identity migration, but the Sol launch cut explicitly removed it as a dependency of the generation-stamped `mkv.9` slice; do not restore that gate for v1.0.
- secret audit **before** enforcement within `mkv.25`.

## Confirmed discrete bugs (fold-ins, all verified)
6 HIGH: exec_io inherits raw-mode TTY stdin (lua.rs:160); OpenAI provider swallows terminal error events (openai.rs:332); OAuth ephemeral-port vs exact redirect URI (oauth.rs:282); HA favorite/hide writes unreadable JSON (ha-manage.sh:28); Bitwarden copy_text = raw password shown on screen (items.lua:366); Docker/k8s ship never-terminating `-f`/`-it`/`stats` actions (containers.lua:249). Plus ~35 mediums distributed across the milestones (see bead descriptions).

## Explicitly NOT worked (refuted / demoted)
parse_key SHIFT drop (refuted); session replay parallel-tool-result split (refuted); widget_disable selection clamp (refuted); quadratic-streaming-markdown (refuted); script-plugins-write-store-directly (refuted); opt-level/syntect-preload (unmeasured — benchmark first, do not assume).

## Sol implementation-pass adjustments (2026-07-15)

A second GPT-5.6-Sol pass reviewed the concrete plan + the 41 high/medium bugs and produced a launch-viability cut list. Applied to beads:

- **Stable-ID migration (`mkv.8`) DEFERRED to v1.1 (now P3)** — architecturally the gate (ADR-012 G5) but too much persistence/contract blast radius right before launch. The launch-critical need (reject stale completions → kill the foreground-hijack class) is met by `mkv.9`'s **minimal per-dispatch execution-id + generation counter on the existing `usize` index**, which does NOT require stable IDs. `mkv.9`'s hard dependency on `mkv.8` was removed; the P0 chain is now `mkv.9` → `mkv.10`, both independent of the deferred migration.
- **Streaming split (`mkv.11`)** — launch = route streaming through the trait + fix the Lua misroute + surface exit status; defer the stable-ID in-flight dedup registry (a lightweight per-usize guard covers single-flight for launch).
- **W1 soft-deps noted, not blocked** — `mkv.3`/`.4`/`.5` get partially revisited when W2 lands; ship them in W1 for the immediate felt win and accept minor rework (Sol Q4: ship Week 1 first — broadest immediate improvement).
- **Deferred past v1.0 (P3):** `mkv.19` theme coherence (polish), `mkv.21` retry/backoff (deadlines + honest errors suffice; retry adds duplicate-execution risk), `mkv.28` wire-version field (defer unless the contract changes — keep only the hermetic `cli_action_test` fix).
- **Scope cuts:** `mkv.18` = fix the confirmed widget/glance mismatches directly, defer the full selector/focused-surface refactor. `mkv.24` = ship the agent-ask `--` terminator + honest zero-arg positioning, defer real tool-arg schema plumbing to v1.1 (product expansion, not hardening). `mkv.31` = fix security/hang/`confirm:true` + cheap integration one-liners; disable rather than deep-repair anything heavier.
- **HIGH-bug confirmations:** all 6 re-confirmed against code; HA favorite/hide reseveritied high→medium (narrow scope).

**Sol's "one week if only one" = Week 1** (`mkv.2`–`mkv.7`, with `mkv.1` reduced to timing logs): fewer reparses, less invalidation churn, no refresh flashing/double-dispatch, recoverable panics, and the known crash/blank-pane fixes — the broadest immediate felt improvement.

## Verification
Every bead carries a `verify_cmd` (mostly `cargo test`; startup/packaging ones use `cargo build --release`/`nix build`; QA uses a manual runbook). Note the existing `cli_action_test` is non-hermetic (depends on the user's installed plugins) — fixed in `mkv.28`.
