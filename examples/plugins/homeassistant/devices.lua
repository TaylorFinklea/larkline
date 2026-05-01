-- Devices — list all entities with current state.

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
        return nil, nil, { title = "Home Assistant", items = {
            error_item({
                label = "HA_URL or HA_TOKEN not set",
                detail = "Add them to ~/.config/larkline/.env",
                help_url = "https://www.home-assistant.io/docs/authentication/",
            }),
            { label = "Open Settings (press S) to set your HA URL", icon = "🔧" },
        }}
    end
    if not token or token == "" then
        return nil, nil, { title = "Home Assistant", items = {
            error_item({
                label = "HA_URL or HA_TOKEN not set",
                detail = "Add them to ~/.config/larkline/.env",
                help_url = "https://www.home-assistant.io/docs/authentication/",
            }),
            { label = "Set via: lark secret set HA_TOKEN", detail = "Long-lived access token from HA profile", icon = "🔑" },
        }}
    end
    return url:gsub("/$", ""), token, nil
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

local function icon_for(entity_id, state)
    local d = tostring(entity_id):match("^([^%.]+)%.")
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
            return { title = "Home Assistant", items = { ha_http_error(resp, url .. "/api/states") } }
        end
        local ok, states = pcall(lark.json.decode, resp.body)
        if not ok or not states then
            return { title = "Home Assistant", items = { error_item({
                label = "Invalid JSON from Home Assistant",
                detail = "Response body could not be parsed",
                help_url = "https://developers.home-assistant.io/docs/api/rest/",
            }) } }
        end

        table.sort(states, function(a, b)
            local da = tostring(a.entity_id or ""):match("^([^%.]+)%.") or ""
            local db = tostring(b.entity_id or ""):match("^([^%.]+)%.") or ""
            if da ~= db then return da < db end
            return friendly_name(a) < friendly_name(b)
        end)

        local items = {}
        local show = { light = true, switch = true, binary_sensor = true, sensor = true,
                       climate = true, media_player = true, cover = true, lock = true, fan = true, camera = true }

        for _, entity in ipairs(states) do
            local eid = entity.entity_id
            if type(eid) ~= "string" then goto next_entity end
            if hidden_entities_set[eid] then goto next_entity end
            local domain = eid:match("^([^%.]+)%.")
            if not show[domain] then goto next_entity end

            local name = friendly_name(entity)
            local state = tostring(entity.state or "unknown")
            if hidden_states_set[state] then goto next_entity end
            local actions = {
                { label = "⭐ Favorite", kind = "shell",
                  args = { "bash", lark.env("HOME") .. "/.config/larkline/plugins/homeassistant/ha-manage.sh", "favorite", eid } },
                { label = "🚫 Hide", kind = "shell",
                  args = { "bash", lark.env("HOME") .. "/.config/larkline/plugins/homeassistant/ha-manage.sh", "hide", eid } },
                { label = "Copy entity ID", kind = "clipboard", args = { eid } },
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
                        "-d", lark.json.encode({ entity_id = eid }),
                    },
                    
                })
            end
            items[#items + 1] = {
                label = name,
                detail = state .. "  " .. eid,
                icon = icon_for(eid, state),
                copy_text = eid,
                actions = actions,
            }
            ::next_entity::
        end

        if #items == 0 then
            return { title = "Home Assistant", items = { { label = "No entities found", icon = "📭" } } }
        end
        return { title = "Home Assistant — " .. #items .. " devices", items = items }
    end,
})
