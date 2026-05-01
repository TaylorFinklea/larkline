-- GitHub: Issues — open issues assigned to you.
-- Shared helpers copied from lib.lua.

-- SHARED: error_item — canonical copy in examples/plugins/_shared/errors.lua.
local function error_item(opts)
    return {
        label = opts.label,
        detail = opts.detail,
        icon = opts.icon or "!",
        retry_action = opts.retry_action,
        help_url = opts.help_url,
    }
end

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
        items = { error_item({
            label = "GITHUB_TOKEN not set",
            detail = "Add it to ~/.config/larkline/.env",
            help_url = "https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens",
        }) },
    }
end

local function github_http_error(status, extra)
    if status == 401 or status == 403 then
        return error_item({
            label = "GitHub auth failed",
            detail = (extra and (extra .. " · ") or "") .. "Run `gh auth login` or refresh GITHUB_TOKEN",
            help_url = "https://docs.github.com/en/authentication",
        })
    end
    if status == 429 then
        return error_item({
            label = "GitHub rate limited",
            detail = "Try again in a few minutes",
            help_url = "https://docs.github.com/en/rest/overview/resources-in-the-rest-api#rate-limiting",
        })
    end
    return error_item({
        label = "GitHub API error",
        detail = "HTTP " .. tostring(status) .. (extra and (" · " .. extra) or ""),
        help_url = "https://docs.github.com/en/rest",
    })
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
        local token, err = github_token_or_error("My Issues")
        if err then return err end

        local resp = lark.http.get(
            "https://api.github.com/search/issues?q=is:issue+is:open+assignee:@me&sort=updated&per_page=25",
            { headers = gh_headers(token), timeout = 10 }
        )

        if resp.status ~= 200 then
            return {
                title = "My Issues",
                items = { github_http_error(resp.status) },
            }
        end

        local ok, data = pcall(lark.json.decode, resp.body)
        if not ok or not data.items then
            return {
                title = "My Issues",
                items = { error_item({ label = "Failed to parse response" }) },
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
                -- Telescope previewer (lark.nvim v0.14.0): search/issues already
                -- returns the issue body, so plumbing this is zero-cost.
                preview = issue.body,
            }
        end

        return { title = "My Issues — " .. #items, items = items }
    end,
})
