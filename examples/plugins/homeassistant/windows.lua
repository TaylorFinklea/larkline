-- Windows — binary_sensor entities with device_class "window".

local function get_config()
    local raw_url = lark.store.get("ha_url")
    local url = (type(raw_url) == "string" and raw_url ~= "") and raw_url or nil
    if url and url:sub(1, 1) == '"' then url = url:sub(2, -2) end
    local token = lark.env("HA_TOKEN")
    if not url or url == "" then
        return nil, nil, { title = "Windows", items = {
            { label = "HA URL not configured — open Settings (S)", icon = "!" },
        }}
    end
    if not token or token == "" then
        return nil, nil, { title = "Windows", items = {
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

local function curl_service(url, token, service, body)
    return {
        "curl", "-s", "-X", "POST",
        url .. "/api/services/" .. service,
        "-H", "Authorization: Bearer " .. token,
        "-H", "Content-Type: application/json",
        "-d", body,
    }
end

lark.register({
    on_run = function()
        local url, token, err = get_config()
        if err then return err end

        local resp = lark.http.get(url .. "/api/states", { headers = ha_headers(token), timeout = 8 })
        if not resp or resp.status ~= 200 then
            local code = resp and resp.status or "no response"
            return { title = "Windows", items = { { label = "HA API error: " .. tostring(code), icon = "!" } } }
        end
        local ok, states = pcall(lark.json.decode, resp.body)
        if not ok or not states then
            return { title = "Windows", items = { { label = "Invalid JSON from HA", icon = "!" } } }
        end

        local items = {}
        for _, entity in ipairs(states) do
            local eid = entity.entity_id
            if type(eid) ~= "string" then goto next_label end
            if not eid:match("^binary_sensor%.") then goto next_label end

            local attrs = (type(entity.attributes) == "table") and entity.attributes or {}
            if attrs.device_class ~= "window" then goto next_label end

            local name = friendly_name(entity)
            local state = tostring(entity.state or "unknown")
            local label_text = state == "on" and "Open" or "Closed"
            local icon = state == "on" and "🪟" or "🔒"

            items[#items + 1] = {
                label = name,
                detail = label_text .. "  " .. eid,
                icon = icon,
                copy_text = eid,
                actions = {
                    { label = "Copy entity ID", kind = "clipboard", args = { eid } },
                    { label = "Open history in browser", kind = "shell",
                      args = { "open", url .. "/history?entity_id=" .. eid },
                       },
                },
            }
            ::next_label::
        end

        table.sort(items, function(a, b) return a.label < b.label end)

        if #items == 0 then
            return { title = "Windows", items = { { label = "No window sensors found", icon = "📭" } } }
        end
        return { title = "Windows — " .. #items, items = items }
    end,
})
