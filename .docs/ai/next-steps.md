# Next Steps

> Updated: 2026-04-20

## v0.10.0 — Shipped ✅

Tag `v0.10.0` (commit `627b8fc`) pushed on 2026-04-19.

Pipeline status (check before next release):

- GitHub Release with macOS + Linux binaries
- Homebrew tap formula bump
- crates.io publish (requires `CRATES_IO_TOKEN` secret)
- AUR PKGBUILD publish — **manual step** per `packaging/aur/README.md`
- `flake.lock` — regenerate on a nix-equipped machine whenever convenient

## v0.11.0 — Ready to Cut

All 7 phases complete. Accumulated since v0.10.0:

1. Input loop fix for Ghostty / Kitty-protocol terminals (commit `fa0eeff`)
2. Bitwarden deep-dive plugin (7 new files under `examples/plugins/bitwarden/`)

**To release:**

```sh
bash scripts/release.sh set 0.11.0
```

This bumps `Cargo.toml` + `Formula/larkline.rb`, commits, tags `v0.11.0`, and pushes. CI takes it from there.

**Before running the script:** verify the bitwarden plugin loads in practice. Steps:

```sh
# Install the plugin to the user's config dir
mkdir -p ~/.config/larkline/plugins
ln -sfn "$(pwd)/examples/plugins/bitwarden" ~/.config/larkline/plugins/bitwarden

# Unlock bw (one-time)
export BW_SESSION=$(bw unlock --raw)

# Launch larkline and try: Search Vault, Favorites, Folders, Generate Password,
# Sync Vault, Lock Vault.
lark
```

## v0.12.0 — Jira + Confluence (next theme)

Scoped in `.docs/ai/roadmap.md` → v0.12.0 section. Requires an OAuth UX decision:

- **Option 1:** local HTTP callback listener for the authorization code flow
- **Option 2:** device-code flow (user pastes code into browser)

Suggest discussing the auth flow before starting Phase A of that milestone.

## Backlog

See `.docs/ai/roadmap.md` → Backlog section.

Newly added:

- **Bitwarden backlog** — rbw support, Send, org/collection filtering, attachments, edit/delete actions, lock-after-clipboard, TOTP countdown in detail view

## Future Themes

- **Telescope integration for lark.nvim v3** — requires `lark list --json` headless subcommand, then a Telescope source module.
- **Streaming `on_action` results back to nvim** — real-time buffer updates driven by mini-app output.
- **`cargo-dist`** — evaluate replacing `scripts/release.sh` + `release.yml` with cargo-dist for unified cross-platform release management.
- **SBOM / `cargo-auditable`** for published binaries.
