#!/bin/bash
# release.sh — validate, bump version, commit, tag, push, and let CI do the rest.
#
# Usage:
#   ./scripts/release.sh patch          # 0.5.0 → 0.5.1
#   ./scripts/release.sh minor          # 0.5.0 → 0.6.0
#   ./scripts/release.sh major          # 0.5.0 → 1.0.0
#   ./scripts/release.sh --patch        # flag syntax also works
#   ./scripts/release.sh --minor
#
# CI pipeline (triggered by v* tag push):
#   1. Builds binaries for macOS (ARM + x86) and Linux
#   2. Creates GitHub Release with tarballs
#   3. Auto-updates Homebrew tap with SHA256 values

set -euo pipefail

# --- Parse arguments ---
BUMP=""
for arg in "$@"; do
    case "$arg" in
        patch|--patch) BUMP="patch" ;;
        minor|--minor) BUMP="minor" ;;
        major|--major) BUMP="major" ;;
        --help|-h) echo "Usage: ./scripts/release.sh <patch|minor|major>"; exit 0 ;;
        *) echo "Unknown argument: $arg"; echo "Usage: ./scripts/release.sh <patch|minor|major>"; exit 1 ;;
    esac
done

if [[ -z "$BUMP" ]]; then
    echo "Usage: ./scripts/release.sh <patch|minor|major>"
    exit 1
fi

# --- Pre-flight checks ---
echo "==> Pre-flight checks"

# Ensure working tree is clean.
if [[ -n "$(git status --porcelain)" ]]; then
    echo "Error: working tree is not clean. Commit or stash changes first."
    exit 1
fi

# Ensure we're on main.
BRANCH=$(git branch --show-current)
if [[ "$BRANCH" != "main" ]]; then
    echo "Error: releases must be cut from main (currently on '$BRANCH')."
    exit 1
fi

echo "==> Running tests"
cargo test --quiet

echo "==> Running clippy"
cargo clippy --quiet -- -D warnings

echo "==> Checking format"
cargo fmt -- --check

echo "    All checks passed."

# --- Bump version ---
CURRENT=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH_NUM <<< "$CURRENT"

case "$BUMP" in
    major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH_NUM=0 ;;
    minor) MINOR=$((MINOR + 1)); PATCH_NUM=0 ;;
    patch) PATCH_NUM=$((PATCH_NUM + 1)) ;;
esac

NEW="${MAJOR}.${MINOR}.${PATCH_NUM}"
echo ""
echo "==> Bumping $CURRENT → $NEW"

# Update Cargo.toml version.
sed -i '' "s/^version = \"$CURRENT\"/version = \"$NEW\"/" Cargo.toml

# Update Formula version + reset SHA256 placeholders.
sed -i '' "s/version \"$CURRENT\"/version \"$NEW\"/" Formula/larkline.rb
sed -i '' 's/sha256 "[a-f0-9]\{64\}"/sha256 "PLACEHOLDER"/g' Formula/larkline.rb

# Update Cargo.lock.
cargo check --quiet 2>/dev/null || true

# --- Commit, tag, push ---
echo "==> Committing and tagging"
git add Cargo.toml Cargo.lock Formula/larkline.rb
git commit -m "Release v${NEW}"
git tag "v${NEW}"

echo "==> Pushing to origin"
git push origin main
git push origin "v${NEW}"

echo ""
echo "RELEASE_VERSION=v${NEW}"
echo ""
echo "CI will:"
echo "  1. Build binaries for macOS (ARM + x86) and Linux"
echo "  2. Create GitHub Release with tarballs"
echo "  3. Auto-update the Homebrew tap with SHA256 values"
echo ""
echo "Users upgrade with: brew upgrade larkline"
