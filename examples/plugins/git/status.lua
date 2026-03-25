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
        for _, path in ipairs(repos) do
            local name = repo_name(path)

            -- Verify it's a git repo.
            local check = lark.exec("git", { "-C", path, "rev-parse", "--git-dir" })
            if not check or check == "" then
                items[#items + 1] = {
                    label = name,
                    detail = "Not a git repo: " .. path,
                    icon = "!",
                }
            else
                -- Get current branch.
                local branch = lark.exec("git", { "-C", path, "branch", "--show-current" })
                branch = branch and branch:gsub("%s+$", "") or "detached"

                -- Get porcelain status.
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

                local is_clean = modified == 0 and untracked == 0 and staged == 0
                local icon = is_clean and "✓" or "●"

                local parts = {}
                if staged > 0 then parts[#parts + 1] = staged .. " staged" end
                if modified > 0 then parts[#parts + 1] = modified .. " modified" end
                if untracked > 0 then parts[#parts + 1] = untracked .. " untracked" end
                local summary = is_clean and "clean" or table.concat(parts, ", ")

                items[#items + 1] = {
                    label = name .. " — " .. summary,
                    detail = branch .. "  " .. path,
                    icon = icon,
                    copy_text = path,
                    actions = {
                        { label = "Open in Terminal", kind = "shell", args = { "open", "-a", "Terminal", path } },
                        { label = "Copy path", kind = "clipboard", args = { path } },
                    },
                }
            end
        end

        local dirty = 0
        for _, item in ipairs(items) do
            if item.icon == "●" then dirty = dirty + 1 end
        end
        local title = "Git Status — " .. #repos .. " repos"
        if dirty > 0 then title = title .. " (" .. dirty .. " dirty)" end

        return { title = title, items = items }
    end,
})
