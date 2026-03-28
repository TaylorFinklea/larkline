-- Shared helpers for Home Assistant plugin.

local M = {}

function M.get_config()
    -- URL from plugin settings (configured via Settings UI), token from secrets.
    local raw_url = lark.store.get("ha_url")
    local url = (type(raw_url) == "string" and raw_url ~= "") and raw_url or nil
    -- Strip JSON quotes if stored as a JSON string value.
    if url and url:sub(1, 1) == '"' then
        url = url:sub(2, -2)
    end
    local token = lark.env("HA_TOKEN")
    if not url or url == "" then
        return nil, nil, {
            title = "Home Assistant",
            items = {
                { label = "HA URL not configured", icon = "!" },
                { label = "Open Settings (press S) to set your HA URL", icon = "🔧" },
            },
        }
    end
    if not token or token == "" then
        return nil, nil, {
            title = "Home Assistant",
            items = {
                { label = "HA_TOKEN not set", icon = "!" },
                { label = "Set via: lark secret set HA_TOKEN", detail = "Long-lived access token from HA profile", icon = "🔑" },
            },
        }
    end
    -- Strip trailing slash.
    url = url:gsub("/$", "")
    return url, token, nil
end

function M.headers(token)
    return {
        Authorization = "Bearer " .. token,
        ["Content-Type"] = "application/json",
    }
end

function M.api_get(url, token, path)
    local resp = lark.http.get(url .. "/api/" .. path, {
        headers = M.headers(token),
        timeout = 8,
    })
    if not resp or resp == "" then return nil end
    return lark.json.decode(resp)
end

function M.api_post(url, token, path, body)
    local resp = lark.http.post(url .. "/api/" .. path, body or "", {
        headers = M.headers(token),
        timeout = 8,
    })
    if not resp or resp == "" then return nil end
    return lark.json.decode(resp)
end

-- Map entity domain to a display icon.
function M.icon_for(entity_id, state)
    local domain = entity_id:match("^([^%.]+)%.")
    if domain == "light" then
        return state == "on" and "💡" or "🌑"
    elseif domain == "switch" then
        return state == "on" and "🔌" or "⭕"
    elseif domain == "binary_sensor" then
        return state == "on" and "🟢" or "⚪"
    elseif domain == "sensor" then
        return "📊"
    elseif domain == "climate" then
        return "🌡️"
    elseif domain == "media_player" then
        return "🎵"
    elseif domain == "cover" then
        return state == "open" and "🪟" or "🔒"
    elseif domain == "lock" then
        return state == "locked" and "🔒" or "🔓"
    elseif domain == "fan" then
        return "🌀"
    elseif domain == "camera" then
        return "📷"
    elseif domain == "scene" then
        return "🎬"
    elseif domain == "automation" then
        return state == "on" and "⚙️" or "⏸️"
    else
        return "📦"
    end
end

-- Friendly name from attributes, falling back to entity_id.
function M.friendly_name(entity)
    if entity.attributes and entity.attributes.friendly_name then
        return entity.attributes.friendly_name
    end
    return entity.entity_id
end

return M
