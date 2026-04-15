-- Tailscale: Devices — all peers in your tailnet with status and actions.
-- SHARED: fetch_status(), status_icon() from lib.lua

local function fetch_status()
    local which = lark.exec("which", { "tailscale" })
    if not which or which == "" then
        return nil, { { label = "tailscale not installed", detail = "brew install tailscale", icon = "!" } }
    end
    local raw = lark.exec("tailscale", { "status", "--json" })
    if not raw or raw == "" then
        return nil, { { label = "tailscale not running", detail = "Run: tailscale up", icon = "!" } }
    end
    local ok, data = pcall(lark.json.decode, raw)
    if not ok or type(data) ~= "table" then
        return nil, { { label = "Failed to parse tailscale status", icon = "!" } }
    end
    return data, nil
end

local function status_icon(peer)
    if peer.Online then
        if peer.Active then return "●" end
        return "○"
    end
    return "✗"
end

lark.register({
    on_run = function()
        local data, err = fetch_status()
        if not data then return { title = "Tailscale Devices", items = err } end

        local items = {}

        -- Self first.
        if data.Self then
            local self = data.Self
            local ips = self.TailscaleIPs or {}
            items[#items + 1] = {
                label = (self.HostName or "self") .. " (this device)",
                detail = (ips[1] or "?") .. " · " .. (self.OS or "?"),
                icon = "★",
                copy_text = ips[1] or "",
                actions = {
                    { label = "Copy IP", kind = "clipboard", args = { ips[1] or "" } },
                    { label = "Copy hostname", kind = "clipboard", args = { self.HostName or "" } },
                },
            }
        end

        -- Peers.
        local peers = {}
        if data.Peer then
            for _, peer in pairs(data.Peer) do
                peers[#peers + 1] = peer
            end
        end

        -- Sort: online first, then by hostname.
        table.sort(peers, function(a, b)
            if a.Online ~= b.Online then return a.Online end
            return (a.HostName or "") < (b.HostName or "")
        end)

        for _, peer in ipairs(peers) do
            local ips = peer.TailscaleIPs or {}
            local name = peer.HostName or "?"
            local ip = ips[1] or "?"
            local os_name = peer.OS or "?"

            local detail_parts = { ip, os_name }
            if not peer.Online and peer.LastSeen then
                local stamp = peer.LastSeen:match("^(%d%d%d%d%-%d%d%-%d%d)")
                if stamp then detail_parts[#detail_parts + 1] = "last: " .. stamp end
            end
            if peer.ExitNode then
                detail_parts[#detail_parts + 1] = "exit node"
            end

            local actions = {
                { label = "Copy IP", kind = "clipboard", args = { ip } },
                { label = "Copy hostname", kind = "clipboard", args = { name } },
            }
            if peer.Online then
                actions[#actions + 1] = {
                    label = "SSH to device",
                    kind = "shell",
                    args = { "open", "-a", "Terminal", "ssh://" .. name },
                }
                actions[#actions + 1] = {
                    label = "Ping",
                    kind = "shell",
                    args = { "tailscale", "ping", "--c", "3", name },
                }
            end

            items[#items + 1] = {
                label = name,
                detail = table.concat(detail_parts, " · "),
                icon = status_icon(peer),
                copy_text = ip,
                actions = actions,
            }
        end

        if #items == 0 then
            return { title = "Tailscale Devices", items = { { label = "No devices found", icon = "📭" } } }
        end

        return { title = "Tailscale Devices — " .. #items, items = items }
    end,
})
