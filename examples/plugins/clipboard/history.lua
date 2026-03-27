-- Clipboard: History — track and restore recently copied text.

local MAX_HISTORY = 20

lark.register({
    on_run = function()
        -- Capture current clipboard content.
        local current = lark.exec("pbpaste", {})
        if current then
            current = current:gsub("\n$", "")  -- trim single trailing newline
        end

        -- Load stored history (array of strings).
        local history = lark.store.get("history") or {}

        -- Prepend current clipboard if it's new and non-empty.
        if current and current ~= "" and current ~= history[1] then
            table.insert(history, 1, current)
            if #history > MAX_HISTORY then
                history[MAX_HISTORY + 1] = nil
            end
            lark.store.set("history", history)
        end

        if #history == 0 then
            return {
                title = "Clipboard History",
                items = { {
                    label  = "No clipboard history yet",
                    icon   = "◈",
                    detail = "Copy some text, then reopen this plugin",
                } },
            }
        end

        local items = {}
        for i, entry in ipairs(history) do
            -- Use first line for the label, truncated to 60 chars.
            local first_line = entry:match("^([^\n]+)") or entry
            local label = first_line
            if #label > 60 then
                label = label:sub(1, 57) .. "..."
            end

            -- Detail: show line count if multiline.
            local line_count = 0
            for _ in entry:gmatch("\n") do line_count = line_count + 1 end
            local detail = ""
            if line_count > 0 then
                detail = (line_count + 1) .. " lines"
            elseif #entry > 60 then
                detail = #entry .. " chars"
            end

            local icon = i == 1 and "●" or "◈"

            items[#items + 1] = {
                label     = label,
                detail    = detail,
                icon      = icon,
                copy_text = entry,
                actions   = {
                    { label = "Restore to Clipboard", kind = "clipboard", args = { entry } },
                    { label = "Copy",                 kind = "clipboard", args = { entry } },
                },
            }
        end

        return { title = "Clipboard History — " .. #items, items = items }
    end,
})
