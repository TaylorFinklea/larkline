# current-state.md template

`current-state.md` is a **loop-state** file: ≤20 lines, fragments only — not a
journal. It holds only what the NEXT iteration needs to resume. Each other kind
of content has exactly ONE home (see the AGENTS.md routing table):

- What happened + why → `git log` + `decisions.md`
- Pending actions / discovered tasks → `roadmap.md` Now (caveats inline)
- Multi-session design detail → `phases/<slug>-spec.md`

Copy the shape below. Keep it terse.

---

**Branch:** `<branch>` — `<ahead/behind origin>`; `<build/test status in one line>`

## Plan

<!-- The ONE active roadmap Now item, expanded into phase checkboxes.
     Empty when no item is active. Each step carries its own Verify. -->

- [ ] First step — Verify: `<command>` or `human: <named check>`
- [ ] Second step — Verify: `<command>`
- [?] Step awaiting human verify

## Blockers

- `<fragment: what's blocked + why>`

## Open questions

- `<product/design decision needing the user>`
