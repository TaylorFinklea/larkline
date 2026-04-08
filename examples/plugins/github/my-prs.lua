-- GitHub: My PRs — open pull requests you authored with quick-actions.
-- Shared helpers copied from lib.lua.

local function gh_headers(token)
    return {
        Authorization = "Bearer " .. token,
        Accept = "application/vnd.github+json",
    }
end

local function github_token_or_error(title)
    local token = lark.env("GITHUB_TOKEN")
    if token then return token end
    return nil, {
        title = title,
        items = { { label = "GITHUB_TOKEN not set", detail = "Add it to ~/.config/larkline/.env", icon = "!" } },
    }
end

local function state_icon(pr)
    if pr.draft then return "◌" end
    if pr.pull_request and pr.pull_request.merged_at then return "🟣" end
    return "⊙"
end

local function review_summary(pr)
    local labels = {}
    if pr.labels then
        for _, l in ipairs(pr.labels) do
            labels[#labels + 1] = l.name
        end
    end
    if #labels > 0 then return table.concat(labels, ", ") end
    return nil
end

lark.register({
    on_run = function()
        local token, err = github_token_or_error("My PRs")
        if err then return err end

        local resp = lark.http.get(
            "https://api.github.com/search/issues?q=is:pr+is:open+author:@me&sort=updated&per_page=25",
            { headers = gh_headers(token), timeout = 10 }
        )

        if resp.status ~= 200 then
            return {
                title = "My PRs",
                items = { { label = "GitHub API error", detail = "HTTP " .. resp.status, icon = "!" } },
            }
        end

        local ok, data = pcall(lark.json.decode, resp.body)
        if not ok or not data.items then
            return {
                title = "My PRs",
                items = { { label = "Failed to parse response", icon = "!" } },
            }
        end

        if #data.items == 0 then
            return {
                title = "My PRs",
                items = { { label = "No open PRs", icon = "✅" } },
            }
        end

        local items = {}
        for _, pr in ipairs(data.items) do
            local repo = pr.repository_url and pr.repository_url:match("repos/(.+)$") or ""
            local num = pr.number or 0
            local comments = pr.comments or 0
            local detail_parts = { repo .. " #" .. num }
            if comments > 0 then
                detail_parts[#detail_parts + 1] = "💬" .. comments
            end
            local label_str = review_summary(pr)
            if label_str then
                detail_parts[#detail_parts + 1] = label_str
            end

            local pr_url = pr.html_url or ""
            local actions = {
                { label = "Open in browser", kind = "open", args = { pr_url } },
            }
            if repo ~= "" then
                actions[#actions + 1] = {
                    label = "Merge PR",
                    kind = "shell",
                    args = { "gh", "pr", "merge", tostring(num), "--repo", repo, "--merge" },
                    confirm = true,
                }
                actions[#actions + 1] = {
                    label = "Squash & Merge",
                    kind = "shell",
                    args = { "gh", "pr", "merge", tostring(num), "--repo", repo, "--squash" },
                    confirm = true,
                }
                actions[#actions + 1] = {
                    label = "Close PR",
                    kind = "shell",
                    args = { "gh", "pr", "close", tostring(num), "--repo", repo },
                    confirm = true,
                }
            end
            actions[#actions + 1] = { label = "Copy URL", kind = "clipboard", args = { pr_url } }

            items[#items + 1] = {
                label = pr.title,
                detail = table.concat(detail_parts, "  "),
                icon = state_icon(pr),
                url = pr_url,
                copy_text = pr_url,
                actions = actions,
            }
        end

        return { title = "My PRs — " .. #items, items = items }
    end,
})
