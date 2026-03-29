-- Claude Usage: Sessions — token and cost breakdown by conversation.

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
        local raw = lark.exec("npx", { "ccusage", "session", "--json", "--order", "desc" })
        if not raw or raw == "" then
            return { title = "Claude Usage", items = { { label = "ccusage not found — npm i -g ccusage", icon = "!" } } }
        end

        local ok, data = pcall(lark.json.decode, raw)
        if not ok or not data or not data.sessions then
            return { title = "Claude Usage", items = { { label = "Failed to parse output", icon = "!" } } }
        end

        local items = {}
        local total_cost = 0
        for _, sess in ipairs(data.sessions) do
            total_cost = total_cost + (sess.totalCost or 0)
            local sid = tostring(sess.sessionId or "?")
            local name = sid:match("([^/%-]+)$") or sid
            if #name > 40 then name = name:sub(1, 40) end

            local cost = fmt_cost(sess.totalCost)
            local tokens = fmt_tokens(sess.totalTokens)
            local last = tostring(sess.lastActivity or "")
            local models = ""
            if type(sess.modelsUsed) == "table" then
                models = table.concat(sess.modelsUsed, ", ")
            end

            items[#items + 1] = {
                label = name .. "  " .. cost,
                detail = tokens .. " tokens  " .. last .. "  " .. models,
                icon = "💬",
                copy_text = sid,
            }
        end

        if #items == 0 then
            return { title = "Claude Usage — Sessions", items = { { label = "No session data", icon = "📭" } } }
        end

        table.insert(items, 1, {
            label = "Total: " .. fmt_cost(total_cost),
            detail = #data.sessions .. " sessions",
            icon = "💰",
            copy_text = fmt_cost(total_cost),
        })

        return { title = "Claude Usage — Sessions", items = items }
    end,
})
