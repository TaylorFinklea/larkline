-- Git Stash — view and manage stashes across all tracked repos.
-- Shared helpers copied from lib.lua.

local function repo_name(path)
    return path:match("([^/]+)$") or path
end

local function is_git_repo(path)
    local check = lark.exec("git", { "-C", path, "rev-parse", "--git-dir" })
    return check and check ~= ""
end

lark.register({
    on_run = function()
        local repos = lark.store.get("repos") or {}

        if #repos == 0 then
            return {
                title = "Git Stash",
                items = { { label = "No repos configured", icon = "📭" } },
            }
        end

        local items = {}
        for _, path in ipairs(repos) do
            local name = repo_name(path)

            if not is_git_repo(path) then goto next_repo end

            local raw = lark.exec("git", { "-C", path, "stash", "list",
                "--format=%gd|%ar|%gs" })

            if not raw or raw == "" then goto next_repo end

            for line in raw:gmatch("[^\n]+") do
                local ref, date, message = line:match("^([^|]+)|([^|]+)|(.*)$")
                if ref and message then
                    ref = ref:gsub("%s+$", "")
                    items[#items + 1] = {
                        label = message,
                        detail = ref .. "  " .. date .. "  " .. name,
                        icon = "📦",
                        copy_text = ref,
                        actions = {
                            { label = "Apply " .. ref, kind = "shell",
                                args = { "git", "-C", path, "stash", "apply", ref } },
                            { label = "Pop " .. ref, kind = "shell",
                                args = { "git", "-C", path, "stash", "pop", ref } },
                            { label = "Drop " .. ref, kind = "shell",
                                args = { "git", "-C", path, "stash", "drop", ref } },
                            { label = "Show diff", kind = "shell",
                                args = { "git", "-C", path, "stash", "show", "-p", ref } },
                            { label = "Copy ref", kind = "clipboard", args = { ref } },
                        },
                    }
                end
            end
            ::next_repo::
        end

        if #items == 0 then
            return { title = "Git Stash", items = { { label = "No stashes found", icon = "✅" } } }
        end

        return { title = "Git Stash — " .. #items .. " stashes", items = items }
    end,
})
