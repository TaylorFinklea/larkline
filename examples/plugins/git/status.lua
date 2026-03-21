-- Status — working tree changes (git status --short).
-- Note: lark.exec() uses tokio::process::Command with explicit args (safe, no shell).

lark.register({
    on_run = function()
        -- Check if we are in a git repo.
        local check = lark.exec("git", { "rev-parse", "--git-dir" })
        if check == "" then
            return {
                title = "Git Status",
                items = { { label = "Not a git repo", detail = "Run lark from a git project", icon = "!" } },
            }
        end

        local raw = lark.exec("git", { "status", "--short" })

        local items = {}
        for line in raw:gmatch("[^\n]+") do
            local status_code, file = line:match("^(..) (.+)$")
            if file then
                local trimmed = status_code:gsub("%s+", "")
                local icon = trimmed == "M" and "M"
                    or trimmed == "??" and "?"
                    or trimmed == "D" and "D"
                    or trimmed == "A" and "A"
                    or "~"
                table.insert(items, { label = file, detail = status_code, icon = icon })
            end
        end

        if #items == 0 then
            items = { { label = "Working tree clean", icon = "✓" } }
        end

        return { title = "Git Status", items = items }
    end,
})
