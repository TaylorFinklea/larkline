-- Shared helpers for Git plugin.
-- This file is NOT loaded by require(). Instead, each command file
-- copies the helpers it needs inline, since the Lark sandbox does not
-- expose require/dofile/loadfile. This file serves as the canonical
-- source — edit here, then sync to the command files.
--
-- SYNC INSTRUCTIONS:
-- When editing helpers here, copy the updated helper functions to each
-- command file that uses them: status.lua, branches.lua, log.lua, stash.lua.
--
-- Helpers provided:
--   repo_name(path)   - return the repo basename for a path
--   is_git_repo(path) - check whether the path is a git repository

local function repo_name(path)
    return path:match("([^/]+)$") or path
end

local function is_git_repo(path)
    local check = lark.exec("git", { "-C", path, "rev-parse", "--git-dir" })
    return check and check ~= ""
end
