-- Lights — dedicated light control with brightness and color temp actions.

local function get_config()
    local raw_url = lark.store.get("ha_url")
    local url = (type(raw_url) == "string" and raw_url ~= "") and raw_url or nil
    if url and url:sub(1, 1) == '"' then url = url:sub(2, -2) end
    local token = lark.env("HA_TOKEN")
    if not url or url == "" then
        return nil, nil, { title = "Lights", items = {
            { label = "HA URL not configured — open Settings (S)", icon = "!" },
        }}
    end
    if not token or token == "" then
        return nil, nil, { title = "Lights", items = {
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
            return { title = "Lights", items = { { label = "HA API error: " .. tostring(code), icon = "!" } } }
        end
        local ok, states = pcall(lark.json.decode, resp.body)
        if not ok or not states then
            return { title = "Lights", items = { { label = "Invalid JSON from HA", icon = "!" } } }
        end

        local items = {}
        for _, entity in ipairs(states) do
            local eid = entity.entity_id
            if type(eid) ~= "string" then goto next_light end
            if not eid:match("^light%.") then goto next_light end

            local name = friendly_name(entity)
            local state = tostring(entity.state or "unknown")
            local icon = state == "on" and "💡" or "🌑"
            local attrs = (type(entity.attributes) == "table") and entity.attributes or {}
            local brightness = attrs.brightness
            local detail = state
            if state == "on" and type(brightness) == "number" then
                detail = detail .. "  " .. math.floor(brightness / 255 * 100) .. "%"
            end

            local actions = {
                { label = "Toggle", kind = "shell",
                  args = curl_service(url, token, "light/toggle", lark.json.encode({ entity_id = eid })),
                  confirm = true },
                { label = "Turn On", kind = "shell",
                  args = curl_service(url, token, "light/turn_on", lark.json.encode({ entity_id = eid })),
                  confirm = true },
                { label = "Turn Off", kind = "shell",
                  args = curl_service(url, token, "light/turn_off", lark.json.encode({ entity_id = eid })),
                  confirm = true },
            }

            -- Brightness presets (25%, 50%, 75%, 100%).
            for _, pct in ipairs({ 25, 50, 75, 100 }) do
                local bri = math.floor(pct / 100 * 255)
                actions[#actions + 1] = {
                    label = "Brightness " .. pct .. "%",
                    kind = "shell",
                    args = curl_service(url, token, "light/turn_on",
                        lark.json.encode({ entity_id = eid, brightness = bri })),
                    confirm = true,
                }
            end

            -- Color temperature presets if supported.
            if attrs.supported_color_modes then
                local supports_ct = false
                if type(attrs.supported_color_modes) == "table" then
                    for _, mode in ipairs(attrs.supported_color_modes) do
                        if mode == "color_temp" then supports_ct = true end
                    end
                end
                if supports_ct then
                    actions[#actions + 1] = {
                        label = "Warm White (2700K)",
                        kind = "shell",
                        args = curl_service(url, token, "light/turn_on",
                            lark.json.encode({ entity_id = eid, color_temp_kelvin = 2700 })),
                        confirm = true,
                    }
                    actions[#actions + 1] = {
                        label = "Cool White (4000K)",
                        kind = "shell",
                        args = curl_service(url, token, "light/turn_on",
                            lark.json.encode({ entity_id = eid, color_temp_kelvin = 4000 })),
                        confirm = true,
                    }
                    actions[#actions + 1] = {
                        label = "Daylight (6500K)",
                        kind = "shell",
                        args = curl_service(url, token, "light/turn_on",
                            lark.json.encode({ entity_id = eid, color_temp_kelvin = 6500 })),
                        confirm = true,
                    }
                end
            end

            actions[#actions + 1] = { label = "Copy entity ID", kind = "clipboard", args = { eid } }

            items[#items + 1] = {
                label = name,
                detail = detail .. "  " .. eid,
                icon = icon,
                copy_text = eid,
                actions = actions,
            }
            ::next_light::
        end

        table.sort(items, function(a, b) return a.label < b.label end)

        if #items == 0 then
            return { title = "Lights", items = { { label = "No lights found", icon = "📭" } } }
        end
        return { title = "Lights — " .. #items, items = items }
    end,
})
