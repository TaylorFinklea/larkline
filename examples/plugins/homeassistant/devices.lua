-- Devices — list all entities with current state.

local function get_config()
    local raw_url = lark.store.get("ha_url")
    local url = (type(raw_url) == "string" and raw_url ~= "") and raw_url or nil
    if url and url:sub(1, 1) == '"' then url = url:sub(2, -2) end
    local token = lark.env("HA_TOKEN")
    if not url or url == "" then
        return nil, nil, { title = "Home Assistant", items = {
            { label = "HA URL not configured", icon = "!" },
            { label = "Open Settings (press S) to set your HA URL", icon = "🔧" },
        }}
    end
    if not token or token == "" then
        return nil, nil, { title = "Home Assistant", items = {
            { label = "HA_TOKEN not set", icon = "!" },
            { label = "Set via: lark secret set HA_TOKEN", detail = "Long-lived access token from HA profile", icon = "🔑" },
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
    elseif d == "binary_sensor" then return state == "on" and "🟢" or "⚪"
    elseif d == "sensor" then return "📊"
    elseif d == "climate" then return "🌡️"
    elseif d == "media_player" then return "🎵"
    elseif d == "cover" then return state == "open" and "🪟" or "🔒"
    elseif d == "lock" then return state == "locked" and "🔒" or "🔓"
    elseif d == "fan" then return "🌀"
    elseif d == "camera" then return "📷"
    else return "📦" end
end

lark.register({
    on_run = function()
        local url, token, err = get_config()
        if err then return err end

        local resp = lark.http.get(url .. "/api/states", { headers = ha_headers(token), timeout = 8 })
        if not resp or resp == "" then
            return { title = "Home Assistant", items = { { label = "Failed to fetch states", icon = "!" } } }
        end
        local states = lark.json.decode(resp)
        if not states then
            return { title = "Home Assistant", items = { { label = "Invalid response from HA", icon = "!" } } }
        end

        table.sort(states, function(a, b)
            local da = a.entity_id:match("^([^%.]+)%.") or ""
            local db = b.entity_id:match("^([^%.]+)%.") or ""
            if da ~= db then return da < db end
            return friendly_name(a) < friendly_name(b)
        end)

        local items = {}
        local show = { light = true, switch = true, binary_sensor = true, sensor = true,
                       climate = true, media_player = true, cover = true, lock = true, fan = true, camera = true }

        for _, entity in ipairs(states) do
            local domain = entity.entity_id:match("^([^%.]+)%.")
            if show[domain] then
                local name = friendly_name(entity)
                local state = entity.state or "unknown"
                local actions = {
                    { label = "Copy entity ID", kind = "clipboard", args = { entity.entity_id } },
                }
                if domain == "light" or domain == "switch" or domain == "fan" or domain == "cover" or domain == "lock" then
                    table.insert(actions, 1, {
                        label = "Toggle",
                        kind = "shell",
                        args = {
                            "curl", "-s", "-X", "POST",
                            url .. "/api/services/" .. domain .. "/toggle",
                            "-H", "Authorization: Bearer " .. token,
                            "-H", "Content-Type: application/json",
                            "-d", lark.json.encode({ entity_id = entity.entity_id }),
                        },
                        confirm = true,
                    })
                end
                items[#items + 1] = {
                    label = name,
                    detail = state .. "  " .. entity.entity_id,
                    icon = icon_for(entity.entity_id, state),
                    copy_text = entity.entity_id,
                    actions = actions,
                }
            end
        end

        if #items == 0 then
            return { title = "Home Assistant", items = { { label = "No entities found", icon = "📭" } } }
        end
        return { title = "Home Assistant — " .. #items .. " devices", items = items }
    end,
})
