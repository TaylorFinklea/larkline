-- Caffeinate: Status — current caffeinate state with stop action.
-- Requires spotlight-caffeinate-cli in PATH.

lark.register({
    on_run = function()
        local raw = lark.exec("spotlight-caffeinate-cli", { "status" })

        if not raw or raw == "" then
            return {
                title = "Caffeinate",
                status = "n/a",
                items = { {
                    label  = "spotlight-caffeinate-cli not found",
                    detail = "Install Spotlight Caffeinate",
                    icon   = "⚠",
                } },
            }
        end

        -- Try JSON first (newer CLI versions).
        local ok, status = pcall(lark.json.decode, raw)
        if ok and type(status) == "table" then
            local items = {}
            local is_active = status.active or status.is_active or false
            local chip

            if is_active then
                local remaining = status.remaining_minutes or status.remaining or "?"
                chip = tostring(remaining) .. "m"
                items[#items + 1] = {
                    label  = "Active — " .. tostring(remaining) .. " min remaining",
                    detail = "Mac is staying awake",
                    icon   = "☕",
                }
                items[#items + 1] = {
                    label   = "Stop",
                    icon    = "⏹",
                    actions = {
                        { label = "Stop caffeinate", kind = "shell", args = { "spotlight-caffeinate-cli", "stop" } },
                    },
                }
            else
                chip = "off"
                items[#items + 1] = {
                    label  = "Inactive",
                    detail = "Mac will sleep normally — use Start to activate",
                    icon   = "💤",
                }
            end
            return { title = "Caffeinate", status = chip, items = items }
        end

        -- Fallback: parse key-value text format (e.g. "State: idle\nRemaining: 0s\n...").
        local items = {}
        local state_value = "unknown"
        local remaining_value = nil

        for line in raw:gmatch("[^\r\n]+") do
            local key, value = line:match("^([^:]+):%s*(.+)$")
            if key and value then
                key = key:match("^%s*(.-)%s*$")
                value = value:match("^%s*(.-)%s*$")
                if key == "State" then state_value = value end
                if key == "Remaining" then remaining_value = value end

                local icon = "📋"
                if key == "State" then
                    icon = value == "idle" and "💤" or "☕"
                elseif key == "Remaining" then
                    icon = "⏱"
                elseif key == "PID" then
                    icon = "🔧"
                elseif key == "Mode" or key == "Preset" then
                    icon = "⚙️"
                end

                items[#items + 1] = {
                    label  = key,
                    detail = value,
                    icon   = icon,
                    copy_text = value,
                }
            end
        end

        if #items == 0 then
            items[#items + 1] = { label = raw:gsub("%s+$", ""), icon = "☕" }
        end

        if state_value ~= "idle" and state_value ~= "unknown" then
            items[#items + 1] = {
                label   = "Stop",
                icon    = "⏹",
                actions = {
                    { label = "Stop caffeinate", kind = "shell", args = { "spotlight-caffeinate-cli", "stop" } },
                },
            }
        end

        -- Chip: when active, prefer the time remaining (e.g. "23m") over the
        -- bare state word; fall back to the state if no useful remaining value.
        local active = state_value ~= "idle" and state_value ~= "unknown"
        local chip
        if active then
            if remaining_value and remaining_value ~= "" and remaining_value ~= "0s" then
                -- Drop the trailing seconds ("28m 16s" -> "28m"); the chip only
                -- refreshes every status_refresh_secs, so second-precision is
                -- misleading. Keep a bare "45s" when that's all there is.
                local compact = (remaining_value:gsub("%s+%d+s$", ""))
                chip = (compact ~= "" and compact) or remaining_value
            else
                chip = state_value
            end
        else
            chip = "off"
        end
        return { title = "Caffeinate — " .. state_value, status = chip, items = items }
    end,
})
