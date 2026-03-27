# Larkline Roadmap

## Vision

Terminal-native Raycast: a keyboard-driven command palette for personal productivity plugins. Launch, search, act, dismiss.

## Completed Milestones

| Phase | Summary |
|-------|---------|
| 0-4.5 | Core scaffold, Plugin trait, TUI, fuzzy search, favorites, keybindings, vim modes |
| 5 | Embedded Lua (mlua 5.4), sandboxed VM, `lark.*` host API |
| 6 | Distribution: `--help`, `init-plugin` scaffolder, Homebrew formula |
| 7 | Enhanced output: ANSI, shell actions, tables, streaming, Nerd Font icons |
| 8 | Unified search: prefetch cache, nucleo filter, flash messages |
| 9 | Global item ranking, match highlighting, RunPlugin rows |
| 10-16 | Forms, action palette, multi-command plugins, output search, markdown rendering |
| 17-22 | Plugin store, sidebar toggle, copy menu, output filter |
| 23-24 | Multi-repo Git, command history, recent section |
| 25-26 | Plugin settings UI, theme presets + TUI switcher |
| 27-28 | DevOps plugins (k8s, ports, kill-process), macOS plugins (calendar, apps, clipboard) |
| 29 | Raycast-style flat list: removed group headers, inline plugin name badges |

## Current Priority: UX Polish (pre-release)

Taylor considers the feature set an "usable MVP" but not daily-driver ready. Polish and UX consistency are the blockers, not features.

- [x] Flat list with inline plugin name (Phase 29)
- [x] Hide descriptions by default (toggle with `d`)
- [x] Quickkey exact-match priority in search
- [x] Normal mode on Back/Enter from ViewOutput
- [ ] Sidebar shrinks to ~2/7 when drilled into a plugin (currently 2/3)
- [ ] Every plugin must have an icon (audit + fill gaps)
- [ ] Arrow keys behave like hjkl everywhere (l/right = drill in, h/left = back)

## Next Features (post-polish)

- **Standard plugin library** — identify gaps vs Raycast, build core plugins
- **Secrets handling** — .env file or keychain integration for API keys
- **Publishing** — Homebrew + `cargo install`, proper semantic versioning

## Constraints

- Terminal only. No GUI dependencies.
- Sub-100ms startup.
- The `Plugin` trait is the only interface between backends and the engine.
- TUI reads state, never owns it. State transitions happen in `app.rs`.

## Non-Goals

- GUI/Electron wrapper
- Plugin marketplace / remote registry (local-first)
- Mouse-only workflows
