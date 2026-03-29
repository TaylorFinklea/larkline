-- Claude Usage: Weekly — token and cost breakdown by week.

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

lark.register({
    on_run = function()
        local args = { "ccusage", "weekly", "--json", "--order", "desc" }
        local since = get_since()
        if since ~= "" then
            args[#args + 1] = "--since"
            args[#args + 1] = since
        end

        local raw = lark.exec("npx", args)
        if not raw or raw == "" then
            return { title = "Claude Usage", items = { { label = "ccusage not found", icon = "!" } } }
        end

        local ok, data = pcall(lark.json.decode, raw)
        if not ok or not data or not data.weekly then
            return { title = "Claude Usage", items = { { label = "Failed to parse output", icon = "!" } } }
        end

        local items = {}
        local total_cost = 0
        for _, week in ipairs(data.weekly) do
            total_cost = total_cost + (week.totalCost or 0)
            local period = tostring(week.week or week.date or "?")
            local cost = fmt_cost(week.totalCost)
            local tokens = fmt_tokens(week.totalTokens)

            items[#items + 1] = {
                label = period .. "  " .. cost,
                detail = tokens .. " tokens",
                icon = "📆",
                copy_text = cost,
            }
        end

        if #items == 0 then
            return { title = "Claude Usage — Weekly", items = { { label = "No usage data", icon = "📭" } } }
        end

        local range_label = tostring(lark.store.get("time_range") or "7d"):gsub('^"', ""):gsub('"$', "")
        table.insert(items, 1, {
            label = "Total: " .. fmt_cost(total_cost) .. "  (" .. range_label .. ")",
            detail = #data.weekly .. " weeks",
            icon = "💰",
            copy_text = fmt_cost(total_cost),
        })

        return { title = "Claude Usage — Weekly", items = items }
    end,
})
