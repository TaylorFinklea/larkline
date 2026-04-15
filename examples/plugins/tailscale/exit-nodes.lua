-- Tailscale: Exit Nodes — list available exit nodes and set current.
-- SHARED: fetch_status() from lib.lua

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

lark.register({
    on_run = function()
        local data, err = fetch_status()
        if not data then return { title = "Exit Nodes", items = err } end

        local current_exit = nil
        if data.Peer then
            for _, peer in pairs(data.Peer) do
                if peer.ExitNode then
                    current_exit = peer.HostName
                    break
                end
            end
        end

        local items = {}

        -- Disable option.
        items[#items + 1] = {
            label = "None (direct)",
            detail = current_exit == nil and "active" or "",
            icon = current_exit == nil and "★" or "○",
            actions = {
                {
                    label = "Disable exit node",
                    kind = "shell",
                    args = { "tailscale", "set", "--exit-node=" },
                    confirm = true,
                },
            },
        }

        -- Available exit nodes (peers with ExitNodeOption = true).
        local exit_nodes = {}
        if data.Peer then
            for _, peer in pairs(data.Peer) do
                if peer.ExitNodeOption then
                    exit_nodes[#exit_nodes + 1] = peer
                end
            end
        end

        table.sort(exit_nodes, function(a, b)
            return (a.HostName or "") < (b.HostName or "")
        end)

        for _, peer in ipairs(exit_nodes) do
            local name = peer.HostName or "?"
            local ips = peer.TailscaleIPs or {}
            local ip = ips[1] or "?"
            local is_current = name == current_exit

            local detail_parts = { ip }
            if peer.Online then detail_parts[#detail_parts + 1] = "online"
            else detail_parts[#detail_parts + 1] = "offline" end
            if is_current then detail_parts[#detail_parts + 1] = "active" end

            items[#items + 1] = {
                label = name,
                detail = table.concat(detail_parts, " · "),
                icon = is_current and "★" or (peer.Online and "○" or "✗"),
                copy_text = name,
                actions = {
                    {
                        label = "Use as exit node",
                        kind = "shell",
                        args = { "tailscale", "set", "--exit-node=" .. name },
                        confirm = true,
                    },
                    { label = "Copy hostname", kind = "clipboard", args = { name } },
                },
            }
        end

        if #exit_nodes == 0 then
            items[#items + 1] = { label = "No exit nodes advertised in tailnet", icon = "📭" }
        end

        return { title = "Exit Nodes — " .. #exit_nodes .. " available", items = items }
    end,
})
