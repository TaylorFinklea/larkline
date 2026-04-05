-- GitHub: Review Requests — PRs requesting your review with approve/comment actions.

local function gh_headers(token)
    return {
        Authorization = "Bearer " .. token,
        Accept = "application/vnd.github+json",
    }
end

lark.register({
    on_run = function()
        local token = lark.env("GITHUB_TOKEN")
        if not token then
            return {
                title = "Review Requests",
                items = { { label = "GITHUB_TOKEN not set", detail = "Add it to ~/.config/larkline/.env", icon = "!" } },
            }
        end

        local resp = lark.http.get(
            "https://api.github.com/search/issues?q=is:pr+is:open+review-requested:@me&sort=updated&per_page=25",
            { headers = gh_headers(token), timeout = 10 }
        )

        if resp.status ~= 200 then
            return {
                title = "Review Requests",
                items = { { label = "GitHub API error", detail = "HTTP " .. resp.status, icon = "!" } },
            }
        end

        local ok, data = pcall(lark.json.decode, resp.body)
        if not ok or not data.items then
            return {
                title = "Review Requests",
                items = { { label = "Failed to parse response", icon = "!" } },
            }
        end

        if #data.items == 0 then
            return {
                title = "Review Requests",
                items = { { label = "No pending reviews", icon = "✅" } },
            }
        end

        local items = {}
        for _, pr in ipairs(data.items) do
            local repo = pr.repository_url and pr.repository_url:match("repos/(.+)$") or ""
            local author = pr.user and pr.user.login or "?"
            local num = pr.number or 0
            local comments = pr.comments or 0
            local pr_url = pr.html_url or ""

            local detail_parts = { repo .. " #" .. num, "by " .. author }
            if comments > 0 then
                detail_parts[#detail_parts + 1] = "💬" .. comments
            end

            local actions = {
                { label = "Open in browser", kind = "open", args = { pr_url } },
            }
            if repo ~= "" then
                actions[#actions + 1] = {
                    label = "Approve",
                    kind = "shell",
                    args = { "gh", "pr", "review", tostring(num), "--repo", repo, "--approve" },
                    confirm = true,
                }
                actions[#actions + 1] = {
                    label = "Request Changes",
                    kind = "shell",
                    args = { "gh", "pr", "review", tostring(num), "--repo", repo, "--request-changes", "--body", "Changes requested via lark" },
                    confirm = true,
                }
                actions[#actions + 1] = {
                    label = "Checkout locally",
                    kind = "shell",
                    args = { "gh", "pr", "checkout", tostring(num), "--repo", repo },
                }
            end
            actions[#actions + 1] = { label = "Copy URL", kind = "clipboard", args = { pr_url } }

            items[#items + 1] = {
                label = pr.title,
                detail = table.concat(detail_parts, "  "),
                icon = "👀",
                url = pr_url,
                copy_text = pr_url,
                actions = actions,
            }
        end

        return { title = "Review Requests — " .. #items, items = items }
    end,
})
