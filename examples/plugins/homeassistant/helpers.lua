-- Helpers — input_boolean, input_number, input_select, input_text, input_button, input_datetime.

local function get_config()
    local raw_url = lark.store.get("ha_url")
    local url = (type(raw_url) == "string" and raw_url ~= "") and raw_url or nil
    if url and url:sub(1, 1) == '"' then url = url:sub(2, -2) end
    local token = lark.env("HA_TOKEN")
    if not url or url == "" then
        return nil, nil, { title = "Helpers", items = {
            { label = "HA URL not configured — open Settings (S)", icon = "!" },
        }}
    end
    if not token or token == "" then
        return nil, nil, { title = "Helpers", items = {
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
            return { title = "Helpers", items = { { label = "HA API error: " .. tostring(code), icon = "!" } } }
        end
        local ok, states = pcall(lark.json.decode, resp.body)
        if not ok or not states then
            return { title = "Helpers", items = { { label = "Invalid JSON from HA", icon = "!" } } }
        end

        local helper_domains = {
            input_boolean = true,
            input_number = true,
            input_select = true,
            input_text = true,
            input_button = true,
            input_datetime = true,
        }

        local items = {}
        for _, entity in ipairs(states) do
            local eid = entity.entity_id
            if type(eid) ~= "string" then goto next_label end
            if hidden_entities_set[eid] then goto next_label end
            local domain = eid:match("^([^%.]+)%.")
            if not domain or not helper_domains[domain] then goto next_label end

            local name = friendly_name(entity)
            local state = tostring(entity.state or "unknown")
            if hidden_states_set[state] then goto next_label end
            local attrs = (type(entity.attributes) == "table") and entity.attributes or {}
            local detail = state
            local actions = {}

            if domain == "input_boolean" then
                detail = state == "on" and "On" or "Off"
                actions[#actions + 1] = {
                    label = "Toggle",
                    kind = "shell",
                    args = curl_service(url, token, "input_boolean/toggle",
                        lark.json.encode({ entity_id = eid })),
                    
                }
            elseif domain == "input_number" then
                local unit = type(attrs.unit_of_measurement) == "string" and attrs.unit_of_measurement or ""
                if unit ~= "" then
                    detail = state .. " " .. unit
                end
            elseif domain == "input_select" then
                local options = attrs.options
                if type(options) == "table" and #options > 0 then
                    detail = state .. " (" .. #options .. " options)"
                end
            elseif domain == "input_text" then
                -- state is the current text value
                detail = state
            elseif domain == "input_button" then
                actions[#actions + 1] = {
                    label = "Press",
                    kind = "shell",
                    args = curl_service(url, token, "input_button/press",
                        lark.json.encode({ entity_id = eid })),
                    
                }
            elseif domain == "input_datetime" then
                detail = state
            end

            actions[#actions + 1] = { label = "Copy entity ID", kind = "clipboard", args = { eid } }

            items[#items + 1] = {
                label = name,
                detail = detail .. "  " .. eid,
                icon = "⚙️",
                copy_text = eid,
                actions = actions,
            }
            ::next_label::
        end

        table.sort(items, function(a, b) return a.label < b.label end)

        if #items == 0 then
            return { title = "Helpers", items = { { label = "No helper entities found", icon = "📭" } } }
        end
        return { title = "Helpers — " .. #items, items = items }
    end,
})
