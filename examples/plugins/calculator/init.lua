-- Calculator — evaluate math expressions via form input.
-- Supports standard Lua math: +, -, *, /, ^, %, math.sqrt(), math.pi, etc.

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

            -- Wrap in return so load() produces a value.
            local chunk, err = load("return " .. expr, "calc", "t", { math = math })
            if not chunk then
                return {
                    title = "Calculator",
                    items = {
                        { label = "Error: " .. tostring(err), detail = expr, icon = "!" },
                    },
                }
            end

            local ok, result = pcall(chunk)
            if not ok then
                return {
                    title = "Calculator",
                    items = {
                        { label = "Error: " .. tostring(result), detail = expr, icon = "!" },
                    },
                }
            end

            local display = tostring(result)
            return {
                title = "Calculator",
                items = {
                    {
                        label = display,
                        detail = expr,
                        icon = "=",
                        copy_text = display,
                        actions = {
                            { label = "Copy result", kind = "clipboard", args = { display } },
                        },
                    },
                },
            }
        end

        -- Show the input form.
        return {
            title = "Calculator",
            form = {
                fields = {
                    {
                        id = "expression",
                        label = "Expression",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "e.g. 2+2, math.sqrt(144), 100*1.08^5",
                    },
                },
                submit_label = "Calculate",
            },
        }
    end,
})
