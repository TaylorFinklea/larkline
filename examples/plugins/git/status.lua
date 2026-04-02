-- Git Status — working tree status across all tracked repos.

local function repo_name(path)
    return path:match("([^/]+)$") or path
end

lark.register({
    on_run = function()
        local repos = lark.store.get("repos") or {}

        if #repos == 0 then
            return {
                title = "Git Status",
                items = {
                    { label = "No repos configured", detail = "Use Manage Repos or Scan Directory to add repos", icon = "📭" },
                },
            }
        end

        local items = {}
        local dirty_count = 0
        for _, path in ipairs(repos) do
            local name = repo_name(path)

            local check = lark.exec("git", { "-C", path, "rev-parse", "--git-dir" })
            if not check or check == "" then
                items[#items + 1] = {
                    label = name,
                    detail = "Not a git repo: " .. path,
                    icon = "⚠",
                }
                goto next_repo
            end

            -- Branch.
            local branch = lark.exec("git", { "-C", path, "branch", "--show-current" })
            branch = branch and branch:gsub("%s+$", "") or "detached"

            -- Porcelain status.
            local raw = lark.exec("git", { "-C", path, "status", "--porcelain" })
            local modified, untracked, staged = 0, 0, 0
            if raw and raw ~= "" then
                for line in raw:gmatch("[^\n]+") do
                    local xy = line:sub(1, 2)
                    if xy:match("^%?%?") then
                        untracked = untracked + 1
                    elseif xy:sub(1, 1) ~= " " and xy:sub(1, 1) ~= "?" then
                        staged = staged + 1
                    elseif xy:sub(2, 2) ~= " " then
                        modified = modified + 1
                    end
                end
            end

            -- Ahead/behind upstream.
            local ahead, behind = 0, 0
            local counts = lark.exec("git", { "-C", path, "rev-list", "--left-right", "--count", "HEAD...@{upstream}" })
            if counts and counts ~= "" then
                local a, b = counts:match("^(%d+)%s+(%d+)")
                if a and b then ahead, behind = tonumber(a) or 0, tonumber(b) or 0 end
            end

            -- Stash count.
            local stash_raw = lark.exec("git", { "-C", path, "stash", "list" })
            local stash_count = 0
            if stash_raw and stash_raw ~= "" then
                for _ in stash_raw:gmatch("[^\n]+") do stash_count = stash_count + 1 end
            end

            -- Last commit.
            local last_commit = lark.exec("git", { "-C", path, "log", "--oneline", "-1", "--format=%h %s" })
            last_commit = last_commit and last_commit:gsub("%s+$", "") or ""

            -- Build summary.
            local is_clean = modified == 0 and untracked == 0 and staged == 0
            if not is_clean then dirty_count = dirty_count + 1 end

            local parts = {}
            if staged > 0 then parts[#parts + 1] = staged .. " staged" end
            if modified > 0 then parts[#parts + 1] = modified .. " modified" end
            if untracked > 0 then parts[#parts + 1] = untracked .. " untracked" end
            if ahead > 0 then parts[#parts + 1] = "↑" .. ahead end
            if behind > 0 then parts[#parts + 1] = "↓" .. behind end
            if stash_count > 0 then parts[#parts + 1] = "📦" .. stash_count .. " stash" end

            local summary = is_clean and "clean" or table.concat(parts, ", ")
            local icon = is_clean and "✅" or "●"

            local detail = branch .. "  " .. last_commit

            -- Actions.
            local actions = {
                { label = "Pull", kind = "shell", args = { "git", "-C", path, "pull" } },
                { label = "Push", kind = "shell", args = { "git", "-C", path, "push" } },
                { label = "Fetch", kind = "shell", args = { "git", "-C", path, "fetch", "--all" } },
            }
            if stash_count > 0 then
                actions[#actions + 1] = { label = "Stash Pop", kind = "shell",
                    args = { "git", "-C", path, "stash", "pop" } }
            end
            actions[#actions + 1] = { label = "Open in Terminal", kind = "shell",
                args = { "open", "-a", "Terminal", path } }
            actions[#actions + 1] = { label = "Open in Editor", kind = "shell",
                args = { lark.env("EDITOR") or "vim", path } }
            actions[#actions + 1] = { label = "Copy branch", kind = "clipboard", args = { branch } }
            actions[#actions + 1] = { label = "Copy path", kind = "clipboard", args = { path } }

            items[#items + 1] = {
                label = name .. " — " .. summary,
                detail = detail,
                icon = icon,
                copy_text = path,
                actions = actions,
            }
            ::next_repo::
        end

        local title = "Git Status — " .. #repos .. " repos"
        if dirty_count > 0 then title = title .. " (" .. dirty_count .. " dirty)" end

        return { title = title, items = items }
    end,
})
