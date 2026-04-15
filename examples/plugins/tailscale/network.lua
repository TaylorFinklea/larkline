-- Tailscale: Network — tailnet overview with self, DNS, MagicDNS info.
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
        if not data then return { title = "Tailnet Network", items = err } end

        local items = {}

        -- Backend state.
        local state = data.BackendState or "Unknown"
        local state_icon = state == "Running" and "●" or (state == "Stopped" and "○" or "?")
        items[#items + 1] = {
            label = "Backend: " .. state,
            detail = "Tailscale daemon state",
            icon = state_icon,
        }

        -- Self info.
        if data.Self then
            local self = data.Self
            local ips = self.TailscaleIPs or {}
            items[#items + 1] = {
                label = self.HostName or "self",
                detail = "Hostname",
                icon = "🖥",
                copy_text = self.HostName or "",
                actions = {
                    { label = "Copy hostname", kind = "clipboard", args = { self.HostName or "" } },
                },
            }
            for i, ip in ipairs(ips) do
                items[#items + 1] = {
                    label = ip,
                    detail = i == 1 and "IPv4" or "IPv6",
                    icon = "📡",
                    copy_text = ip,
                    actions = { { label = "Copy IP", kind = "clipboard", args = { ip } } },
                }
            end
            if self.DNSName then
                items[#items + 1] = {
                    label = self.DNSName:gsub("%.$", ""),
                    detail = "MagicDNS name",
                    icon = "🌐",
                    copy_text = self.DNSName:gsub("%.$", ""),
                    actions = { { label = "Copy DNS name", kind = "clipboard", args = { self.DNSName:gsub("%.$", "") } } },
                }
            end
        end

        -- MagicDNS status.
        if data.MagicDNSSuffix then
            items[#items + 1] = {
                label = "." .. data.MagicDNSSuffix,
                detail = "MagicDNS suffix",
                icon = "🔧",
                copy_text = data.MagicDNSSuffix,
            }
        end

        -- Peer count.
        local peer_count = 0
        local online_count = 0
        if data.Peer then
            for _, peer in pairs(data.Peer) do
                peer_count = peer_count + 1
                if peer.Online then online_count = online_count + 1 end
            end
        end
        items[#items + 1] = {
            label = online_count .. "/" .. peer_count .. " peers online",
            detail = "Tailnet size",
            icon = "👥",
        }

        -- Tailnet name.
        if data.CurrentTailnet and data.CurrentTailnet.Name then
            items[#items + 1] = {
                label = data.CurrentTailnet.Name,
                detail = "Tailnet name",
                icon = "🏷",
                copy_text = data.CurrentTailnet.Name,
            }
        end

        return { title = "Tailnet Network", items = items }
    end,
})
