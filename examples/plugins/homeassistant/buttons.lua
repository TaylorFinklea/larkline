-- Buttons — pressable button entities.

-- SHARED: error_item — canonical copy in examples/plugins/_shared/errors.lua.
local function error_item(opts)
    return {
        label = opts.label,
        detail = opts.detail,
        icon = opts.icon or "!",
        retry_action = opts.retry_action,
        help_url = opts.help_url,
    }
end

-- SHARED: get_config template (canonical copy in helpers.lua; only the title literal in error items diverges per file)
local function get_config()
    local raw_url = lark.store.get("ha_url")
    local url = (type(raw_url) == "string" and raw_url ~= "") and raw_url or nil
    if url and url:sub(1, 1) == '"' then url = url:sub(2, -2) end
    local token = lark.env("HA_TOKEN")
    if not url or url == "" then
        return nil, nil, { title = "Buttons", items = { error_item({
            label = "HA_URL or HA_TOKEN not set",
            detail = "Add them to ~/.config/larkline/.env",
            help_url = "https://www.home-assistant.io/docs/authentication/",
        }) } }
    end
    if not token or token == "" then
        return nil, nil, { title = "Buttons", items = { error_item({
            label = "HA_URL or HA_TOKEN not set",
            detail = "Add them to ~/.config/larkline/.env",
            help_url = "https://www.home-assistant.io/docs/authentication/",
        }) } }
    end
    return url:gsub("/$", ""), token, nil
end

-- SHARED: ha_headers (canonical copy in helpers.lua)
local function ha_headers(token)
    return { Authorization = "Bearer " .. token, ["Content-Type"] = "application/json" }
end

-- SHARED: friendly_name (canonical copy in helpers.lua)
local function friendly_name(entity)
    if entity.attributes and type(entity.attributes) == "table" and entity.attributes.friendly_name then
        return tostring(entity.attributes.friendly_name)
    end
    return tostring(entity.entity_id or "unknown")
end

-- SHARED: curl_service (canonical copy in helpers.lua)
local function curl_service(url, token, service, body)
    return {
        "curl", "-s", "-X", "POST",
        url .. "/api/services/" .. service,
        "-H", "Authorization: Bearer " .. token,
        "-H", "Content-Type: application/json",
        "-d", body,
    }
end

-- SHARED: ha_http_error (canonical copy in helpers.lua)
local function ha_http_error(resp, url)
    if not resp then
        return error_item({
            label = "Cannot reach Home Assistant",
            detail = url,
            help_url = "https://www.home-assistant.io/docs/configuration/remote/",
        })
    end
    local status = resp.status
    if status == 401 or status == 403 then
        return error_item({
            label = "Home Assistant auth failed",
            detail = "HA_TOKEN may be expired",
            help_url = "https://www.home-assistant.io/docs/authentication/",
        })
    end
    if status == 404 then
        return error_item({
            label = "HA endpoint not found",
            detail = "HTTP 404 at " .. tostring(url),
            help_url = "https://developers.home-assistant.io/docs/api/rest/",
        })
    end
    return error_item({
        label = "Home Assistant API error",
        detail = "HTTP " .. tostring(status),
        help_url = "https://developers.home-assistant.io/docs/api/rest/",
    })
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
            return { title = "Buttons", items = { ha_http_error(resp, url .. "/api/states") } }
        end
        local ok, states = pcall(lark.json.decode, resp.body)
        if not ok or not states then
            return { title = "Buttons", items = { error_item({
                label = "Invalid JSON from Home Assistant",
                detail = "Response body could not be parsed",
                help_url = "https://developers.home-assistant.io/docs/api/rest/",
            }) } }
        end

        local items = {}
        for _, entity in ipairs(states) do
            local eid = entity.entity_id
            if type(eid) ~= "string" then goto next_label end
            if hidden_entities_set[eid] then goto next_label end
            if not eid:match("^button%.") then goto next_label end

            local name = friendly_name(entity)
            local state = tostring(entity.state or "unknown")
            if hidden_states_set[state] then goto next_label end

            items[#items + 1] = {
                label = name,
                detail = state .. "  " .. eid,
                icon = "🔘",
                copy_text = eid,
                actions = {
                    { label = "Press", kind = "shell",
                      args = curl_service(url, token, "button/press", lark.json.encode({ entity_id = eid })),
                       },
                    { label = "⭐ Favorite", kind = "shell",
                      args = { "bash", lark.env("HOME") .. "/.config/larkline/plugins/homeassistant/ha-manage.sh", "favorite", eid } },
                    { label = "🚫 Hide", kind = "shell",
                      args = { "bash", lark.env("HOME") .. "/.config/larkline/plugins/homeassistant/ha-manage.sh", "hide", eid } },
                    { label = "Copy entity ID", kind = "clipboard", args = { eid } },
                },
            }
            ::next_label::
        end

        table.sort(items, function(a, b) return a.label < b.label end)

        if #items == 0 then
            return { title = "Buttons", items = { { label = "No buttons found", icon = "📭" } } }
        end
        return { title = "Buttons — " .. #items, items = items }
    end,
})
