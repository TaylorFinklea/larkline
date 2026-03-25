-- Todo: Tasks — list all tasks with complete/delete actions.
-- Data persisted via lark.store.

lark.register({
    on_run = function()
        local raw = lark.store.get("tasks")
        local tasks = raw or {}

        if #tasks == 0 then
            return {
                title = "Tasks",
                items = {
                    { label = "No tasks yet", detail = "Use 'Add Task' to create one", icon = "📭" },
                },
            }
        end

        local items = {}
        for i, task in ipairs(tasks) do
            local icon = task.done and "✓" or "○"
            local label = task.done and ("~~" .. task.title .. "~~") or task.title
            local detail = task.due and ("Due: " .. task.due) or nil

            items[#items + 1] = {
                label = label,
                detail = detail,
                icon = icon,
                copy_text = task.title,
                actions = {
                    {
                        label = task.done and "Mark incomplete" or "Mark complete",
                        kind = "shell",
                        args = { "lark", "invoke", "Todo", "Tasks" },
                        confirm = false,
                        id = "toggle_" .. tostring(i),
                    },
                    {
                        label = "Delete",
                        kind = "shell",
                        args = { "lark", "invoke", "Todo", "Tasks" },
                        confirm = true,
                        id = "delete_" .. tostring(i),
                    },
                    {
                        label = "Copy",
                        kind = "clipboard",
                        args = { task.title },
                    },
                },
            }
        end

        -- Summary line at the end.
        local done_count = 0
        for _, t in ipairs(tasks) do
            if t.done then done_count = done_count + 1 end
        end
        items[#items + 1] = {
            label = done_count .. "/" .. #tasks .. " completed",
            detail = "Total tasks",
            icon = "📊",
        }

        return { title = "Tasks", items = items }
    end,
})
