-- Tailscale: shared helpers for status fetching and device classification.
-- SYNC INSTRUCTIONS: Copy helpers into each command file that uses them
-- (sandbox has no require). This file is the canonical source.

-- Fetch tailscale status JSON. Returns (data, nil) or (nil, error_items).
local function fetch_status()
    local which = lark.exec("which", { "tailscale" })
    if not which or which == "" then
        return nil, {
            { label = "tailscale not installed", detail = "brew install tailscale", icon = "!" },
        }
    end

    local raw = lark.exec("tailscale", { "status", "--json" })
    if not raw or raw == "" then
        return nil, {
            { label = "tailscale not running", detail = "Run: tailscale up", icon = "!" },
        }
    end

    local ok, data = pcall(lark.json.decode, raw)
    if not ok or type(data) ~= "table" then
        return nil, { { label = "Failed to parse tailscale status", icon = "!" } }
    end
    return data, nil
end

-- Map tailscale online/active state to an icon.
local function status_icon(peer)
    if peer.Online then
        if peer.Active then return "●" end  -- active connection
        return "○"  -- online but idle
    end
    return "✗"  -- offline
end

-- Format a "last seen" relative time.
local function last_seen(peer)
    if peer.Online then return "online" end
    if not peer.LastSeen then return "unknown" end
    local stamp = peer.LastSeen:match("^(%d%d%d%d%-%d%d%-%d%d)")
    if stamp then return "last: " .. stamp end
    return "offline"
end
