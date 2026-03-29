-- Codex Usage: Daily — OpenAI Codex token and cost by day.

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

lark.register({
    on_run = function()
        local raw = lark.exec("npx", { "@ccusage/codex", "daily", "--json" })
        if not raw or raw == "" then
            return { title = "Codex Usage", items = { { label = "codex not found — npx @ccusage/codex --help", icon = "!" } } }
        end

        local ok, data = pcall(lark.json.decode, raw)
        if not ok or not data or not data.daily then
            return { title = "Codex Usage", items = { { label = "Failed to parse output", icon = "!" } } }
        end

        local items = {}
        local total_cost = 0
        for i = #data.daily, 1, -1 do
            local day = data.daily[i]
            total_cost = total_cost + (day.costUSD or 0)
            local date = tostring(day.date or "?")
            local cost = fmt_cost(day.costUSD)
            local tokens = fmt_tokens(day.totalTokens)
            local models = {}
            if type(day.models) == "table" then
                for name, _ in pairs(day.models) do
                    models[#models + 1] = name
                end
            end

            items[#items + 1] = {
                label = date .. "  " .. cost,
                detail = tokens .. " tokens  " .. table.concat(models, ", "),
                icon = "📅",
                copy_text = cost,
            }
        end

        if #items == 0 then
            return { title = "Codex Usage — Daily", items = { { label = "No usage data", icon = "📭" } } }
        end

        table.insert(items, 1, {
            label = "Total: " .. fmt_cost(total_cost),
            detail = #data.daily .. " days",
            icon = "💰",
            copy_text = fmt_cost(total_cost),
        })

        return { title = "Codex Usage — Daily", items = items }
    end,
})
