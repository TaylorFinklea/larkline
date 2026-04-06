---
name: release
description: Run the automated release pipeline. Mechanical — just runs a script.
model: haiku
---

# Release

Run the larkline release pipeline. This is a mechanical task — no judgment needed.

## Steps

1. Run `bash scripts/release.sh patch` (or `minor`/`major` if instructed).
2. Report the version from the `RELEASE_VERSION=` line in the output.
3. If it fails, report the full error output. Do not retry or attempt to fix.

## Notes

- The script validates (test, clippy, fmt) before bumping.
- It commits, tags, and pushes — CI handles the rest.
- Do not modify any files. Just run the script and report results.
