-- Notes: Recent — recently modified notes in the vault.
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
        if not vault then return { title = "Recent Notes", items = err } end

        -- Find markdown files sorted by modification time (most recent first).
        local raw = lark.exec("find", { vault, "-name", "*.md", "-not", "-path", "*/.trash/*", "-not", "-path", "*/.obsidian/*" })
        if not raw or raw == "" then
            return { title = "Recent Notes", items = { { label = "No notes found", icon = "📭" } } }
        end

        -- Collect files with mtime.
        local files = {}
        for path in raw:gmatch("[^\n]+") do
            local stat = lark.exec("stat", { "-f", "%m", path })
            local mtime = tonumber(stat and stat:gsub("%s+$", "") or "0") or 0
            files[#files + 1] = { path = path, mtime = mtime }
        end

        -- Sort by mtime descending.
        table.sort(files, function(a, b) return a.mtime > b.mtime end)

        local now = tonumber(lark.exec("date", { "+%s" })) or 0
        local items = {}
        for i = 1, math.min(25, #files) do
            local f = files[i]
            local name = f.path:match("([^/]+)%.md$") or f.path:match("([^/]+)$") or "?"
            local rel = f.path:gsub("^" .. vault:gsub("([%(%)%.%%%+%-%*%?%[%]%^%$])", "%%%1") .. "/?", "")
            local folder = rel:match("(.+)/[^/]+$") or ""

            -- Time ago.
            local diff = now - f.mtime
            local ago
            if diff < 60 then ago = "just now"
            elseif diff < 3600 then ago = math.floor(diff / 60) .. "m ago"
            elseif diff < 86400 then ago = math.floor(diff / 3600) .. "h ago"
            else ago = math.floor(diff / 86400) .. "d ago"
            end

            local detail_parts = {}
            if folder ~= "" then detail_parts[#detail_parts + 1] = folder end
            detail_parts[#detail_parts + 1] = ago

            local actions = {}
            if lark.env("NVIM") then
                actions[#actions + 1] = { label = "Open in Neovim", kind = "nvim_edit", args = { f.path } }
            end
            actions[#actions + 1] = { label = "Open in Obsidian", kind = "open", args = { "obsidian://open?vault=" .. vault:match("([^/]+)$") .. "&file=" .. rel } }
            actions[#actions + 1] = { label = "Open in editor", kind = "shell", args = { "open", f.path } }
            actions[#actions + 1] = { label = "Copy path", kind = "clipboard", args = { f.path } }

            items[#items + 1] = {
                label = name,
                detail = table.concat(detail_parts, " · "),
                icon = "📄",
                actions = actions,
            }
        end

        return { title = "Recent Notes — " .. #items, items = items }
    end,
})
