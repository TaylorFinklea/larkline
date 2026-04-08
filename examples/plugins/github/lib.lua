-- Shared helpers for GitHub plugin.
-- This file is NOT loaded by require(). Instead, each command file
-- copies the helpers it needs inline, since the Lark sandbox does not
-- expose require/dofile/loadfile. This file serves as the canonical
-- source — edit here, then sync to the command files.
--
-- SYNC INSTRUCTIONS:
-- When editing helpers here, copy the updated helper functions to each
-- command file that uses them: my-prs.lua, reviews.lua, issues.lua,
-- notifications.lua.
--
-- Helpers provided:
--   gh_headers(token)                  - build common GitHub API headers
--   github_token_or_error(title)       - fetch GITHUB_TOKEN or return an error payload

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
