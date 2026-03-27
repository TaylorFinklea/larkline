# Next Steps

> Updated: 2026-03-27

## Immediate (UX polish)

- [ ] Sidebar width: shrink to ~2/7 when in ViewOutput, keep 2/3 when browsing
- [ ] Arrow key parity: ensure Right/Left map to l/h (drill-in / back) in all modes
- [ ] Plugin icon audit: ensure every plugin has a non-empty `icon_nerd` or `icon`
- [ ] Esc flow review: Esc from ViewOutput -> main list (highlight current) -> Esc clears search -> Normal mode

## After Polish

- [ ] Secrets handling: decide on .env vs keychain integration, implement for plugins needing API keys
- [ ] Standard plugin gaps: audit vs Raycast, identify missing core plugins
- [ ] Publishing prep: version bump, changelog, Homebrew formula update, `cargo install` validation
