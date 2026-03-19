#!/usr/bin/env bash
# System Info — demonstrates a plugin that reads real system data.
# Uses standard macOS/Linux commands; gracefully degrades if commands are unavailable.

# Memory (macOS)
if command -v vm_stat &>/dev/null; then
  page_size=$(pagesize 2>/dev/null || echo 4096)
  free_pages=$(vm_stat | awk '/Pages free/ { gsub(/\./, "", $3); print $3 }')
  free_mb=$(( free_pages * page_size / 1024 / 1024 ))
  mem_detail="~${free_mb}MB free"
else
  mem_detail="unavailable on this platform"
fi

# Disk (root filesystem)
if command -v df &>/dev/null; then
  disk_info=$(df -h / | awk 'NR==2 { print $3 " used / " $2 " total (" $5 " full)" }')
else
  disk_info="unavailable"
fi

# Load average
if [ -f /proc/loadavg ]; then
  load=$(cut -d' ' -f1-3 /proc/loadavg)
elif command -v sysctl &>/dev/null; then
  load=$(sysctl -n vm.loadavg 2>/dev/null | tr -d '{}' | xargs)
else
  load="unavailable"
fi

# Hostname
host=$(hostname -s 2>/dev/null || echo "unknown")

cat <<EOF
{
  "title": "System Info — $host",
  "items": [
    {
      "label": "Memory",
      "detail": "$mem_detail",
      "icon": "🧠"
    },
    {
      "label": "Disk (root)",
      "detail": "$disk_info",
      "icon": "💾"
    },
    {
      "label": "Load Average",
      "detail": "$load",
      "icon": "📈"
    }
  ]
}
EOF
