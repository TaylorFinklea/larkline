-- Create Note — demonstrates lark form input.
-- First run shows a form; on submit the plugin re-runs with lark.form_values.

lark.register({
    on_run = function()
        -- If form values are present, the user submitted the form.
        if lark.form_values then
            local title = lark.form_values.title or "Untitled"
            local category = lark.form_values.category or "general"
            local urgent = lark.form_values.urgent or "false"

            local prefix = ""
            if urgent == "true" then prefix = "URGENT: " end

            -- Persist the note count.
            local count = (lark.store.get("note_count") or 0) + 1
            lark.store.set("note_count", count)

            return {
                title = "Note Created",
                items = {
                    {
                        label = prefix .. title,
                        detail = "Category: " .. category,
                        icon = "✅",
                        copy_text = title,
                        actions = {
                            {
                                label = "Copy note",
                                command = "clipboard",
                                args = { prefix .. title .. " [" .. category .. "]" },
                            },
                        },
                    },
                    {
                        label = "Total notes created: " .. tostring(count),
                        detail = "Stored via lark.store",
                        icon = "🔢",
                    },
                },
            }
        end

        -- No form values — show the form.
        return {
            title = "Create Note",
            form = {
                fields = {
                    {
                        id = "title",
                        label = "Title",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "Enter note title...",
                    },
                    {
                        id = "category",
                        label = "Category",
                        type = {
                            kind = "select",
                            options = { "general", "work", "personal", "ideas" },
                        },
                        default_value = "general",
                    },
                    {
                        id = "urgent",
                        label = "Urgent",
                        type = { kind = "toggle" },
                        default_value = "false",
                    },
                },
                submit_label = "Create Note",
            },
        }
    end,
})
