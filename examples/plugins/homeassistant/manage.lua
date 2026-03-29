-- Manage — add/remove favorites and hidden entities.

local function get_config()
    local raw_url = lark.store.get("ha_url")
    local url = (type(raw_url) == "string" and raw_url ~= "") and raw_url or nil
    if url and url:sub(1, 1) == '"' then url = url:sub(2, -2) end
    local token = lark.env("HA_TOKEN")
    if not url or url == "" then
        return nil, nil, { title = "Manage", items = {
            { label = "HA URL not configured — open Settings (S)", icon = "!" },
        }}
    end
    if not token or token == "" then
        return nil, nil, { title = "Manage", items = {
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

local function load_list(key)
    local raw = lark.store.get(key)
    if type(raw) == "string" and raw ~= "" then
        local ok, list = pcall(lark.json.decode, raw)
        if ok and type(list) == "table" then return list end
    end
    return {}
end

local function save_list(key, list)
    lark.store.set(key, lark.json.encode(list))
end

local function toggle_in_list(key, entity_id)
    local list = load_list(key)
    for i, eid in ipairs(list) do
        if eid == entity_id then
            table.remove(list, i)
            save_list(key, list)
            return false -- removed
        end
    end
    list[#list + 1] = entity_id
    save_list(key, list)
    return true -- added
end

lark.register({
    on_run = function()
        local url, token, err = get_config()
        if err then return err end

        -- Handle form submission (add favorite or hide entity).
        if lark.form_values then
            local action = lark.form_values.action or ""
            local entity_id = lark.form_values.entity_id or ""
            if entity_id == "" then
                return { title = "Manage", items = { { label = "No entity ID provided", icon = "!" } } }
            end
            if action == "favorite" then
                local added = toggle_in_list("favorites", entity_id)
                local verb = added and "Added to" or "Removed from"
                return { title = "Manage", items = {
                    { label = verb .. " favorites: " .. entity_id, icon = "⭐" },
                }}
            elseif action == "hide" then
                local added = toggle_in_list("hidden_entities", entity_id)
                local verb = added and "Hidden" or "Unhidden"
                return { title = "Manage", items = {
                    { label = verb .. ": " .. entity_id, icon = "🚫" },
                }}
            end
        end

        -- Show current favorites and hidden entities, plus all entities for management.
        local favorites = load_list("favorites")
        local hidden = load_list("hidden_entities")

        local resp = lark.http.get(url .. "/api/states", { headers = ha_headers(token), timeout = 8 })
        if not resp or resp.status ~= 200 then
            local code = resp and resp.status or "no response"
            return { title = "Manage", items = { { label = "HA API error: " .. tostring(code), icon = "!" } } }
        end
        local ok, states = pcall(lark.json.decode, resp.body)
        if not ok or not states then
            return { title = "Manage", items = { { label = "Invalid JSON from HA", icon = "!" } } }
        end

        -- Build lookup sets.
        local fav_set = {}
        for _, eid in ipairs(favorites) do fav_set[eid] = true end
        local hidden_set = {}
        for _, eid in ipairs(hidden) do hidden_set[eid] = true end

        local items = {}

        -- Section: current hidden entities (at top for easy unhiding).
        if #hidden > 0 then
            items[#items + 1] = {
                label = "── Hidden Entities (" .. #hidden .. ") ──",
                detail = "Select to unhide",
                icon = "🚫",
            }
            for _, eid in ipairs(hidden) do
                -- Find the entity in states for its friendly name.
                local name = eid
                for _, entity in ipairs(states) do
                    if entity.entity_id == eid then
                        name = friendly_name(entity)
                        break
                    end
                end
                items[#items + 1] = {
                    label = "🚫 " .. name,
                    detail = eid .. " (select to unhide)",
                    icon = "👁",
                    actions = {
                        { label = "Unhide " .. name, kind = "shell",
                          args = { "lark", "invoke", "Home Assistant:manage:unhide:" .. eid } },
                    },
                }
            end
        end

        -- Section: all entities for adding to favorites or hiding.
        items[#items + 1] = {
            label = "── All Entities ──",
            detail = "⭐ = favorited, Select to manage",
            icon = "📋",
        }

        for _, entity in ipairs(states) do
            local eid = entity.entity_id
            if type(eid) ~= "string" then goto next_manage end

            local name = friendly_name(entity)
            local state = tostring(entity.state or "unknown")
            local is_fav = fav_set[eid]
            local is_hidden = hidden_set[eid]
            local prefix = ""
            if is_fav then prefix = "⭐ " end
            if is_hidden then prefix = "🚫 " end

            local actions = {}
            if is_fav then
                actions[#actions + 1] = { label = "Remove from Favorites", kind = "clipboard", args = { "unfav:" .. eid } }
            else
                actions[#actions + 1] = { label = "⭐ Add to Favorites", kind = "clipboard", args = { "fav:" .. eid } }
            end
            if is_hidden then
                actions[#actions + 1] = { label = "Unhide", kind = "clipboard", args = { "unhide:" .. eid } }
            else
                actions[#actions + 1] = { label = "🚫 Hide Entity", kind = "clipboard", args = { "hide:" .. eid } }
            end
            actions[#actions + 1] = { label = "Copy entity ID", kind = "clipboard", args = { eid } }

            items[#items + 1] = {
                label = prefix .. name,
                detail = state .. "  " .. eid,
                icon = is_fav and "⭐" or "📦",
                copy_text = eid,
                actions = actions,
            }
            ::next_manage::
        end

        return {
            title = "Manage — " .. #favorites .. " fav, " .. #hidden .. " hidden",
            items = items,
            form = {
                fields = {
                    {
                        id = "action",
                        label = "Action",
                        type = { kind = "select", options = { "favorite", "hide", "unfavorite", "unhide" } },
                    },
                    {
                        id = "entity_id",
                        label = "Entity ID",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "e.g. light.kitchen_light",
                    },
                },
                submit_label = "Apply",
            },
        }
    end,
})
