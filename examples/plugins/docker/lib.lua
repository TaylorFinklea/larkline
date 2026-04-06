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
--   check_docker(plugin_name)                  - return an error item if Docker is unavailable
--   trim(text)                                 - trim leading/trailing whitespace
--   split_lines(raw)                           - split command output into lines
--   shell_action(label, args, confirm_flag)    - build shell action tables
--   clipboard_action(label, value)             - build clipboard action tables
--   docker_action(label, docker_args, confirm) - build shell actions prefixed with `docker`
--   compose_action(...)                        - build Compose shell actions for v1/v2

-- Check if Docker is installed and return an error item if not.
-- Returns nil if Docker is available, or an error item table if not.
local function check_docker(plugin_name)
    local which = lark.exec("which", { "docker" })
    if not which or not which:match("docker") then
        return {
            title = plugin_name,
            items = { { label = "Docker not installed", icon = "!" } },
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
