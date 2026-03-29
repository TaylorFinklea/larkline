-- Codex Usage: Sessions — OpenAI Codex token and cost by session.

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
        local raw = lark.exec("npx", { "@ccusage/codex", "session", "--json" })
        if not raw or raw == "" then
            return { title = "Codex Usage", items = { { label = "codex not found", icon = "!" } } }
        end

        local ok, data = pcall(lark.json.decode, raw)
        if not ok or not data or not data.sessions then
            return { title = "Codex Usage", items = { { label = "Failed to parse output", icon = "!" } } }
        end

        local items = {}
        local total_cost = 0
        for i = #data.sessions, 1, -1 do
            local sess = data.sessions[i]
            total_cost = total_cost + (sess.costUSD or 0)
            local name = tostring(sess.sessionId or sess.name or "?")
            if #name > 40 then name = name:sub(1, 40) end
            local cost = fmt_cost(sess.costUSD)
            local tokens = fmt_tokens(sess.totalTokens)
            local models = {}
            if type(sess.models) == "table" then
                for mname, _ in pairs(sess.models) do
                    models[#models + 1] = mname
                end
            end

            items[#items + 1] = {
                label = name .. "  " .. cost,
                detail = tokens .. " tokens  " .. table.concat(models, ", "),
                icon = "💬",
                copy_text = tostring(sess.sessionId or ""),
            }
        end

        if #items == 0 then
            return { title = "Codex Usage — Sessions", items = { { label = "No session data", icon = "📭" } } }
        end

        table.insert(items, 1, {
            label = "Total: " .. fmt_cost(total_cost),
            detail = #data.sessions .. " sessions",
            icon = "💰",
            copy_text = fmt_cost(total_cost),
        })

        return { title = "Codex Usage — Sessions", items = items }
    end,
})
