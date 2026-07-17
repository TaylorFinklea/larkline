-- Clipboard: History — persistent clipboard history using lark.clipboard_read + lark.store.
-- No external dependencies (Maccy/Flycut not required).

local MAX_ENTRIES = 50

-- The sandboxed VM ships no `os` library — indexing the nil global crashed
-- every capture. Nil-safe lookup keeps timestamps if the sandbox ever adds it.
local os_lib = rawget(_G, "os")

lark.register({
    on_run = function()
        -- Read current clipboard.
        local current = lark.clipboard_read()

        -- Load history from persistent store.
        local raw = lark.store.get("history")
        local history = {}
        if raw then
            local ok, decoded = pcall(lark.json.decode, raw)
            if ok and type(decoded) == "table" then
                history = decoded
            end
        end

        -- If clipboard has new content, prepend it (dedup by value).
        if current and current ~= "" then
            local dominated = false
            for i, entry in ipairs(history) do
                if entry.value == current then
                    dominated = true
                    -- Move to front.
                    table.remove(history, i)
                    break
                end
            end
            table.insert(history, 1, {
                value = current,
                time = os_lib and os_lib.time() or 0,
            })
        end

        -- Trim to max.
        while #history > MAX_ENTRIES do
            table.remove(history)
        end

        -- Persist.
        lark.store.set("history", lark.json.encode(history))

        -- Build output items.
        if #history == 0 then
            return {
                title = "Clipboard History",
                items = { { label = "No clipboard history yet", icon = "📭" } },
            }
        end

        local items = {}
        for i, entry in ipairs(history) do
            local value = entry.value or ""
            local label = value:match("^([^\n]+)") or value
            if #label > 80 then
                label = label:sub(1, 77) .. "..."
            end

            local line_count = select(2, value:gsub("\n", "")) + 1
            local detail = ""
            if line_count > 1 then
                detail = line_count .. " lines"
            elseif #value > 80 then
                detail = #value .. " chars"
            end

            items[#items + 1] = {
                label = label,
                detail = detail,
                icon = i == 1 and "●" or "○",
                copy_text = value,
                actions = {
                    { label = "Paste to clipboard", kind = "clipboard", args = { value } },
                },
            }
        end

        return { title = "Clipboard — " .. #history .. " entries", items = items }
    end,
})
