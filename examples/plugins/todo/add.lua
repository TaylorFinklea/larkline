-- Todo: Add Task — form to create a new task, persisted via lark.store.

lark.register({
    on_run = function()
        if lark.form_values then
            local title = lark.form_values.title or ""
            if title == "" then
                return {
                    title = "Add Task",
                    items = { { label = "Title is required", icon = "!" } },
                }
            end

            local tasks = lark.store.get("tasks") or {}
            tasks[#tasks + 1] = {
                title = title,
                due = lark.form_values.due ~= "" and lark.form_values.due or nil,
                done = false,
            }
            lark.store.set("tasks", tasks)

            return {
                title = "Add Task",
                items = {
                    {
                        label = "Added: " .. title,
                        detail = lark.form_values.due ~= "" and ("Due: " .. lark.form_values.due) or nil,
                        icon = "✅",
                    },
                    {
                        label = tostring(#tasks) .. " total tasks",
                        icon = "📊",
                    },
                },
            }
        end

        return {
            title = "Add Task",
            form = {
                fields = {
                    {
                        id = "title",
                        label = "Task",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "What needs to be done?",
                    },
                    {
                        id = "due",
                        label = "Due date",
                        type = { kind = "text" },
                        placeholder = "e.g. 2026-03-25 (optional)",
                    },
                },
                submit_label = "Add Task",
            },
        }
    end,
})
