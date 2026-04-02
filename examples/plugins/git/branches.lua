-- Git Branches — branches across all tracked repos with checkout action.

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
                    icon = "⚠",
                }
                goto next_repo
            end

            local current = lark.exec("git", { "-C", path, "branch", "--show-current" })
            current = current and current:gsub("%s+$", "") or "detached"

            local raw = lark.exec("git", { "-C", path, "branch", "--sort=-committerdate",
                "--format=%(refname:short)|%(committerdate:relative)|%(subject)" })

            if not raw or raw == "" then
                items[#items + 1] = {
                    label = name .. " — " .. current,
                    detail = "No branches found",
                    icon = "🌿",
                    copy_text = current,
                }
                goto next_repo
            end

            for line in raw:gmatch("[^\n]+") do
                local branch, date, subject = line:match("^([^|]+)|([^|]+)|(.*)$")
                if branch then
                    branch = branch:gsub("^%s+", ""):gsub("%s+$", "")
                    local is_current = (branch == current)
                    local icon = is_current and "★" or "○"
                    local prefix = is_current and "● " or "  "

                    local detail = date
                    if subject and subject ~= "" then
                        detail = detail .. "  " .. subject
                    end

                    local actions = {}
                    if not is_current then
                        actions[#actions + 1] = {
                            label = "Checkout " .. branch,
                            kind = "shell",
                            args = { "git", "-C", path, "checkout", branch },
                        }
                    end
                    actions[#actions + 1] = {
                        label = "Pull " .. branch,
                        kind = "shell",
                        args = { "git", "-C", path, "pull", "origin", branch },
                    }
                    if not is_current then
                        actions[#actions + 1] = {
                            label = "Delete " .. branch,
                            kind = "shell",
                            args = { "git", "-C", path, "branch", "-d", branch },
                        }
                    end
                    actions[#actions + 1] = { label = "Copy branch", kind = "clipboard", args = { branch } }
                    actions[#actions + 1] = { label = "Copy path", kind = "clipboard", args = { path } }

                    items[#items + 1] = {
                        label = prefix .. branch .. "  " .. name,
                        detail = detail,
                        icon = icon,
                        copy_text = branch,
                        actions = actions,
                    }
                end
            end
            ::next_repo::
        end

        return { title = "Git Branches — " .. #items .. " branches", items = items }
    end,
})
