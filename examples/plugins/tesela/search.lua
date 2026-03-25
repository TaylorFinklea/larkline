-- Tesela: Search Notes — search the knowledge base by keyword.

lark.register({
    on_run = function()
        if lark.form_values then
            local query = lark.form_values.query or ""
            if query == "" then
                return {
                    title = "Search Notes",
                    items = { { label = "No query entered", icon = "!" } },
                }
            end

            local raw = lark.exec("tesela", { "-s", query })
            if not raw or raw == "" then
                return {
                    title = "Search Notes",
                    items = { { label = "No results for: " .. query, icon = "📭" } },
                }
            end

            local items = {}
            for line in raw:gmatch("[^\n]+") do
                local trimmed = line:gsub("^%s+", ""):gsub("%s+$", "")
                if trimmed ~= "" then
                    items[#items + 1] = {
                        label = trimmed,
                        icon = "📄",
                        copy_text = trimmed,
                        actions = {
                            { label = "Open in Tesela", kind = "shell", args = { "tesela", "tui" } },
                            { label = "Copy", kind = "clipboard", args = { trimmed } },
                        },
                    }
                end
            end

            if #items == 0 then
                return {
                    title = "Search Notes",
                    items = { { label = "No results for: " .. query, icon = "📭" } },
                }
            end

            return { title = "Search: " .. query .. " — " .. #items .. " results", items = items }
        end

        return {
            title = "Search Notes",
            form = {
                fields = {
                    {
                        id = "query",
                        label = "Search",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "Search your notes...",
                    },
                },
                submit_label = "Search",
            },
        }
    end,
})
