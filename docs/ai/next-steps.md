# Next Steps

> Updated: 2026-04-03

## Plugin Deep-Dive (continue)

- [x] Git: richer status, branches, log, stash (v0.4.0)
- [x] Docker: full Portainer-style — 6 commands with stats, logs, exec, compose, networks, system (v0.5.0)
- [ ] Kubernetes: log streaming, describe pod, context switching
- [ ] SSH: connection status, recent connections
- [ ] Weather: forecast view, location setting
- [ ] GitHub: review request count badge, workflow status icons

## Features

- [ ] Publish to crates.io (`cargo install larkline`)
- [ ] lark.nvim: action dispatch back to Neovim (file search opens buffers)
- [ ] Widget card Enter to drill into the full command output
- [ ] `lark plugin sync` should update existing plugins (currently skips if directory exists)

## UX Polish

- [ ] Widget picker: add search/filter when there are many widget-eligible commands
- [ ] Update checker: make the status bar hint clickable or add a power menu action to run the upgrade command

## Tech Debt

- [ ] HA plugin boilerplate: 21 files with duplicated get_config/headers/filters — consider a shared Lua module if sandbox gains require() support
- [ ] Widget refresh timer could be smarter (skip when not visible)
- [ ] Compose plugin action helper could be simplified — currently builds args arrays inline

## Release Notes (unpushed since v0.5.0)

- Widget picker overlay (A key)
- Widget discoverability status bar hints
- These are committed but not yet released — next release will include them
