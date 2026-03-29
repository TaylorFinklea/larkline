#!/bin/bash
# ha-manage.sh — toggle an entity in favorites or hidden_entities list.
# Usage: ha-manage.sh <favorite|hide> <entity_id>
#
# Reads/writes ~/.local/share/larkline/stores/home_assistant.json directly.

set -euo pipefail

ACTION="${1:-}"
ENTITY_ID="${2:-}"
STORE="${XDG_DATA_HOME:-$HOME/.local/share}/larkline/stores/home_assistant.json"

if [ -z "$ACTION" ] || [ -z "$ENTITY_ID" ]; then
    echo "Usage: ha-manage.sh <favorite|unfavorite|hide|unhide> <entity_id>"
    exit 1
fi

# Ensure store file exists.
mkdir -p "$(dirname "$STORE")"
if [ ! -f "$STORE" ]; then
    echo '{}' > "$STORE"
fi

case "$ACTION" in
    favorite)
        KEY="favorites"
        # Add entity_id to the array (if not already present).
        jq --arg eid "$ENTITY_ID" \
            'if (.[$key] // []) | map(select(. == $eid)) | length > 0
             then .
             else .[$key] = ((.[$key] // []) + [$eid])
             end' --arg key "$KEY" "$STORE" > "${STORE}.tmp" && mv "${STORE}.tmp" "$STORE"
        echo "⭐ Added $ENTITY_ID to favorites"
        ;;
    unfavorite)
        KEY="favorites"
        jq --arg eid "$ENTITY_ID" --arg key "$KEY" \
            '.[$key] = ([.[$key] // [] | .[] | select(. != $eid)])' "$STORE" > "${STORE}.tmp" && mv "${STORE}.tmp" "$STORE"
        echo "Removed $ENTITY_ID from favorites"
        ;;
    hide)
        KEY="hidden_entities"
        jq --arg eid "$ENTITY_ID" \
            'if (.[$key] // []) | map(select(. == $eid)) | length > 0
             then .
             else .[$key] = ((.[$key] // []) + [$eid])
             end' --arg key "$KEY" "$STORE" > "${STORE}.tmp" && mv "${STORE}.tmp" "$STORE"
        echo "🚫 Hidden $ENTITY_ID"
        ;;
    unhide)
        KEY="hidden_entities"
        jq --arg eid "$ENTITY_ID" --arg key "$KEY" \
            '.[$key] = ([.[$key] // [] | .[] | select(. != $eid)])' "$STORE" > "${STORE}.tmp" && mv "${STORE}.tmp" "$STORE"
        echo "Unhidden $ENTITY_ID"
        ;;
    *)
        echo "Unknown action: $ACTION"
        exit 1
        ;;
esac
