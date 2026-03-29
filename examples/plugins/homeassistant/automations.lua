-- Automations — list, trigger, enable, or disable HA automations.

local function get_config()
    local raw_url = lark.store.get("ha_url")
    local url = (type(raw_url) == "string" and raw_url ~= "") and raw_url or nil
    if url and url:sub(1, 1) == '"' then url = url:sub(2, -2) end
    local token = lark.env("HA_TOKEN")
    if not url or url == "" then
        return nil, nil, { title = "Automations", items = {
            { label = "HA URL not configured — open Settings (S)", icon = "!" },
        }}
    end
    if not token or token == "" then
        return nil, nil, { title = "Automations", items = {
            { label = "HA_TOKEN not set — lark secret set HA_TOKEN", icon = "!" },
        }}
    end
    return url:gsub("/$", ""), token, nil
end

local function ha_headers(token)
    return { Authorization = "Bearer " .. token, ["Content-Type"] = "application/json" }
end

local function friendly_name(entity)
    if entity.attributes and type(entity.attributes) == "table" and entity.attributes.friendly_name then
        return tostring(entity.attributes.friendly_name)
    end
    return tostring(entity.entity_id or "unknown")
end

lark.register({
    on_run = function()
        local url, token, err = get_config()
        if err then return err end

        local resp = lark.http.get(url .. "/api/states", { headers = ha_headers(token), timeout = 8 })
        if not resp or resp.status ~= 200 then
            local code = resp and resp.status or "no response"
            return { title = "Automations", items = { { label = "HA API error: " .. tostring(code), icon = "!" } } }
        end
        local ok, states = pcall(lark.json.decode, resp.body)
        if not ok or not states then
            return { title = "Automations", items = { { label = "Invalid JSON from HA", icon = "!" } } }
        end

        local items = {}
        for _, entity in ipairs(states) do
            local eid = entity.entity_id
            if type(eid) ~= "string" then goto next_auto end
            if not eid:match("^automation%.") then goto next_auto end

            local name = friendly_name(entity)
            local state = tostring(entity.state or "unknown")
            local icon = state == "on" and "⚙️" or "⏸️"
            local body = lark.json.encode({ entity_id = eid })

            local actions = {
                {
                    label = "Trigger",
                    kind = "shell",
                    args = {
                        "curl", "-s", "-X", "POST",
                        url .. "/api/services/automation/trigger",
                        "-H", "Authorization: Bearer " .. token,
                        "-H", "Content-Type: application/json",
                        "-d", body,
                    },
                    confirm = true,
                },
            }

            local toggle_service = state == "on" and "turn_off" or "turn_on"
            local toggle_label = state == "on" and "Disable" or "Enable"
            actions[#actions + 1] = {
                label = toggle_label,
                kind = "shell",
                args = {
                    "curl", "-s", "-X", "POST",
                    url .. "/api/services/automation/" .. toggle_service,
                    "-H", "Authorization: Bearer " .. token,
                    "-H", "Content-Type: application/json",
                    "-d", body,
                },
                confirm = true,
            }

            actions[#actions + 1] = { label = "Copy entity ID", kind = "clipboard", args = { eid } }

            items[#items + 1] = {
                label = name,
                detail = state .. "  " .. eid,
                icon = icon,
                actions = actions,
            }
            ::next_auto::
        end

        table.sort(items, function(a, b) return a.label < b.label end)

        if #items == 0 then
            return { title = "Automations", items = { { label = "No automations found", icon = "📭" } } }
        end
        return { title = "Automations — " .. #items, items = items }
    end,
})
