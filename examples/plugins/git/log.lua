-- Git Log — recent commits across all tracked repos.

local function repo_name(path)
    return path:match("([^/]+)$") or path
end

lark.register({
    on_run = function()
        local repos = lark.store.get("repos") or {}

        if #repos == 0 then
            return {
                title = "Git Log",
                items = {
                    { label = "No repos configured", icon = "📭" },
                },
            }
        end

        local items = {}
        for _, path in ipairs(repos) do
            local name = repo_name(path)

            local check = lark.exec("git", { "-C", path, "rev-parse", "--git-dir" })
            if not check or check == "" then goto next_repo end

            local raw = lark.exec("git", { "-C", path, "log", "--oneline", "-10",
                "--format=%h|%ar|%an|%s" })

            if not raw or raw == "" then goto next_repo end

            for line in raw:gmatch("[^\n]+") do
                local hash, date, author, subject = line:match("^([^|]+)|([^|]+)|([^|]+)|(.*)$")
                if hash and subject then
                    hash = hash:gsub("%s+$", "")
                    items[#items + 1] = {
                        label = subject,
                        detail = hash .. "  " .. author .. "  " .. date .. "  " .. name,
                        icon = "◆",
                        copy_text = hash,
                        actions = {
                            { label = "Copy hash", kind = "clipboard", args = { hash } },
                            { label = "Copy message", kind = "clipboard", args = { subject } },
                            { label = "Show diff", kind = "shell",
                                args = { "git", "-C", path, "show", "--stat", hash } },
                            { label = "Cherry-pick", kind = "shell",
                                args = { "git", "-C", path, "cherry-pick", hash } },
                            { label = "Open in Terminal", kind = "shell",
                                args = { "open", "-a", "Terminal", path } },
                        },
                    }
                end
            end
            ::next_repo::
        end

        if #items == 0 then
            return { title = "Git Log", items = { { label = "No commits found", icon = "📭" } } }
        end

        return { title = "Git Log — " .. #items .. " commits", items = items }
    end,
})
