-- Notes: Search — full-text search across vault markdown files.
-- SHARED: get_vault_path() from lib.lua

local function get_vault_path()
    local stored = lark.store.get("notes_vault_path")
    if stored and stored ~= "" then return stored, nil end
    local home = lark.env("HOME") or "/tmp"
    local candidates = {
        home .. "/Documents/Obsidian", home .. "/Obsidian",
        home .. "/Documents/Notes", home .. "/Notes", home .. "/vaults",
    }
    for _, path in ipairs(candidates) do
        local check = lark.exec("test", { "-d", path })
        if check ~= nil then
            lark.store.set("notes_vault_path", path)
            return path, nil
        end
    end
    return nil, { { label = "No vault found", detail = "Use Settings to set vault path", icon = "!" } }
end

lark.register({
    on_run = function()
        local vault, err = get_vault_path()
        if not vault then return { title = "Search Notes", items = err } end

        -- Use form for search query.
        local query = lark.form_values and lark.form_values.query
        if not query or query == "" then
            return {
                title = "Search Notes",
                items = { { label = "Enter a search term below", icon = "🔍" } },
                form = {
                    fields = { { id = "query", label = "Search notes", type = "text" } },
                    submit_label = "Search",
                },
            }
        end

        -- Use grep to search markdown files.
        local raw = lark.exec("grep", { "-ril", "--include=*.md", query, vault })
        if not raw or raw == "" then
            return {
                title = "Search: " .. query,
                items = { { label = "No results for '" .. query .. "'", icon = "📭" } },
                form = {
                    fields = { { id = "query", label = "Search notes", type = "text" } },
                    submit_label = "Search",
                },
            }
        end

        local items = {}
        for path in raw:gmatch("[^\n]+") do
            local name = path:match("([^/]+)%.md$") or path:match("([^/]+)$") or path
            local rel = path:gsub("^" .. vault:gsub("([%(%)%.%%%+%-%*%?%[%]%^%$])", "%%%1") .. "/?", "")
            local folder = rel:match("(.+)/[^/]+$") or ""

            -- Get a context line with the match.
            local context_raw = lark.exec("grep", { "-m1", "-i", query, path })
            local context = context_raw and context_raw:gsub("^%s+", ""):sub(1, 80) or ""

            local detail_parts = {}
            if folder ~= "" then detail_parts[#detail_parts + 1] = folder end
            if context ~= "" then detail_parts[#detail_parts + 1] = context end

            local actions = {}
            if lark.env("NVIM") then
                actions[#actions + 1] = { label = "Open in Neovim", kind = "nvim_edit", args = { path } }
            end
            actions[#actions + 1] = { label = "Open in Obsidian", kind = "open", args = { "obsidian://open?vault=" .. vault:match("([^/]+)$") .. "&file=" .. rel } }
            actions[#actions + 1] = { label = "Open in editor", kind = "shell", args = { "open", path } }
            actions[#actions + 1] = { label = "Copy path", kind = "clipboard", args = { path } }

            items[#items + 1] = {
                label = name,
                detail = #detail_parts > 0 and table.concat(detail_parts, " · ") or nil,
                icon = "📄",
                actions = actions,
            }
            if #items >= 30 then break end
        end

        return {
            title = "Search: " .. query .. " — " .. #items,
            items = items,
            form = {
                fields = { { id = "query", label = "Search notes", type = "text" } },
                submit_label = "Search",
            },
        }
    end,
})
