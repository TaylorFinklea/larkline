-- Toggle — toggle a light, switch, fan, cover, or lock.

local function get_config()
    local raw_url = lark.store.get("ha_url")
    local url = (type(raw_url) == "string" and raw_url ~= "") and raw_url or nil
    if url and url:sub(1, 1) == '"' then url = url:sub(2, -2) end
    local token = lark.env("HA_TOKEN")
    if not url or url == "" then
        return nil, nil, { title = "Toggle", items = {
            { label = "HA URL not configured — open Settings (S)", icon = "!" },
        }}
    end
    if not token or token == "" then
        return nil, nil, { title = "Toggle", items = {
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

local function icon_for(entity_id, state)
    local d = entity_id:match("^([^%.]+)%.")
    if d == "light" then return state == "on" and "💡" or "🌑"
    elseif d == "switch" then return state == "on" and "🔌" or "⭕"
    elseif d == "fan" then return "🌀"
    elseif d == "cover" then return state == "open" and "🪟" or "🔒"
    elseif d == "lock" then return state == "locked" and "🔒" or "🔓"
    else return "📦" end
end

lark.register({
    on_run = function()
        local url, token, err = get_config()
        if err then return err end

        local resp = lark.http.get(url .. "/api/states", { headers = ha_headers(token), timeout = 8 })
        if not resp or resp == "" then
            return { title = "Toggle", items = { { label = "Failed to fetch states", icon = "!" } } }
        end
        local states = lark.json.decode(resp)
        if not states then
            return { title = "Toggle", items = { { label = "Invalid response", icon = "!" } } }
        end

        local toggleable = { light = true, switch = true, fan = true, cover = true, lock = true }
        local items = {}

        for _, entity in ipairs(states) do
            local domain = entity.entity_id:match("^([^%.]+)%.")
            if toggleable[domain] then
                local name = friendly_name(entity)
                local state = entity.state or "unknown"
                local body = lark.json.encode({ entity_id = entity.entity_id })
                items[#items + 1] = {
                    label = name,
                    detail = state .. "  " .. entity.entity_id,
                    icon = icon_for(entity.entity_id, state),
                    actions = {
                        {
                            label = "Toggle " .. name,
                            kind = "shell",
                            args = {
                                "curl", "-s", "-X", "POST",
                                url .. "/api/services/" .. domain .. "/toggle",
                                "-H", "Authorization: Bearer " .. token,
                                "-H", "Content-Type: application/json",
                                "-d", body,
                            },
                            confirm = true,
                        },
                        { label = "Copy entity ID", kind = "clipboard", args = { entity.entity_id } },
                    },
                }
            end
        end

        table.sort(items, function(a, b) return a.label < b.label end)

        if #items == 0 then
            return { title = "Toggle", items = { { label = "No toggleable devices found", icon = "📭" } } }
        end
        return { title = "Toggle — " .. #items .. " devices", items = items }
    end,
})
