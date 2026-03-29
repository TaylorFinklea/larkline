-- Claude Usage: Blocks — billing blocks with timestamps and burn rate.

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

local function fmt_time(iso)
    if type(iso) ~= "string" then return "" end
    return iso:match("T(%d+:%d+)") or ""
end

lark.register({
    on_run = function()
        local raw = lark.exec("npx", { "ccusage", "blocks", "--json", "--recent", "--order", "desc" })
        if not raw or raw == "" then
            return { title = "Claude Usage", items = { { label = "ccusage not found — npm i -g ccusage", icon = "!" } } }
        end

        local ok, data = pcall(lark.json.decode, raw)
        if not ok or not data or not data.blocks then
            return { title = "Claude Usage", items = { { label = "Failed to parse output", icon = "!" } } }
        end

        local items = {}
        local total_cost = 0
        for _, block in ipairs(data.blocks) do
            if block.isGap then goto next_block end
            total_cost = total_cost + (block.costUSD or 0)

            local start_time = fmt_time(block.startTime)
            local end_time = fmt_time(block.endTime or block.actualEndTime)
            local date = tostring(block.startTime or ""):match("^(%d+-%d+-%d+)") or ""
            local cost = fmt_cost(block.costUSD)
            local tokens = fmt_tokens(block.totalTokens)
            local entries = tostring(block.entries or 0)
            local models = ""
            if type(block.models) == "table" then
                models = table.concat(block.models, ", ")
            end

            local icon = "📊"
            local label_prefix = ""
            if block.isActive then
                icon = "🔴"
                label_prefix = "ACTIVE  "
                if type(block.burnRate) == "number" then
                    label_prefix = label_prefix .. fmt_cost(block.burnRate) .. "/hr  "
                end
            end

            local time_range = start_time .. "–" .. end_time
            local detail = entries .. " entries  " .. tokens .. " tokens  " .. models

            if block.isActive and type(block.projection) == "table" then
                if type(block.projection.projectedCost) == "number" then
                    detail = detail .. "\nProjected: " .. fmt_cost(block.projection.projectedCost)
                end
            end

            items[#items + 1] = {
                label = label_prefix .. date .. " " .. time_range .. "  " .. cost,
                detail = detail,
                icon = icon,
                copy_text = cost,
            }
            ::next_block::
        end

        if #items == 0 then
            return { title = "Claude Usage — Blocks", items = { { label = "No recent blocks", icon = "📭" } } }
        end

        table.insert(items, 1, {
            label = "Total (recent): " .. fmt_cost(total_cost),
            detail = #items .. " blocks",
            icon = "💰",
            copy_text = fmt_cost(total_cost),
        })

        return { title = "Claude Usage — Blocks", items = items }
    end,
})
