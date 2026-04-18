#!/bin/bash
# release.sh — validate, bump version, commit, tag, push, and let CI do the rest.
#
# Usage:
#   ./scripts/release.sh patch          # 0.5.0 → 0.5.1
#   ./scripts/release.sh minor          # 0.5.0 → 0.6.0
#   ./scripts/release.sh major          # 0.5.0 → 1.0.0
#   ./scripts/release.sh set 0.10.0     # explicit — jumps past skipped versions
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
EXPLICIT_VERSION=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        patch|--patch) BUMP="patch"; shift ;;
        minor|--minor) BUMP="minor"; shift ;;
        major|--major) BUMP="major"; shift ;;
        set)
            shift
            if [[ -z "${1:-}" ]]; then
                echo "Error: 'set' requires a version argument (e.g. set 0.10.0)"
                exit 1
            fi
            if [[ ! "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
                echo "Error: version '$1' is not semver X.Y.Z"
                exit 1
            fi
            BUMP="set"
            EXPLICIT_VERSION="$1"
            shift
            ;;
        --help|-h) echo "Usage: ./scripts/release.sh <patch|minor|major|set X.Y.Z>"; exit 0 ;;
        *) echo "Unknown argument: $1"; echo "Usage: ./scripts/release.sh <patch|minor|major|set X.Y.Z>"; exit 1 ;;
    esac
done

if [[ -z "$BUMP" ]]; then
    echo "Usage: ./scripts/release.sh <patch|minor|major|set X.Y.Z>"
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
    set)
        # Explicit version — already validated as semver above.
        IFS='.' read -r MAJOR MINOR PATCH_NUM <<< "$EXPLICIT_VERSION"
        ;;
esac

NEW="${MAJOR}.${MINOR}.${PATCH_NUM}"

# Guard against accidental same-or-lower version.
if [[ "$BUMP" == "set" ]]; then
    if [[ "$NEW" == "$CURRENT" ]]; then
        echo "Error: target version $NEW equals current version; nothing to do."
        exit 1
    fi
fi
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
