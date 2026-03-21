-- Run Counter — demonstrates lark.store persistent storage.
-- Each run increments a counter that persists across lark sessions.

lark.register({
    on_run = function()
        -- Read or initialize the counter.
        local count = lark.store.get("run_count") or 0
        count = count + 1
        lark.store.set("run_count", count)

        -- Record a timestamp via subprocess (os.date unavailable in sandbox).
        local timestamp = lark.exec("date", { "+%Y-%m-%d %H:%M:%S" })
        timestamp = timestamp:match("^(.-)%s*$") -- trim trailing newline
        lark.store.set("last_run", timestamp)

        -- List all stored keys.
        local keys = lark.store.keys()

        return {
            title = "Run Counter",
            items = {
                {
                    label = "Times run: " .. tostring(count),
                    detail = "This count persists across lark sessions",
                    icon = "🔢",
                    copy_text = tostring(count),
                },
                {
                    label = "Last run: " .. timestamp,
                    detail = "Stored via lark.store.set()",
                    icon = "🕐",
                },
                {
                    label = "Stored keys: " .. table.concat(keys, ", "),
                    detail = "Retrieved via lark.store.keys()",
                    icon = "🔑",
                },
            },
        }
    end,
})
