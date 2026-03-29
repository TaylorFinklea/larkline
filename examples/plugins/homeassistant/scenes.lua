-- Scenes — list and activate Home Assistant scenes.

local function get_config()
    local raw_url = lark.store.get("ha_url")
    local url = (type(raw_url) == "string" and raw_url ~= "") and raw_url or nil
    if url and url:sub(1, 1) == '"' then url = url:sub(2, -2) end
    local token = lark.env("HA_TOKEN")
    if not url or url == "" then
        return nil, nil, { title = "Scenes", items = {
            { label = "HA URL not configured — open Settings (S)", icon = "!" },
        }}
    end
    if not token or token == "" then
        return nil, nil, { title = "Scenes", items = {
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

        -- Load filters.
        local hidden_states_raw = lark.store.get("hidden_states") or ""
        if type(hidden_states_raw) == "string" and hidden_states_raw:sub(1,1) == '"' then
            hidden_states_raw = hidden_states_raw:sub(2, -2)
        end
        local hidden_states_set = {}
        for s in (tostring(hidden_states_raw)):gmatch("[^,]+") do
            hidden_states_set[s:match("^%s*(.-)%s*$")] = true
        end
        local hidden_entities_raw = lark.store.get("hidden_entities")
        local hidden_entities_set = {}
        if type(hidden_entities_raw) == "string" and hidden_entities_raw ~= "" then
            local hok, hlist = pcall(lark.json.decode, hidden_entities_raw)
            if hok and type(hlist) == "table" then
                for _, eid in ipairs(hlist) do hidden_entities_set[eid] = true end
            end
        end

        local resp = lark.http.get(url .. "/api/states", { headers = ha_headers(token), timeout = 8 })
        if not resp or resp.status ~= 200 then
            local code = resp and resp.status or "no response"
            return { title = "Scenes", items = { { label = "HA API error: " .. tostring(code), icon = "!" } } }
        end
        local ok, states = pcall(lark.json.decode, resp.body)
        if not ok or not states then
            return { title = "Scenes", items = { { label = "Invalid JSON from HA", icon = "!" } } }
        end

        local items = {}
        for _, entity in ipairs(states) do
            local eid = entity.entity_id
            if type(eid) ~= "string" then goto next_scene end
            if hidden_entities_set[eid] then goto next_scene end
            if not eid:match("^scene%.") then goto next_scene end

            local state = tostring(entity.state or "unknown")
            if hidden_states_set[state] then goto next_scene end

            local name = friendly_name(entity)
            local body = lark.json.encode({ entity_id = eid })
            items[#items + 1] = {
                label = name,
                detail = eid,
                icon = "🎬",
                actions = {
                    {
                        label = "Activate " .. name,
                        kind = "shell",
                        args = {
                            "curl", "-s", "-X", "POST",
                            url .. "/api/services/scene/turn_on",
                            "-H", "Authorization: Bearer " .. token,
                            "-H", "Content-Type: application/json",
                            "-d", body,
                        },
                        
                    },
                    { label = "⭐ Favorite", kind = "shell",
                      args = { "bash", os.getenv("HOME") .. "/.config/larkline/plugins/homeassistant/ha-manage.sh", "favorite", eid } },
                    { label = "🚫 Hide", kind = "shell",
                      args = { "bash", os.getenv("HOME") .. "/.config/larkline/plugins/homeassistant/ha-manage.sh", "hide", eid } },
                    { label = "Copy entity ID", kind = "clipboard", args = { eid } },
                },
            }
            ::next_scene::
        end

        table.sort(items, function(a, b) return a.label < b.label end)

        if #items == 0 then
            return { title = "Scenes", items = { { label = "No scenes found", icon = "📭" } } }
        end
        return { title = "Scenes — " .. #items .. " scenes", items = items }
    end,
})
