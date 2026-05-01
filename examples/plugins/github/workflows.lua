-- GitHub: Workflow Runs — recent CI/CD runs across your repos.

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

local function time_ago(iso_str)
    if not iso_str then return "" end
    local ts = lark.exec("date", { "-jf", "%Y-%m-%dT%H:%M:%SZ", iso_str, "+%s" })
    if not ts or ts == "" then return "" end
    local now = tonumber(lark.exec("date", { "+%s" })) or 0
    local diff = now - (tonumber((ts:gsub("%s+$", ""))) or 0)
    if diff < 60 then return "just now" end
    if diff < 3600 then return math.floor(diff / 60) .. "m ago" end
    if diff < 86400 then return math.floor(diff / 3600) .. "h ago" end
    return math.floor(diff / 86400) .. "d ago"
end

lark.register({
    on_run = function()
        local token = lark.env("GITHUB_TOKEN")
        if not token then
            return {
                title = "Workflow Runs",
                items = { error_item({
                    label = "GITHUB_TOKEN not set",
                    detail = "Add it to ~/.config/larkline/.env",
                    help_url = "https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens",
                }) },
            }
        end

        local headers = {
            Authorization = "Bearer " .. token,
            Accept = "application/vnd.github+json",
        }

        -- Fetch user's recently-pushed repos, then get workflow runs for each.
        local repo_resp = lark.http.get(
            "https://api.github.com/user/repos?sort=pushed&per_page=10&type=owner",
            { headers = headers, timeout = 8 }
        )

        if repo_resp.status ~= 200 then
            local label = "Failed to fetch repos"
            local detail = "HTTP " .. repo_resp.status
            local help = "https://docs.github.com/en/rest"
            if repo_resp.status == 401 or repo_resp.status == 403 then
                label = "GitHub auth failed"
                detail = "Run `gh auth login` or refresh GITHUB_TOKEN"
                help = "https://docs.github.com/en/authentication"
            end
            return {
                title = "Workflow Runs",
                items = { error_item({ label = label, detail = detail, help_url = help }) },
            }
        end

        local ok, repos = pcall(lark.json.decode, repo_resp.body)
        if not ok or type(repos) ~= "table" then
            return {
                title = "Workflow Runs",
                items = { error_item({ label = "Failed to parse repos" }) },
            }
        end

        local runs = {}
        for _, repo in ipairs(repos) do
            local resp = lark.http.get(
                "https://api.github.com/repos/" .. repo.full_name .. "/actions/runs?per_page=3",
                { headers = headers, timeout = 8 }
            )
            if resp.status == 200 then
                local rok, data = pcall(lark.json.decode, resp.body)
                if rok and data.workflow_runs then
                    for _, run in ipairs(data.workflow_runs) do
                        run._repo = repo.full_name
                        runs[#runs + 1] = run
                    end
                end
            end
        end

        if #runs == 0 then
            return {
                title = "Workflow Runs",
                items = { { label = "No workflow runs found", icon = "📭" } },
            }
        end

        -- Sort by most recent first.
        table.sort(runs, function(a, b)
            return (a.created_at or "") > (b.created_at or "")
        end)

        local items = {}
        for i = 1, math.min(30, #runs) do
            local run = runs[i]
            local conclusion = type(run.conclusion) == "string" and run.conclusion or nil
            local status = type(run.status) == "string" and run.status or "unknown"

            local icon = "⏳"
            if status == "completed" then
                if conclusion == "success" then icon = "✅"
                elseif conclusion == "failure" then icon = "❌"
                elseif conclusion == "cancelled" then icon = "⊘"
                else icon = "⚠" end
            elseif status == "in_progress" then icon = "▶"
            end

            local display_status = conclusion or status
            local ago = time_ago(type(run.created_at) == "string" and run.created_at or nil)
            local repo_short = run._repo and run._repo:match("([^/]+)$") or ""
            local branch = type(run.head_branch) == "string" and run.head_branch or ""
            local run_url = type(run.html_url) == "string" and run.html_url or ""

            items[#items + 1] = {
                label = (type(run.name) == "string" and run.name or "workflow") .. " — " .. display_status,
                detail = repo_short .. "  " .. branch .. "  " .. ago,
                icon = icon,
                url = run_url,
                copy_text = run_url,
                actions = {
                    { label = "Open in browser", kind = "open", args = { run_url } },
                    { label = "Copy URL", kind = "clipboard", args = { run_url } },
                },
            }
        end

        return { title = "Workflow Runs — " .. #items, items = items }
    end,
})
