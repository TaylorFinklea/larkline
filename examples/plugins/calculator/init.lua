-- Calculator — evaluate expressions via qalc (libqalculate) or bc fallback.
-- Install qalc for full features: brew install libqalculate
-- Supports: math, units (5km to miles), currency (100 USD to EUR), constants (pi, c, e)

lark.register({
    on_run = function()
        if lark.form_values then
            local expr = lark.form_values.expression or ""
            if expr == "" then
                return {
                    title = "Calculator",
                    items = { { label = "No expression entered", icon = "!" } },
                }
            end

            -- Try qalc first (full-featured), fall back to bc (basic math).
            local has_qalc = lark.exec("which", { "qalc" })
            local result, engine

            if has_qalc and has_qalc:match("qalc") then
                engine = "qalc"
                result = lark.exec("qalc", { "-t", expr })
            else
                engine = "bc"
                result = lark.exec("sh", { "-c", "echo '" .. expr:gsub("'", "") .. "' | bc -l 2>&1" })
            end

            if not result or result == "" then
                return {
                    title = "Calculator",
                    items = { { label = "Could not evaluate: " .. expr, icon = "!" } },
                }
            end

            result = result:gsub("%s+$", "")

            local items = {}
            for line in result:gmatch("[^\n]+") do
                line = line:gsub("^%s+", ""):gsub("%s+$", "")
                if line ~= "" then
                    items[#items + 1] = {
                        label = line,
                        detail = expr .. "  (" .. engine .. ")",
                        icon = "=",
                        copy_text = line,
                        actions = {
                            { label = "Copy result", kind = "clipboard", args = { line } },
                        },
                    }
                end
            end

            if #items == 0 then
                return {
                    title = "Calculator",
                    items = { { label = "No result for: " .. expr, icon = "!" } },
                }
            end

            return { title = "Calculator", items = items }
        end

        local placeholder = "e.g. 2+2, sqrt(144), 5km to miles, 100 USD to EUR"
        local has_qalc = lark.exec("which", { "qalc" })
        if not has_qalc or not has_qalc:match("qalc") then
            placeholder = "e.g. 2+2, 100*1.08^5, sqrt(144)  (install qalc for units/currency)"
        end

        return {
            title = "Calculator",
            form = {
                fields = {
                    {
                        id = "expression",
                        label = "Expression",
                        type = { kind = "text" },
                        required = true,
                        placeholder = placeholder,
                    },
                },
                submit_label = "Calculate",
            },
        }
    end,
})
