-- Notes: Browse — list top-level folders and files in the vault.
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
        if not vault then return { title = "Browse Vault", items = err } end

        -- List top-level entries.
        local raw = lark.exec("ls", { "-1", vault })
        if not raw or raw == "" then
            return { title = "Browse Vault", items = { { label = "Empty vault", icon = "📭" } } }
        end

        local folders = {}
        local files = {}
        for entry in raw:gmatch("[^\n]+") do
            -- Skip hidden files/folders and .obsidian config.
            if not entry:match("^%.") then
                local full = vault .. "/" .. entry
                local is_dir = lark.exec("test", { "-d", full })
                if is_dir ~= nil then
                    -- Count markdown files in folder.
                    local count_raw = lark.exec("sh", { "-c", "find '" .. full .. "' -name '*.md' | wc -l" })
                    local count = tonumber(count_raw and count_raw:gsub("%s+", "") or "0") or 0
                    folders[#folders + 1] = { name = entry, path = full, count = count }
                elseif entry:match("%.md$") then
                    files[#files + 1] = { name = entry:gsub("%.md$", ""), path = full }
                end
            end
        end

        -- Sort folders and files alphabetically.
        table.sort(folders, function(a, b) return a.name < b.name end)
        table.sort(files, function(a, b) return a.name < b.name end)

        local items = {}
        for _, f in ipairs(folders) do
            items[#items + 1] = {
                label = f.name,
                detail = f.count .. " note" .. (f.count == 1 and "" or "s"),
                icon = "📁",
                actions = {
                    { label = "Open in Finder", kind = "shell", args = { "open", f.path } },
                    { label = "Copy path", kind = "clipboard", args = { f.path } },
                },
            }
        end
        for _, f in ipairs(files) do
            items[#items + 1] = {
                label = f.name,
                icon = "📄",
                actions = {
                    { label = "Open in Obsidian", kind = "open", args = { "obsidian://open?vault=" .. vault:match("([^/]+)$") .. "&file=" .. f.name } },
                    { label = "Open in editor", kind = "shell", args = { "open", f.path } },
                    { label = "Copy path", kind = "clipboard", args = { f.path } },
                },
            }
        end

        return { title = "Vault — " .. #folders .. " folders, " .. #files .. " files", items = items }
    end,
})
