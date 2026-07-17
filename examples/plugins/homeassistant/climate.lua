-- Climate — thermostats and HVAC with temperature and mode control.

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
        return nil, nil, { title = "Climate", items = { error_item({
            label = "HA_URL or HA_TOKEN not set",
            detail = "Add them to ~/.config/larkline/.env",
            help_url = "https://www.home-assistant.io/docs/authentication/",
        }) } }
    end
    if not token or token == "" then
        return nil, nil, { title = "Climate", items = { error_item({
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
    -- Token via env, not argv: argv is visible in `ps` and in copy-as-JSON.
    -- The host injects secrets (HA_TOKEN) into shell-action children; sh
    -- expands it at run time. url/body ride as positional args ("$1"/"$2").
    local _ = token
    return {
        "sh", "-c",
        'curl -s -X POST "$1" -H "Authorization: Bearer $HA_TOKEN" -H "Content-Type: application/json" -d "$2"',
        "curl", url .. "/api/services/" .. service, body,
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
            return { title = "Climate", items = { ha_http_error(resp, url .. "/api/states") } }
        end
        local ok, states = pcall(lark.json.decode, resp.body)
        if not ok or not states then
            return { title = "Climate", items = { error_item({
                label = "Invalid JSON from Home Assistant",
                detail = "Response body could not be parsed",
                help_url = "https://developers.home-assistant.io/docs/api/rest/",
            }) } }
        end

        local items = {}
        for _, entity in ipairs(states) do
            local eid = entity.entity_id
            if type(eid) ~= "string" then goto next_climate end
            if hidden_entities_set[eid] then goto next_climate end
            if not eid:match("^climate%.") then goto next_climate end

            local name = friendly_name(entity)
            local state = tostring(entity.state or "unknown")
            if hidden_states_set[state] then goto next_climate end
            local attrs = (type(entity.attributes) == "table") and entity.attributes or {}
            local current_temp = attrs.current_temperature
            local target_temp = attrs.temperature
            local detail = state
            if type(current_temp) == "number" then
                detail = detail .. "  " .. current_temp .. "°"
            end
            if type(target_temp) == "number" then
                detail = detail .. " → " .. target_temp .. "°"
            end

            local actions = {}

            -- HVAC mode actions.
            local modes = attrs.hvac_modes
            if type(modes) == "table" then
                for _, mode in ipairs(modes) do
                    if type(mode) == "string" then
                        actions[#actions + 1] = {
                            label = "Mode: " .. mode,
                            kind = "shell",
                            args = curl_service(url, token, "climate/set_hvac_mode",
                                lark.json.encode({ entity_id = eid, hvac_mode = mode })),
                            
                        }
                    end
                end
            end

            -- Temperature presets.
            local min_t = type(attrs.min_temp) == "number" and attrs.min_temp or 60
            local max_t = type(attrs.max_temp) == "number" and attrs.max_temp or 85
            local step = type(attrs.target_temp_step) == "number" and attrs.target_temp_step or 1
            -- Common temperatures in range.
            for t = min_t, max_t, step * 5 do
                actions[#actions + 1] = {
                    label = "Set " .. t .. "°",
                    kind = "shell",
                    args = curl_service(url, token, "climate/set_temperature",
                        lark.json.encode({ entity_id = eid, temperature = t })),
                    
                }
            end

            -- Increment/decrement.
            if type(target_temp) == "number" then
                actions[#actions + 1] = {
                    label = "Temp +1°",
                    kind = "shell",
                    args = curl_service(url, token, "climate/set_temperature",
                        lark.json.encode({ entity_id = eid, temperature = target_temp + step })),
                    
                }
                actions[#actions + 1] = {
                    label = "Temp -1°",
                    kind = "shell",
                    args = curl_service(url, token, "climate/set_temperature",
                        lark.json.encode({ entity_id = eid, temperature = target_temp - step })),
                    
                }
            end

            actions[#actions + 1] = { label = "Copy entity ID", kind = "clipboard", args = { eid } }

            items[#items + 1] = {
                label = name,
                detail = detail .. "  " .. eid,
                icon = "🌡️",
                copy_text = eid,
                actions = actions,
            }
            ::next_climate::
        end

        table.sort(items, function(a, b) return a.label < b.label end)

        if #items == 0 then
            return { title = "Climate", items = { { label = "No climate entities found", icon = "📭" } } }
        end
        return { title = "Climate — " .. #items, items = items }
    end,
})
