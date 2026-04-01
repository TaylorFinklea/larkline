# Next Steps

> Updated: 2026-03-31

## Plugin Deep-Dive (continue)

- [ ] Git Status: add more detail (branch name, ahead/behind, stash count)
- [ ] GitHub: review request count badge, workflow status icons
- [ ] Docker: container logs action, image prune action
- [ ] Kubernetes: log streaming, describe pod
- [ ] SSH: connection status, recent connections
- [ ] Weather: forecast view, location setting

## Features

- [ ] Publish to crates.io (`cargo install larkline`)
- [ ] lark.nvim: action dispatch back to Neovim (file search opens buffers)
- [ ] Widget card Enter to drill in (currently h/l only navigates)
- [ ] v0.4.0 release with all widget + plugin manager features

## Tech Debt

- [ ] HA plugin boilerplate: 21 files with duplicated get_config/headers/filters — consider a shared Lua module if sandbox gains require() support
- [ ] Widget refresh timer could be smarter (skip when not visible)
