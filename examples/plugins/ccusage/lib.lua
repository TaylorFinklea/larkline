-- Shared helpers for ccusage plugin.
-- This file is NOT loaded by require(). Instead, each command file
-- copies the helpers it needs inline, since the Lark sandbox does not
-- expose require/dofile/loadfile. This file serves as the canonical
-- source — edit here, then sync to the command files.
--
-- SYNC INSTRUCTIONS:
-- When editing helpers here, copy the updated helper functions to each
-- command file that uses them: daily.lua, sessions.lua, monthly.lua,
-- weekly.lua, blocks.lua.
--
-- Helpers provided:
--   fmt_tokens(n) - format token counts with K/M suffixes
--   fmt_cost(n)   - format cost as USD
--   get_since()   - derive ccusage --since cutoff from saved time_range

local function fmt_tokens(n)
    if type(n) ~= "number" then return "0" end
    if n >= 1000000 then return string.format("%.1fM", n / 1000000) end
    if n >= 1000 then return string.format("%.1fK", n / 1000) end
    return tostring(n)
end

local function fmt_cost(n)
    if type(n) ~= "number" then return "$0.00" end
    return string.format("$%.2f", n)
end

local function get_since()
    local raw = lark.store.get("time_range") or "7d"
    local range = tostring(raw):gsub('^"', ""):gsub('"$', "")
    if range == "today" then
        return (lark.exec("date", { "+%Y%m%d" }) or ""):match("%d+") or ""
    elseif range == "3d" then
        return (lark.exec("date", { "-v-3d", "+%Y%m%d" }) or ""):match("%d+") or ""
    elseif range == "7d" then
        return (lark.exec("date", { "-v-7d", "+%Y%m%d" }) or ""):match("%d+") or ""
    elseif range == "30d" then
        return (lark.exec("date", { "-v-30d", "+%Y%m%d" }) or ""):match("%d+") or ""
    end
    return ""
end
