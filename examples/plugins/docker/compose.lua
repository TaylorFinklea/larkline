-- Docker: Compose — manage Compose stacks with logs, services, lifecycle.
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

-- SHARED: trim (canonical copy in lib.lua)
local function trim(text)
    if not text then return nil end
    return text:gsub("^%s+", ""):gsub("%s+$", "")
end

-- SHARED: split_lines (canonical copy in lib.lua)
local function split_lines(raw)
    local lines = {}
    if not raw or raw == "" then return lines end
    for line in raw:gmatch("[^\n]+") do
        lines[#lines + 1] = line
    end
    return lines
end

-- SHARED: shell_action (canonical copy in lib.lua)
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

-- SHARED: clipboard_action (canonical copy in lib.lua)
local function clipboard_action(label, value)
    return {
        label = label,
        kind = "clipboard",
        args = { value },
    }
end

-- SHARED: compose_action (canonical copy in lib.lua)
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

lark.register({
    on_run = function()
        -- Detect docker compose v2 (plugin) vs v1 (standalone).
        local v2 = lark.exec("docker", { "compose", "version" })
        local use_v2 = v2 and v2:match("Docker Compose")

        if not use_v2 then
            local v1 = lark.exec("which", { "docker-compose" })
            if not v1 or not v1:match("docker%-compose") then
                return {
                    title = "Compose",
                    items = { error_item({
                        label = "Docker Compose not installed",
                        detail = "Install: https://docs.docker.com/compose/install/",
                        help_url = "https://docs.docker.com/compose/",
                    }) },
                }
            end
        end

        -- List all compose projects.
        local raw
        if use_v2 then
            raw = lark.exec("docker", { "compose", "ls", "--format", "table", "--all" })
        else
            raw = lark.exec("docker-compose", { "ls" })
        end

        if not raw or raw == "" then
            return {
                title = "Compose",
                items = { error_item({
                    label = "No Compose projects found",
                    detail = "Run `docker compose up` in a project directory first",
                    icon = "📭",
                    help_url = "https://docs.docker.com/compose/",
                }) },
            }
        end

        local items = {}
        local first = true
        for _, line in ipairs(split_lines(raw)) do
            if first then first = false goto next end

            local name, status, config = line:match("^(%S+)%s+(%S+)%s+(.+)$")
            if not name or type(name) ~= "string" or name == "" then goto next end

            local is_running = status and status:match("running")
            local icon = is_running and "▶" or "⏹"

            -- Count services.
            local svc_count = 0
            if use_v2 then
                local ps = lark.exec("docker", {
                    "compose", "-p", name, "ps", "--format", "{{.Name}}"
                })
                if ps then
                    for _ in ipairs(split_lines(ps)) do svc_count = svc_count + 1 end
                end
            end

            local detail = (status or "")
            if svc_count > 0 then
                detail = detail .. "  " .. svc_count .. " services"
            end
            if config then
                local short = trim(config)
                if #short > 50 then short = "…" .. short:sub(-49) end
                detail = detail .. "  " .. short
            end

            local actions = {}

            if is_running then
                actions[#actions + 1] = compose_action("Logs (last 100)", use_v2, name, "logs", { "--tail", "100" })
                actions[#actions + 1] = compose_action("Follow Logs", use_v2, name, "logs", { "-f", "--tail", "30" })
                actions[#actions + 1] = compose_action("Services (ps)", use_v2, name, "ps")
                actions[#actions + 1] = compose_action("Stop", use_v2, name, "stop", nil, true)
                actions[#actions + 1] = compose_action("Restart", use_v2, name, "restart", nil, true)
                actions[#actions + 1] = compose_action("Down", use_v2, name, "down", nil, true)
                actions[#actions + 1] = compose_action("Down + Remove Volumes", use_v2, name, "down", { "-v" }, true)
                actions[#actions + 1] = compose_action("Pull Latest Images", use_v2, name, "pull")
            else
                actions[#actions + 1] = compose_action("Up (detached)", use_v2, name, "up", { "-d" })
                actions[#actions + 1] = compose_action("Pull Latest Images", use_v2, name, "pull")
            end

            actions[#actions + 1] = clipboard_action("Copy Name", name)

            items[#items + 1] = {
                label = name,
                detail = detail,
                icon = icon,
                copy_text = name,
                actions = actions,
            }

            ::next::
        end

        if #items == 0 then
            return {
                title = "Compose",
                items = { error_item({
                    label = "No Compose projects found",
                    detail = "Run `docker compose up` in a project directory first",
                    icon = "📭",
                    help_url = "https://docs.docker.com/compose/",
                }) },
            }
        end

        return { title = "Compose — " .. #items .. " stacks", items = items }
    end,
})
