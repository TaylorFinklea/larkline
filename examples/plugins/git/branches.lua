-- Git Branches — current branch and last commit across all tracked repos.

local function repo_name(path)
    return path:match("([^/]+)$") or path
end

lark.register({
    on_run = function()
        local repos = lark.store.get("repos") or {}

        if #repos == 0 then
            return {
                title = "Git Branches",
                items = {
                    { label = "No repos configured", detail = "Use Manage Repos or Scan Directory", icon = "📭" },
                },
            }
        end

        local items = {}
        for _, path in ipairs(repos) do
            local name = repo_name(path)

            local check = lark.exec("git", { "-C", path, "rev-parse", "--git-dir" })
            if not check or check == "" then
                items[#items + 1] = {
                    label = name,
                    detail = "Not a git repo: " .. path,
                    icon = "!",
                }
            else
                local branch = lark.exec("git", { "-C", path, "branch", "--show-current" })
                branch = branch and branch:gsub("%s+$", "") or "detached"

                local last_commit = lark.exec("git", { "-C", path, "log", "--oneline", "-1" })
                last_commit = last_commit and last_commit:gsub("%s+$", "") or ""

                -- Ahead/behind upstream.
                local ahead_behind = ""
                local counts = lark.exec("git", { "-C", path, "rev-list", "--left-right", "--count", "HEAD...@{upstream}" })
                if counts and counts ~= "" then
                    local ahead, behind = counts:match("^(%d+)%s+(%d+)")
                    if ahead and behind then
                        local parts = {}
                        if tonumber(ahead) > 0 then parts[#parts + 1] = "↑" .. ahead end
                        if tonumber(behind) > 0 then parts[#parts + 1] = "↓" .. behind end
                        if #parts > 0 then
                            ahead_behind = "  " .. table.concat(parts, " ")
                        end
                    end
                end

                items[#items + 1] = {
                    label = name .. " — " .. branch .. ahead_behind,
                    detail = last_commit,
                    icon = "B",
                    copy_text = branch,
                    actions = {
                        { label = "Copy branch", kind = "clipboard", args = { branch } },
                        { label = "Open in Terminal", kind = "shell", args = { "open", "-a", "Terminal", path } },
                        { label = "Copy path", kind = "clipboard", args = { path } },
                    },
                }
            end
        end

        return { title = "Git Branches — " .. #repos .. " repos", items = items }
    end,
})
