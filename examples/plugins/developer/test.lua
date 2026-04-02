-- Developer: Test Plugin — run a plugin via lark invoke and show output.

lark.register({
    on_run = function()
        if lark.form_values then
            local name = lark.form_values.plugin_name or ""
            if name == "" then
                return { title = "Test Plugin", items = { { label = "Plugin name required", icon = "!" } } }
            end

            local raw = lark.exec("lark", { "invoke", name })
            if not raw or raw == "" then
                return {
                    title = "Test: " .. name,
                    items = { { label = "No output or plugin not found", icon = "!" } },
                }
            end

            local ok, data = pcall(lark.json.decode, raw)
            if ok and type(data) == "table" then
                local items = {}
                items[#items + 1] = {
                    label = "Title: " .. tostring(data.title or "none"),
                    icon = "📋",
                }
                if data.items then
                    items[#items + 1] = {
                        label = #data.items .. " items returned",
                        icon = "📊",
                    }
                    for i, item in ipairs(data.items) do
                        if i > 10 then
                            items[#items + 1] = { label = "... +" .. (#data.items - 10) .. " more", icon = "…" }
                            break
                        end
                        items[#items + 1] = {
                            label = tostring(item.label or ""),
                            detail = tostring(item.detail or ""),
                            icon = tostring(item.icon or "·"),
                        }
                    end
                end
                if data.raw_text then
                    items[#items + 1] = { label = "Has raw_text (" .. #data.raw_text .. " chars)", icon = "📝" }
                end
                if data.form then
                    items[#items + 1] = { label = "Has form (" .. #(data.form.fields or {}) .. " fields)", icon = "📝" }
                end
                return { title = "Test: " .. name, items = items }
            end

            return { title = "Test: " .. name, raw_text = raw }
        end

        return {
            title = "Test Plugin",
            form = {
                fields = {
                    {
                        id = "plugin_name",
                        label = "Plugin Name (as shown in lark)",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "e.g. Weather, Git:Status, Calculator",
                    },
                },
                submit_label = "Run Test",
            },
        }
    end,
})
