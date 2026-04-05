-- GitHub: Issues — open issues assigned to you.

local function gh_headers(token)
    return {
        Authorization = "Bearer " .. token,
        Accept = "application/vnd.github+json",
    }
end

local function label_names(issue)
    if not issue.labels or #issue.labels == 0 then return nil end
    local names = {}
    for _, l in ipairs(issue.labels) do
        names[#names + 1] = l.name
    end
    return table.concat(names, ", ")
end

lark.register({
    on_run = function()
        local token = lark.env("GITHUB_TOKEN")
        if not token then
            return {
                title = "My Issues",
                items = { { label = "GITHUB_TOKEN not set", detail = "Add it to ~/.config/larkline/.env", icon = "!" } },
            }
        end

        local resp = lark.http.get(
            "https://api.github.com/search/issues?q=is:issue+is:open+assignee:@me&sort=updated&per_page=25",
            { headers = gh_headers(token), timeout = 10 }
        )

        if resp.status ~= 200 then
            return {
                title = "My Issues",
                items = { { label = "GitHub API error", detail = "HTTP " .. resp.status, icon = "!" } },
            }
        end

        local ok, data = pcall(lark.json.decode, resp.body)
        if not ok or not data.items then
            return {
                title = "My Issues",
                items = { { label = "Failed to parse response", icon = "!" } },
            }
        end

        if #data.items == 0 then
            return {
                title = "My Issues",
                items = { { label = "No open issues assigned to you", icon = "✅" } },
            }
        end

        local items = {}
        for _, issue in ipairs(data.items) do
            local repo = issue.repository_url and issue.repository_url:match("repos/(.+)$") or ""
            local num = issue.number or 0
            local comments = issue.comments or 0
            local issue_url = issue.html_url or ""

            local detail_parts = { repo .. " #" .. num }
            if comments > 0 then
                detail_parts[#detail_parts + 1] = "💬" .. comments
            end
            local lbls = label_names(issue)
            if lbls then
                detail_parts[#detail_parts + 1] = lbls
            end

            local actions = {
                { label = "Open in browser", kind = "open", args = { issue_url } },
            }
            if repo ~= "" then
                actions[#actions + 1] = {
                    label = "Close issue",
                    kind = "shell",
                    args = { "gh", "issue", "close", tostring(num), "--repo", repo },
                    confirm = true,
                }
            end
            actions[#actions + 1] = { label = "Copy URL", kind = "clipboard", args = { issue_url } }

            items[#items + 1] = {
                label = issue.title,
                detail = table.concat(detail_parts, "  "),
                icon = "●",
                url = issue_url,
                copy_text = issue_url,
                actions = actions,
            }
        end

        return { title = "My Issues — " .. #items, items = items }
    end,
})
