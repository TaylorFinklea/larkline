# Session Handoff Template

Copy this template and fill it in at the end of each work session. Update `current-state.md` and `next-steps.md` with the same information.

---

## Session Summary

**Date:** YYYY-MM-DD
**Branch:** main (or feature branch name)
**Commits this session:**
- `abc1234` Short description
- `def5678` Short description

## What Changed

- Bullet list of functional changes, not file-level diffs
- Focus on behavior the next session needs to know about

## Files Modified

- `src/app.rs` — what changed and why
- `src/tui/ui.rs` — what changed and why

## Decisions Made

If any architectural decisions were made, add them to `docs/ai/decisions.md` as a new ADR entry.

## Current State

- Tests: passing / failing (which?)
- Clippy: clean / warnings (which?)
- Branch: clean / uncommitted changes

## Next Session Should

1. Exact first action (e.g. "Implement sidebar width ratio change in `src/tui/ui.rs`")
2. Second action
3. Third action

## Open Questions for Taylor

- Any product decisions that need Taylor's input before proceeding
