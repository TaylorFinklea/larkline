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
