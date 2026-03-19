#!/usr/bin/env bash
# Hello World — the simplest possible Larkline plugin.
# Demonstrates the JSON output contract.

cat <<'EOF'
{
  "title": "Hello from Larkline!",
  "items": [
    {
      "label": "Hello, World!",
      "detail": "This is the simplest possible plugin",
      "icon": "👋",
      "actions": [
        {
          "id": "copy",
          "label": "Copy greeting",
          "command": "clipboard",
          "args": ["Hello, World!"]
        }
      ]
    },
    {
      "label": "Visit the docs",
      "detail": "Learn how to write your own plugins",
      "icon": "📖",
      "url": "https://github.com/tfinklea/larkline",
      "actions": [
        {
          "id": "open",
          "label": "Open in browser",
          "command": "open",
          "args": ["https://github.com/tfinklea/larkline"]
        }
      ]
    }
  ]
}
EOF
