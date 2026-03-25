-- GitHub: My PRs — open pull requests you authored.

lark.register({
    on_run = function()
        local token = lark.env("GITHUB_TOKEN")
        if not token then
            return {
                title = "My PRs",
                items = { { label = "GITHUB_TOKEN not set", detail = "Add it to ~/.config/larkline/.env", icon = "!" } },
            }
        end

        local headers = {
            Authorization = "Bearer " .. token,
            Accept = "application/vnd.github+json",
        }

        local resp = lark.http.get(
            "https://api.github.com/search/issues?q=is:pr+is:open+author:@me&sort=updated&per_page=25",
            { headers = headers, timeout = 10 }
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
            items[#items + 1] = {
                label = pr.title,
                detail = repo .. " #" .. pr.number,
                icon = "⊙",
                url = pr.html_url,
                copy_text = pr.html_url,
                actions = {
                    { label = "Open in browser", kind = "open", args = { pr.html_url } },
                    { label = "Copy URL", kind = "clipboard", args = { pr.html_url } },
                },
            }
        end

        return { title = "My PRs — " .. #items, items = items }
    end,
})
