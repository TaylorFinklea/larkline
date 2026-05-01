-- Shared helpers for Docker plugin.
-- This file is NOT loaded by require(). Instead, each command file
-- copies the helpers it needs inline, since the Lark sandbox does not
-- expose require/dofile/loadfile. This file serves as the canonical
-- source — edit here, then sync to the command files.
--
-- SYNC INSTRUCTIONS:
-- When editing helpers here, copy the updated helper functions to each
-- command file that uses them: containers.lua, compose.lua, images.lua,
-- volumes.lua, networks.lua, system.lua.
--
-- Helpers provided:
--   error_item(opts)                           - structured error row (see _shared/errors.lua)
--   from_exit(stderr, hints)                   - translate shell stderr to error_item or nil
--   check_docker(plugin_name)                  - return an error item if Docker is unavailable
--   trim(text)                                 - trim leading/trailing whitespace
--   split_lines(raw)                           - split command output into lines
--   shell_action(label, args, confirm_flag)    - build shell action tables
--   clipboard_action(label, value)             - build clipboard action tables
--   docker_action(label, docker_args, confirm) - build shell actions prefixed with `docker`
--   compose_action(...)                        - build Compose shell actions for v1/v2

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

-- SHARED: from_exit — canonical copy in examples/plugins/_shared/errors.lua.
local function from_exit(stderr, hints)
    hints = hints or {}
    stderr = stderr or ""
    local lower = stderr:lower()

    if lower:find("command not found", 1, true)
        or lower:find("no such file or directory", 1, true) then
        local cli = hints.cli or "command"
        local detail
        if hints.install_url then
            detail = "Install: " .. hints.install_url
        else
            detail = "Check your $PATH"
        end
        return error_item({
            label = cli .. " not found",
            detail = detail,
            help_url = hints.install_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("401", 1, true)
        or lower:find("403", 1, true)
        or lower:find("unauthorized", 1, true)
        or lower:find("forbidden", 1, true)
        or lower:find("not authenticated", 1, true)
        or lower:find("not logged in", 1, true)
        or lower:find("authentication required", 1, true) then
        local detail
        if hints.login_command then
            detail = "Run `" .. hints.login_command .. "`"
        else
            detail = "Check credentials"
        end
        return error_item({
            label = (hints.service or "Service") .. " auth failed",
            detail = detail,
            help_url = hints.login_help_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("429", 1, true)
        or lower:find("rate limit", 1, true)
        or lower:find("too many requests", 1, true) then
        local retry_after = stderr:match("[Rr]etry%-[Aa]fter:?%s*(%d+)")
        local detail
        if retry_after then
            detail = "Rate limited — retry in " .. retry_after .. "s"
        else
            detail = "Rate limited — try again later"
        end
        return error_item({
            label = (hints.service or "Service") .. " rate limited",
            detail = detail,
            help_url = hints.login_help_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("could not resolve host", 1, true)
        or lower:find("getaddrinfo", 1, true)
        or lower:find("name or service not known", 1, true)
        or lower:find("connection refused", 1, true)
        or lower:find("network is unreachable", 1, true)
        or lower:find("no route to host", 1, true) then
        return error_item({
            label = "Network unreachable",
            detail = "Check your connection",
            retry_action = hints.retry_action,
        })
    end

    return nil
end

-- Check if Docker is installed and return an error item if not.
-- Returns nil if Docker is available, or an error item table if not.
local function check_docker(plugin_name)
    local which = lark.exec("which", { "docker" })
    if not which or not which:match("docker") then
        return {
            title = plugin_name,
            items = { error_item({
                label = "Docker not installed",
                detail = "Install: https://docs.docker.com/get-docker/",
                help_url = "https://docs.docker.com/get-docker/",
            }) },
        }
    end
    return nil
end

local function trim(text)
    if not text then return nil end
    return text:gsub("^%s+", ""):gsub("%s+$", "")
end

local function split_lines(raw)
    local lines = {}
    if not raw or raw == "" then return lines end
    for line in raw:gmatch("[^\n]+") do
        lines[#lines + 1] = line
    end
    return lines
end

local function shell_action(label, args, confirm_flag)
    local action = {
        label = label,
        kind = "shell",
        args = args,
    }
    if confirm_flag then
        action.confirm = true
    end
    return action
end

local function clipboard_action(label, value)
    return {
        label = label,
        kind = "clipboard",
        args = { value },
    }
end

local function docker_action(label, docker_args, confirm_flag)
    local args = { "docker" }
    for _, arg in ipairs(docker_args) do
        args[#args + 1] = arg
    end
    return shell_action(label, args, confirm_flag)
end

local function compose_action(label, use_v2, project, subcmd, extra, confirm_flag)
    if use_v2 then
        local args = { "docker", "compose", "-p", project, subcmd }
        if extra then
            for _, arg in ipairs(extra) do
                args[#args + 1] = arg
            end
        end
        return shell_action(label, args, confirm_flag)
    end

    local args = { "docker-compose", "-p", project, subcmd }
    if extra then
        for _, arg in ipairs(extra) do
            args[#args + 1] = arg
        end
    end
    return shell_action(label, args, confirm_flag)
end
