-- Tesela: Daily Note — open or create today's daily note.

lark.register({
    on_run = function()
        local raw = lark.exec("tesela", { "-d" })

        local msg = raw and raw:gsub("%s+$", "") or "Daily note created"

        return {
            title = "Daily Note",
            items = {
                {
                    label = msg,
                    icon = "📅",
                    actions = {
                        { label = "Open in Tesela TUI", kind = "shell", args = { "tesela", "tui" } },
                        { label = "Copy", kind = "clipboard", args = { msg } },
                    },
                },
            },
        }
    end,
})
