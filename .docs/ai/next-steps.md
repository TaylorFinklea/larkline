# Next Steps

> Updated: 2026-04-18

## v0.10.0 — Complete

All 8 phases complete:
- [x] A: Tracing to file during TUI
- [x] B: `plugin sync` repair + `--force`
- [x] C: crates.io publish wiring
- [x] D: Install docs in README
- [x] E: AUR PKGBUILD
- [x] F: Nix flake
- [x] G: lark.nvim v2 (`$NVIM` socket awareness)
- [x] H: `release.sh set` mode + handoff docs

**To release:**

```sh
# One-time repo setup:
#   1. Create crates.io API token, add as `CRATES_IO_TOKEN` secret.
#   2. Generate flake.lock on a machine with nix installed and commit.

bash scripts/release.sh set 0.10.0
```

This bumps `Cargo.toml` + `Formula/larkline.rb`, commits, tags `v0.10.0`, and pushes. CI then:
1. Builds binaries for macOS (ARM + x86) and Linux.
2. Creates the GitHub Release with tarballs.
3. Auto-updates the Homebrew tap with SHA256 values.
4. Publishes to crates.io (gated on the secret; fails gracefully if missing).

After tag: push the AUR PKGBUILD manually per `packaging/aur/README.md`.

## Backlog

See `.docs/ai/roadmap.md` → Backlog section.

## Future Themes

- **Telescope integration for lark.nvim v3** — requires a new `lark list --json` headless subcommand, then a Telescope source module.
- **Streaming `on_action` results back to nvim** — real-time buffer updates driven by mini-app output.
- **`cargo-dist`** — evaluate replacing `scripts/release.sh` + `release.yml` with cargo-dist for unified cross-platform release management.
- **SBOM / `cargo-auditable`** for published binaries.
