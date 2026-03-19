#!/usr/bin/env bash
# IP Addresses — IPv4 only. Public first, then local interfaces.
# Skips: loopback, tunnels (utun/gif/stf/ipsec/tun/tap), bridges,
#        AirDrop (awdl), low-latency WLAN (llw), access points (ap),
#        Apple private interfaces (anpi), and link-local (169.254.x.x).

items=()

# ── Public IP ────────────────────────────────────────────────────────────────
public_ip=$(curl -4 -s --connect-timeout 3 --max-time 5 ip.me 2>/dev/null | tr -d '[:space:]')
if [[ "$public_ip" =~ ^[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$ ]]; then
    items+=(
        "{\"label\":\"${public_ip}\",\"detail\":\"public\",\"icon\":\"🌍\",\"actions\":[{\"id\":\"copy\",\"label\":\"Copy\",\"command\":\"clipboard\",\"args\":[\"${public_ip}\"]}]}"
    )
fi

# ── Local interfaces ─────────────────────────────────────────────────────────
current_iface=""
while IFS= read -r line; do
    # Capture interface name from lines like "en0: flags=..."
    if [[ "$line" =~ ^([A-Za-z0-9]+): ]]; then
        current_iface="${BASH_REMATCH[1]}"
        continue
    fi

    # Only process "inet x.x.x.x" lines (not inet6)
    [[ "$line" =~ ^[[:space:]]+inet[[:space:]]+([0-9]+\.[0-9]+\.[0-9]+\.[0-9]+) ]] || continue
    ip="${BASH_REMATCH[1]}"

    # Skip unwanted interface types
    case "$current_iface" in
        lo*|gif*|stf*|anpi*|bridge*|utun*|ipsec*|tun*|tap*|awdl*|llw*|ap*|vmenet*|vnet*|docker*|virbr*|vboxnet*|veth*)
            continue ;;
    esac

    # Skip loopback and link-local ranges
    [[ "$ip" == 127.* || "$ip" == 169.254.* ]] && continue

    items+=(
        "{\"label\":\"${ip}\",\"detail\":\"${current_iface}\",\"icon\":\"🖥\",\"actions\":[{\"id\":\"copy\",\"label\":\"Copy\",\"command\":\"clipboard\",\"args\":[\"${ip}\"]}]}"
    )
done < <(ifconfig 2>/dev/null)

# ── Output ───────────────────────────────────────────────────────────────────
printf '{\n  "title": "IP Addresses",\n  "items": ['
for i in "${!items[@]}"; do
    [[ $i -gt 0 ]] && printf ','
    printf '\n    %s' "${items[$i]}"
done
printf '\n  ]\n}\n'
