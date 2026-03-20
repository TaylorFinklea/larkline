#!/usr/bin/env bash
# Top Processes — 20 entries by CPU with table output.
# Uses jq for all JSON construction (no shell variable interpolation in JSON).

items=()

while IFS= read -r line; do
  pid=$(echo "$line" | awk '{print $1}')
  cpu=$(echo "$line" | awk '{print $2}')
  mem_kb=$(echo "$line" | awk '{print $3}')
  mem_mb=$(( mem_kb / 1024 ))
  name=$(echo "$line" | awk '{$1=$2=$3=""; sub(/^[[:space:]]+/, ""); print}')
  [[ -z "$name" ]] && continue
  short=$(basename "${name%% *}")
  item=$(jq -n \
    --arg label "$short" \
    --arg pid "$pid" \
    --arg cpu "$cpu" \
    --arg mem "${mem_mb}MB" \
    '{label: $label, metadata: {pid: $pid, cpu: $cpu, mem: $mem}}')
  items+=("$item")
done < <(ps aux | sort -k3 -rn | awk 'NR>1 {printf "%s %s %s ", $2, $3, $6; for(i=11;i<=NF;i++) printf "%s ", $i; print ""}' | head -20)

# Build the full output with columns for table rendering.
items_json=$(printf '%s' "${items[0]}")
for ((i=1; i<${#items[@]}; i++)); do
  items_json+=",${items[$i]}"
done

jq -n \
  --argjson items "[$items_json]" \
  '{
    title: "Top Processes",
    columns: [
      {header: "Process", key: "label"},
      {header: "PID", key: "pid", align: "right"},
      {header: "%CPU", key: "cpu", align: "right"},
      {header: "Memory", key: "mem", align: "right"}
    ],
    items: $items
  }'
