#!/bin/bash
# PostToolUse hook: detect milestone feature commits and signal for release.
# Triggers on commit messages matching: feat(m<digit>)
INPUT=$(cat)
OUTPUT=$(echo "$INPUT" | jq -r '.tool_output // empty')

if echo "$OUTPUT" | grep -qE 'feat\(m[0-9]'; then
  echo 'AUTO_RELEASE: Milestone feature committed — dispatch release agent'
fi

exit 0
