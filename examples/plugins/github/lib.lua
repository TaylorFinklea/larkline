-- Shared helpers for GitHub plugin.
-- This file is NOT loaded by require(). Instead, each command file
-- copies the helpers it needs inline, since the Lark sandbox does not
-- expose require/dofile/loadfile. This file serves as the canonical
-- source — edit here, then sync to the command files.
--
-- SYNC INSTRUCTIONS:
-- When editing helpers here, copy the updated helper functions to each
-- command file that uses them: my-prs.lua, reviews.lua, issues.lua,
-- notifications.lua, workflows.lua.
--
-- Helpers provided:
--   error_item(opts)              - structured error row (see _shared/errors.lua)
--   gh_headers(token)             - build common GitHub API headers
--   github_token_or_error(title)  - fetch GITHUB_TOKEN or return an error payload

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

-- Build a friendly error item from a GitHub HTTP response.
-- Common: 401/403 (auth), 404 (missing), 429 (rate limit), 5xx (transient).
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
