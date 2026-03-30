#!/bin/bash
# release.sh — bump version, commit, tag, push, and let CI do the rest.
#
# Usage: ./scripts/release.sh <major|minor|patch>
# Example: ./scripts/release.sh patch  →  0.3.1 → 0.3.2

set -euo pipefail

BUMP="${1:-}"
if [[ -z "$BUMP" || ! "$BUMP" =~ ^(major|minor|patch)$ ]]; then
    echo "Usage: ./scripts/release.sh <major|minor|patch>"
    exit 1
fi

# Ensure working tree is clean.
if [[ -n "$(git status --porcelain)" ]]; then
    echo "Error: working tree is not clean. Commit or stash changes first."
    exit 1
fi

# Read current version from Cargo.toml.
CURRENT=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

case "$BUMP" in
    major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
    minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
    patch) PATCH=$((PATCH + 1)) ;;
esac

NEW="${MAJOR}.${MINOR}.${PATCH}"
echo "Bumping $CURRENT → $NEW"

# Update Cargo.toml version.
sed -i '' "s/^version = \"$CURRENT\"/version = \"$NEW\"/" Cargo.toml

# Update Formula version + reset SHA256 placeholders.
sed -i '' "s/version \"$CURRENT\"/version \"$NEW\"/" Formula/larkline.rb
sed -i '' 's/sha256 "[a-f0-9]\{64\}"/sha256 "PLACEHOLDER"/g' Formula/larkline.rb

# Update Cargo.lock.
cargo check --quiet 2>/dev/null || true

# Commit, tag, push.
git add Cargo.toml Cargo.lock Formula/larkline.rb
git commit -m "Release v${NEW}"
git tag "v${NEW}"
git push origin main
git push origin "v${NEW}"

echo ""
echo "✅ v${NEW} released!"
echo ""
echo "CI will:"
echo "  1. Build binaries for macOS (ARM + x86) and Linux"
echo "  2. Create GitHub Release with tarballs"
echo "  3. Auto-update the Homebrew tap with SHA256 values"
echo ""
echo "Users upgrade with: brew upgrade larkline"
echo "Plugins update with: lark plugin sync"
