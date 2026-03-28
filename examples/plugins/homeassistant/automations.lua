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
    if entity.attributes and entity.attributes.friendly_name then return entity.attributes.friendly_name end
    return entity.entity_id
end

lark.register({
    on_run = function()
        local url, token, err = get_config()
        if err then return err end

        local resp = lark.http.get(url .. "/api/states", { headers = ha_headers(token), timeout = 8 })
        if not resp or resp == "" then
            return { title = "Automations", items = { { label = "Failed to fetch states", icon = "!" } } }
        end
        local states = lark.json.decode(resp)
        if not states then
            return { title = "Automations", items = { { label = "Invalid response", icon = "!" } } }
        end

        local items = {}
        for _, entity in ipairs(states) do
            if entity.entity_id:match("^automation%.") then
                local name = friendly_name(entity)
                local state = entity.state or "unknown"
                local icon = state == "on" and "⚙️" or "⏸️"
                local body = lark.json.encode({ entity_id = entity.entity_id })

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

                if state == "on" then
                    actions[#actions + 1] = {
                        label = "Disable",
                        kind = "shell",
                        args = {
                            "curl", "-s", "-X", "POST",
                            url .. "/api/services/automation/turn_off",
                            "-H", "Authorization: Bearer " .. token,
                            "-H", "Content-Type: application/json",
                            "-d", body,
                        },
                        confirm = true,
                    }
                else
                    actions[#actions + 1] = {
                        label = "Enable",
                        kind = "shell",
                        args = {
                            "curl", "-s", "-X", "POST",
                            url .. "/api/services/automation/turn_on",
                            "-H", "Authorization: Bearer " .. token,
                            "-H", "Content-Type: application/json",
                            "-d", body,
                        },
                        confirm = true,
                    }
                end

                actions[#actions + 1] = { label = "Copy entity ID", kind = "clipboard", args = { entity.entity_id } }

                items[#items + 1] = {
                    label = name,
                    detail = state .. "  " .. entity.entity_id,
                    icon = icon,
                    actions = actions,
                }
            end
        end

        table.sort(items, function(a, b) return a.label < b.label end)

        if #items == 0 then
            return { title = "Automations", items = { { label = "No automations found", icon = "📭" } } }
        end
        return { title = "Automations — " .. #items, items = items }
    end,
})
