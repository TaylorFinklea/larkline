-- Binary Sensors — on/off sensors with device_class-aware labels.

local function get_config()
    local raw_url = lark.store.get("ha_url")
    local url = (type(raw_url) == "string" and raw_url ~= "") and raw_url or nil
    if url and url:sub(1, 1) == '"' then url = url:sub(2, -2) end
    local token = lark.env("HA_TOKEN")
    if not url or url == "" then
        return nil, nil, { title = "Binary Sensors", items = {
            { label = "HA URL not configured — open Settings (S)", icon = "!" },
        }}
    end
    if not token or token == "" then
        return nil, nil, { title = "Binary Sensors", items = {
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

local function state_label(device_class, state)
    local on_labels = {
        motion = "Detected",
        door = "Open",
        window = "Open",
        opening = "Open",
        garage_door = "Open",
        lock = "Unlocked",
        moisture = "Wet",
        smoke = "Detected",
        gas = "Detected",
        co = "Detected",
        occupancy = "Occupied",
        vibration = "Detected",
        presence = "Home",
        problem = "Problem",
        safety = "Unsafe",
        sound = "Detected",
        tamper = "Detected",
        connectivity = "Connected",
        plug = "Plugged In",
        power = "On",
        running = "Running",
        heat = "Hot",
        cold = "Cold",
        light = "Light",
        battery = "Low",
        battery_charging = "Charging",
        moving = "Moving",
        update = "Available",
    }
    local off_labels = {
        motion = "Clear",
        door = "Closed",
        window = "Closed",
        opening = "Closed",
        garage_door = "Closed",
        lock = "Locked",
        moisture = "Dry",
        smoke = "Clear",
        gas = "Clear",
        co = "Clear",
        occupancy = "Clear",
        vibration = "Clear",
        presence = "Away",
        problem = "OK",
        safety = "Safe",
        sound = "Clear",
        tamper = "Clear",
        connectivity = "Disconnected",
        plug = "Unplugged",
        power = "Off",
        running = "Not Running",
        heat = "Normal",
        cold = "Normal",
        light = "No Light",
        battery = "Normal",
        battery_charging = "Not Charging",
        moving = "Stopped",
        update = "Up-to-date",
    }

    if state == "on" then
        return (device_class and on_labels[device_class]) or "On"
    else
        return (device_class and off_labels[device_class]) or "Off"
    end
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
            return { title = "Binary Sensors", items = { { label = "HA API error: " .. tostring(code), icon = "!" } } }
        end
        local ok, states = pcall(lark.json.decode, resp.body)
        if not ok or not states then
            return { title = "Binary Sensors", items = { { label = "Invalid JSON from HA", icon = "!" } } }
        end

        local items = {}
        for _, entity in ipairs(states) do
            local eid = entity.entity_id
            if type(eid) ~= "string" then goto next_label end
            if hidden_entities_set[eid] then goto next_label end
            if not eid:match("^binary_sensor%.") then goto next_label end

            local name = friendly_name(entity)
            local state = tostring(entity.state or "unknown")
            if hidden_states_set[state] then goto next_label end
            local attrs = (type(entity.attributes) == "table") and entity.attributes or {}
            local device_class = type(attrs.device_class) == "string" and attrs.device_class or nil
            local icon = state == "on" and "🟢" or "⚪"
            local label_text = state_label(device_class, state)
            local detail = label_text .. "  " .. eid

            items[#items + 1] = {
                label = name,
                detail = detail,
                icon = icon,
                copy_text = eid,
                actions = {
                    { label = "⭐ Favorite", kind = "shell",
                      args = { "bash", lark.env("HOME") .. "/.config/larkline/plugins/homeassistant/ha-manage.sh", "favorite", eid } },
                    { label = "🚫 Hide", kind = "shell",
                      args = { "bash", lark.env("HOME") .. "/.config/larkline/plugins/homeassistant/ha-manage.sh", "hide", eid } },
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
            return { title = "Binary Sensors", items = { { label = "No binary sensors found", icon = "📭" } } }
        end
        return { title = "Binary Sensors — " .. #items, items = items }
    end,
})
