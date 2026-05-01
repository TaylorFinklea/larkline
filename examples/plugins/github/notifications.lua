-- GitHub: Notifications — unread notifications with mark-read actions.
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

local function type_icon(subject_type)
    if subject_type == "PullRequest" then return "⊙" end
    if subject_type == "Issue" then return "●" end
    if subject_type == "Release" then return "🏷" end
    if subject_type == "Discussion" then return "💬" end
    if subject_type == "CheckSuite" then return "▶" end
    return "○"
end

local function reason_label(reason)
    if reason == "review_requested" then return "review requested" end
    if reason == "mention" then return "mentioned" end
    if reason == "author" then return "your thread" end
    if reason == "assign" then return "assigned" end
    if reason == "ci_activity" then return "CI" end
    if reason == "subscribed" then return "watching" end
    return reason or ""
end

lark.register({
    on_run = function()
        local token, err = github_token_or_error("Notifications")
        if err then return err end

        local resp = lark.http.get(
            "https://api.github.com/notifications?per_page=30",
            { headers = gh_headers(token), timeout = 10 }
        )

        if resp.status ~= 200 then
            return {
                title = "Notifications",
                items = { github_http_error(resp.status) },
            }
        end

        local ok, notifs = pcall(lark.json.decode, resp.body)
        if not ok or type(notifs) ~= "table" then
            return {
                title = "Notifications",
                items = { error_item({ label = "Failed to parse response" }) },
            }
        end

        if #notifs == 0 then
            return {
                title = "Notifications",
                items = { { label = "All caught up!", icon = "✅" } },
            }
        end

        local items = {}
        for _, n in ipairs(notifs) do
            local subj = n.subject or {}
            local repo = n.repository and n.repository.full_name or ""
            local repo_short = repo:match("([^/]+)$") or repo
            local title = subj.title or "Notification"
            local stype = subj.type or "Unknown"
            local reason = reason_label(n.reason)

            -- Build a browser URL from the API URL.
            local web_url = ""
            if subj.url and subj.url:match("api.github.com/repos/") then
                web_url = subj.url:gsub("api.github.com/repos/", "github.com/")
                web_url = web_url:gsub("/pulls/", "/pull/")
            end

            local detail_parts = { repo_short, stype }
            if reason ~= "" then
                detail_parts[#detail_parts + 1] = reason
            end

            local actions = {}
            if web_url ~= "" then
                actions[#actions + 1] = { label = "Open in browser", kind = "open", args = { web_url } }
            end
            -- Mark as read via gh CLI.
            if n.id then
                actions[#actions + 1] = {
                    label = "Mark as read",
                    kind = "shell",
                    args = { "gh", "api", "--method", "PATCH", "/notifications/threads/" .. n.id },
                }
            end
            actions[#actions + 1] = { label = "Copy title", kind = "clipboard", args = { title } }

            items[#items + 1] = {
                label = title,
                detail = table.concat(detail_parts, " · "),
                icon = type_icon(stype),
                url = web_url ~= "" and web_url or nil,
                copy_text = title,
                actions = actions,
            }
        end

        return { title = "Notifications — " .. #items, items = items }
    end,
})
