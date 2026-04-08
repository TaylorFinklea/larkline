-- Shared helpers for Weather plugin.
-- This file is NOT loaded by require(). Instead, each command file
-- copies the helpers it needs inline, since the Lark sandbox does not
-- expose require/dofile/loadfile. This file serves as the canonical
-- source — edit here, then sync to the command files.
--
-- SYNC INSTRUCTIONS:
-- When editing helpers here, copy the updated helper functions to each
-- command file that uses them: current.lua, forecast.lua.
--
-- Helpers provided:
--   weather_icon(code) - map wttr.in weather codes to icons
--   get_location()     - return the saved location or nil for auto-detect

local function weather_icon(code)
    local c = tonumber(code) or 0
    if c == 113 then return "☀" end
    if c == 116 then return "⛅" end
    if c == 119 or c == 122 then return "☁" end
    if c >= 176 and c <= 185 then return "🌧" end
    if c >= 200 and c <= 232 then return "⛈" end
    if c >= 263 and c <= 302 then return "🌦" end
    if c >= 308 and c <= 359 then return "🌧" end
    if c >= 368 and c <= 395 then return "🌨" end
    return "🌤"
end

local function get_location()
    local saved = lark.store.get("weather_location")
    if saved and saved ~= "" then return saved end
    return nil
end
