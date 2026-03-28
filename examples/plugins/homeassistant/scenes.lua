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
    if entity.attributes and entity.attributes.friendly_name then return entity.attributes.friendly_name end
    return entity.entity_id
end

lark.register({
    on_run = function()
        local url, token, err = get_config()
        if err then return err end

        local resp = lark.http.get(url .. "/api/states", { headers = ha_headers(token), timeout = 8 })
        if not resp or resp == "" then
            return { title = "Scenes", items = { { label = "Failed to fetch states", icon = "!" } } }
        end
        local states = lark.json.decode(resp)
        if not states then
            return { title = "Scenes", items = { { label = "Invalid response", icon = "!" } } }
        end

        local items = {}
        for _, entity in ipairs(states) do
            if entity.entity_id:match("^scene%.") then
                local name = friendly_name(entity)
                local body = lark.json.encode({ entity_id = entity.entity_id })
                items[#items + 1] = {
                    label = name,
                    detail = entity.entity_id,
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
                            confirm = true,
                        },
                        { label = "Copy entity ID", kind = "clipboard", args = { entity.entity_id } },
                    },
                }
            end
        end

        table.sort(items, function(a, b) return a.label < b.label end)

        if #items == 0 then
            return { title = "Scenes", items = { { label = "No scenes found", icon = "📭" } } }
        end
        return { title = "Scenes — " .. #items .. " scenes", items = items }
    end,
})
