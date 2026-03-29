-- Favorites — your pinned Home Assistant entities.

local function get_config()
    local raw_url = lark.store.get("ha_url")
    local url = (type(raw_url) == "string" and raw_url ~= "") and raw_url or nil
    if url and url:sub(1, 1) == '"' then url = url:sub(2, -2) end
    local token = lark.env("HA_TOKEN")
    if not url or url == "" then
        return nil, nil, { title = "Favorites", items = {
            { label = "HA URL not configured — open Settings (S)", icon = "!" },
        }}
    end
    if not token or token == "" then
        return nil, nil, { title = "Favorites", items = {
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

local function icon_for(entity_id, state)
    local d = tostring(entity_id):match("^([^%.]+)%.")
    if d == "light" then return state == "on" and "💡" or "🌑"
    elseif d == "switch" then return state == "on" and "🔌" or "⭕"
    elseif d == "climate" then return "🌡️"
    elseif d == "media_player" then return "🎵"
    elseif d == "cover" then return state == "open" and "🪟" or "🔒"
    elseif d == "fan" then return "🌀"
    elseif d == "scene" then return "🎬"
    elseif d == "automation" then return state == "on" and "⚙️" or "⏸️"
    elseif d == "sensor" then return "📊"
    elseif d == "binary_sensor" then return state == "on" and "🟢" or "⚪"
    elseif d == "camera" then return "📷"
    elseif d == "person" then return state == "home" and "👤" or "🏃"
    elseif d == "vacuum" then return "🤖"
    else return "📦" end
end

local function curl_service(url, token, service, body)
    return {
        "curl", "-s", "-X", "POST",
        url .. "/api/services/" .. service,
        "-H", "Authorization: Bearer " .. token,
        "-H", "Content-Type: application/json",
        "-d", body,
    }
end

local function load_favorites()
    local raw = lark.store.get("favorites")
    if type(raw) == "string" and raw ~= "" then
        local ok, list = pcall(lark.json.decode, raw)
        if ok and type(list) == "table" then return list end
    end
    return {}
end

lark.register({
    on_run = function()
        local url, token, err = get_config()
        if err then return err end

        local favs = load_favorites()
        if #favs == 0 then
            return { title = "Favorites", items = {
                { label = "No favorites yet", icon = "⭐" },
                { label = "Use Manage → Add Favorite to pin entities", detail = "Or use ⭐ Favorite action on any entity", icon = "💡" },
            }}
        end

        -- Build a set for fast lookup.
        local fav_set = {}
        for _, eid in ipairs(favs) do fav_set[eid] = true end

        local resp = lark.http.get(url .. "/api/states", { headers = ha_headers(token), timeout = 8 })
        if not resp or resp.status ~= 200 then
            local code = resp and resp.status or "no response"
            return { title = "Favorites", items = { { label = "HA API error: " .. tostring(code), icon = "!" } } }
        end
        local ok, states = pcall(lark.json.decode, resp.body)
        if not ok or not states then
            return { title = "Favorites", items = { { label = "Invalid JSON from HA", icon = "!" } } }
        end

        local items = {}
        for _, entity in ipairs(states) do
            local eid = entity.entity_id
            if type(eid) ~= "string" then goto next_fav end
            if not fav_set[eid] then goto next_fav end

            local name = friendly_name(entity)
            local state = tostring(entity.state or "unknown")
            local domain = eid:match("^([^%.]+)%.")

            local actions = {}
            -- Toggleable domains get a toggle action.
            if domain == "light" or domain == "switch" or domain == "fan" or domain == "cover" or domain == "lock" then
                actions[#actions + 1] = {
                    label = "Toggle",
                    kind = "shell",
                    args = curl_service(url, token, domain .. "/toggle", lark.json.encode({ entity_id = eid })),
                }
            elseif domain == "scene" then
                actions[#actions + 1] = {
                    label = "Activate",
                    kind = "shell",
                    args = curl_service(url, token, "scene/turn_on", lark.json.encode({ entity_id = eid })),
                }
            elseif domain == "script" then
                actions[#actions + 1] = {
                    label = "Run",
                    kind = "shell",
                    args = curl_service(url, token, "script/turn_on", lark.json.encode({ entity_id = eid })),
                }
            end
            actions[#actions + 1] = { label = "Remove from Favorites", kind = "shell",
              args = { "bash", os.getenv("HOME") .. "/.config/larkline/plugins/homeassistant/ha-manage.sh", "unfavorite", eid } }
            actions[#actions + 1] = { label = "Copy entity ID", kind = "clipboard", args = { eid } }

            items[#items + 1] = {
                label = "⭐ " .. name,
                detail = state .. "  " .. eid,
                icon = icon_for(eid, state),
                copy_text = eid,
                actions = actions,
            }
            ::next_fav::
        end

        if #items == 0 then
            return { title = "Favorites", items = { { label = "Favorited entities not found in HA", icon = "📭" } } }
        end
        return { title = "Favorites — " .. #items, items = items }
    end,
})
