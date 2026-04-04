-- Git Sync — show only repos that need pushing or pulling, with commit preview.

local function repo_name(path)
    return path:match("([^/]+)$") or path
end

lark.register({
    on_run = function()
        local repos = lark.store.get("repos") or {}

        if #repos == 0 then
            return {
                title = "Git Sync",
                items = {
                    { label = "No repos configured", detail = "Use Manage Repos or Scan Directory to add repos", icon = "📭" },
                },
            }
        end

        -- Fetch all repos first so ahead/behind is accurate.
        for _, path in ipairs(repos) do
            lark.exec("git", { "-C", path, "fetch", "--quiet" })
        end

        local items = {}
        local needs_attention = 0

        for _, path in ipairs(repos) do
            local name = repo_name(path)

            local check = lark.exec("git", { "-C", path, "rev-parse", "--git-dir" })
            if not check or check == "" then goto next_repo end

            local branch = lark.exec("git", { "-C", path, "branch", "--show-current" })
            branch = branch and branch:gsub("%s+$", "") or "detached"

            -- Ahead/behind upstream.
            local ahead, behind = 0, 0
            local counts = lark.exec("git", { "-C", path, "rev-list", "--left-right", "--count", "HEAD...@{upstream}" })
            if counts and counts ~= "" then
                local a, b = counts:match("^(%d+)%s+(%d+)")
                if a and b then ahead, behind = tonumber(a) or 0, tonumber(b) or 0 end
            end

            -- Dirty working tree.
            local porcelain = lark.exec("git", { "-C", path, "status", "--porcelain" })
            local dirty = porcelain and porcelain ~= ""

            -- Skip repos with nothing to do.
            if ahead == 0 and behind == 0 and not dirty then goto next_repo end

            needs_attention = needs_attention + 1

            -- Build status parts.
            local parts = {}
            if ahead > 0 then parts[#parts + 1] = "↑" .. ahead .. " to push" end
            if behind > 0 then parts[#parts + 1] = "↓" .. behind .. " to pull" end
            if dirty then parts[#parts + 1] = "uncommitted changes" end

            -- Get unpushed commit summaries for the detail preview.
            local unpushed_preview = ""
            if ahead > 0 then
                local log = lark.exec("git", { "-C", path, "log", "--oneline",
                    "@{upstream}..HEAD", "--format=%h %s" })
                if log and log ~= "" then
                    unpushed_preview = log:gsub("%s+$", "")
                end
            end

            -- Choose icon based on priority.
            local icon = "⬜"
            if dirty then icon = "🟡"
            elseif ahead > 0 and behind > 0 then icon = "🔴"
            elseif ahead > 0 then icon = "🔵"
            elseif behind > 0 then icon = "🟠"
            end

            -- Build actions.
            local actions = {}
            if ahead > 0 then
                actions[#actions + 1] = { label = "Push", kind = "shell",
                    args = { "git", "-C", path, "push" } }
            end
            if behind > 0 then
                actions[#actions + 1] = { label = "Pull (rebase)", kind = "shell",
                    args = { "git", "-C", path, "pull", "--rebase" } }
                actions[#actions + 1] = { label = "Pull (merge)", kind = "shell",
                    args = { "git", "-C", path, "pull" } }
            end
            actions[#actions + 1] = { label = "Fetch", kind = "shell",
                args = { "git", "-C", path, "fetch", "--all" } }
            actions[#actions + 1] = { label = "Open in Terminal", kind = "shell",
                args = { "open", "-a", "Terminal", path } }
            actions[#actions + 1] = { label = "Copy path", kind = "clipboard", args = { path } }

            items[#items + 1] = {
                label = name .. " — " .. table.concat(parts, ", "),
                detail = branch .. "  ·  " .. table.concat(parts, ", "),
                icon = icon,
                copy_text = unpushed_preview ~= "" and unpushed_preview or path,
                action = "detail:" .. path,
                actions = actions,
            }

            ::next_repo::
        end

        if #items == 0 then
            return {
                title = "Git Sync — all clean",
                items = {
                    { label = "All repos are in sync", detail = "Nothing to push or pull", icon = "✅" },
                },
            }
        end

        -- Sort: dirty first, then diverged, then ahead-only, then behind-only.
        table.sort(items, function(a, b)
            local function priority(item)
                local l = item.label
                if l:match("uncommitted") then return 1 end
                if l:match("to push") and l:match("to pull") then return 2 end
                if l:match("to push") then return 3 end
                return 4
            end
            local pa, pb = priority(a), priority(b)
            if pa ~= pb then return pa < pb end
            return a.label < b.label
        end)

        return {
            title = "Git Sync — " .. needs_attention .. " repo" .. (needs_attention ~= 1 and "s" or "") .. " need attention",
            items = items,
        }
    end,

    on_action = function(action)
        if not action then return end
        local path = action:match("^detail:(.+)$")
        if not path then return end

        local name = repo_name(path)
        local branch = lark.exec("git", { "-C", path, "branch", "--show-current" })
        branch = branch and branch:gsub("%s+$", "") or "detached"

        local ahead, behind = 0, 0
        local counts = lark.exec("git", { "-C", path, "rev-list", "--left-right", "--count", "HEAD...@{upstream}" })
        if counts and counts ~= "" then
            local a, b = counts:match("^(%d+)%s+(%d+)")
            if a and b then ahead, behind = tonumber(a) or 0, tonumber(b) or 0 end
        end

        local items = {}

        -- Show unpushed commits.
        if ahead > 0 then
            items[#items + 1] = { label = "── Unpushed (" .. ahead .. ") ──", detail = "", icon = "↑" }
            local log = lark.exec("git", { "-C", path, "log", "--oneline",
                "@{upstream}..HEAD", "--format=%h|%ar|%s" })
            if log and log ~= "" then
                for line in log:gmatch("[^\n]+") do
                    local hash, date, subject = line:match("^([^|]+)|([^|]+)|(.*)$")
                    if hash and subject then
                        items[#items + 1] = {
                            label = subject,
                            detail = hash:gsub("%s+$", "") .. "  " .. date,
                            icon = "◆",
                            copy_text = hash:gsub("%s+$", ""),
                            actions = {
                                { label = "Copy hash", kind = "clipboard", args = { hash:gsub("%s+$", "") } },
                                { label = "Show diff", kind = "shell",
                                    args = { "git", "-C", path, "show", "--stat", hash:gsub("%s+$", "") } },
                            },
                        }
                    end
                end
            end
        end

        -- Show incoming commits.
        if behind > 0 then
            items[#items + 1] = { label = "── Incoming (" .. behind .. ") ──", detail = "", icon = "↓" }
            local log = lark.exec("git", { "-C", path, "log", "--oneline",
                "HEAD..@{upstream}", "--format=%h|%ar|%s" })
            if log and log ~= "" then
                for line in log:gmatch("[^\n]+") do
                    local hash, date, subject = line:match("^([^|]+)|([^|]+)|(.*)$")
                    if hash and subject then
                        items[#items + 1] = {
                            label = subject,
                            detail = hash:gsub("%s+$", "") .. "  " .. date,
                            icon = "◇",
                            copy_text = hash:gsub("%s+$", ""),
                        }
                    end
                end
            end
        end

        -- Show dirty files.
        local porcelain = lark.exec("git", { "-C", path, "status", "--porcelain" })
        if porcelain and porcelain ~= "" then
            items[#items + 1] = { label = "── Uncommitted Changes ──", detail = "", icon = "✎" }
            for line in porcelain:gmatch("[^\n]+") do
                local xy = line:sub(1, 2)
                local file = line:sub(4)
                local icon = "📄"
                if xy:match("^%?%?") then icon = "➕"
                elseif xy:sub(1, 1) ~= " " then icon = "📦"
                else icon = "✏️"
                end
                items[#items + 1] = { label = file, detail = xy, icon = icon }
            end
        end

        -- Action buttons at the bottom.
        items[#items + 1] = { label = "─────────────", detail = "", icon = " " }

        if ahead > 0 then
            items[#items + 1] = {
                label = "Push " .. ahead .. " commit" .. (ahead ~= 1 and "s" or ""),
                detail = "git push",
                icon = "🚀",
                action = "shell:git -C " .. path .. " push",
            }
        end
        if behind > 0 then
            items[#items + 1] = {
                label = "Pull " .. behind .. " commit" .. (behind ~= 1 and "s" or ""),
                detail = "git pull --rebase",
                icon = "⬇️",
                action = "shell:git -C " .. path .. " pull --rebase",
            }
        end

        return {
            title = name .. " (" .. branch .. ") — " .. ahead .. "↑ " .. behind .. "↓",
            items = items,
        }
    end,
})
