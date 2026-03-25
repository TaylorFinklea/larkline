-- Tesela: New Note — create a new note via form input.

lark.register({
    on_run = function()
        if lark.form_values then
            local title = lark.form_values.title or ""
            if title == "" then
                return {
                    title = "New Note",
                    items = { { label = "Title is required", icon = "!" } },
                }
            end

            local raw = lark.exec("tesela", { "-n", title })
            local msg = raw and raw:gsub("%s+$", "") or "Note created: " .. title

            return {
                title = "New Note",
                items = {
                    {
                        label = msg,
                        icon = "✅",
                        actions = {
                            { label = "Open in Tesela TUI", kind = "shell", args = { "tesela", "tui" } },
                            { label = "Copy title", kind = "clipboard", args = { title } },
                        },
                    },
                },
            }
        end

        return {
            title = "New Note",
            form = {
                fields = {
                    {
                        id = "title",
                        label = "Note Title",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "Enter note title...",
                    },
                },
                submit_label = "Create Note",
            },
        }
    end,
})
