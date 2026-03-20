-- Git Branches — recent local branches with last-commit info.
-- Note: lark.exec() uses tokio::process::Command with explicit args (safe, no shell).

lark.register({
    on_run = function()
        -- Check if we're in a git repo.
        local check = lark.exec("git", { "rev-parse", "--git-dir" })
        if check == "" then
            return {
                title = "Git Branches",
                items = { { label = "Not a git repo", detail = "Run lark from a git project", icon = "!" } },
            }
        end

        local raw = lark.exec("git", {
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname:short)|%(committerdate:relative)|%(subject)",
            "refs/heads/",
            "--count=20",
        })

        local items = {}
        for line in raw:gmatch("[^\n]+") do
            local branch, date, subject = line:match("^([^|]+)|([^|]+)|(.+)$")
            if branch then
                table.insert(items, {
                    label = branch,
                    detail = date .. "  " .. (subject or ""):sub(1, 50),
                    icon = "B",
                    actions = {
                        { label = "Copy branch name", command = "clipboard", args = { branch } },
                    },
                })
            end
        end

        if #items == 0 then
            items = { { label = "No branches found", icon = "-" } }
        end

        return { title = "Git Branches", items = items }
    end,
})
