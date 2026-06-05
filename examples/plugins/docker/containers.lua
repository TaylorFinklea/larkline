-- Docker: Containers — full container management with logs, exec, stats, lifecycle.
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

local function check_docker(plugin_name)
    local which = lark.exec("which", { "docker" })
    if not which or not which:match("docker") then
        return {
            title = plugin_name,
            level = "warn",
            items = { error_item({
                label = "Docker not installed",
                detail = "Install: https://docs.docker.com/get-docker/",
                help_url = "https://docs.docker.com/get-docker/",
            }) },
        }
    end
    return nil
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

lark.register({
    on_run = function()
        local err = check_docker("Containers")
        if err then return err end

        -- Get container list with all details.
        local res = lark.exec_io("docker", {
            "ps", "-a", "--format",
            "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.State}}\t{{.Ports}}\t{{.Size}}"
        })

        if res.exit_code ~= 0 or res.stdout == "" then
            return {
                title = "Containers",
                level = "warn",
                items = { from_exit(res.stderr, {
                    cli = "docker",
                    install_url = "https://docs.docker.com/get-docker/",
                }) or error_item({
                    label = "No containers or Docker daemon not running",
                    detail = "Start Docker Desktop or run `docker info` to verify",
                    icon = "📭",
                    help_url = "https://docs.docker.com/config/daemon/start/",
                }) },
            }
        end
        local raw = res.stdout

        -- Get live stats for running containers (non-streaming).
        local stats_map = {}
        local stats_raw = lark.exec("docker", {
            "stats", "--no-stream", "--format",
            "{{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}"
        })
        if stats_raw and stats_raw ~= "" then
            for _, line in ipairs(split_lines(stats_raw)) do
                local sname, cpu, mem, net = line:match("^(.-)%\t(.-)%\t(.-)%\t(.-)$")
                if sname then
                    stats_map[sname] = { cpu = cpu, mem = mem, net = net }
                end
            end
        end

        local running = 0
        local stopped = 0
        local items = {}

        for _, line in ipairs(split_lines(raw)) do
            local id, name, image, status, state, ports, size =
                line:match("^(.-)%\t(.-)%\t(.-)%\t(.-)%\t(.-)%\t(.-)%\t(.*)$")
            if not id or type(id) ~= "string" or id == "" then goto next_container end

            local is_running = state == "running"
            if is_running then running = running + 1 else stopped = stopped + 1 end

            local short_id = id:sub(1, 12)

            -- Build rich detail line.
            local detail_parts = { image }

            -- Add stats for running containers.
            local st = stats_map[name]
            if st then
                detail_parts[#detail_parts + 1] = "CPU:" .. st.cpu
                detail_parts[#detail_parts + 1] = "Mem:" .. st.mem:gsub("%s+/.*", "")
            end

            -- Add port mappings (shortened).
            if ports and ports ~= "" then
                local short_ports = ports
                    :gsub("0%.0%.0%.0:", ":")
                    :gsub(":::", "[::]:")
                    :gsub("%->", "→")
                    :gsub("/tcp", "")
                    :gsub("/udp", "/u")
                if #short_ports > 40 then
                    short_ports = short_ports:sub(1, 37) .. "..."
                end
                detail_parts[#detail_parts + 1] = short_ports
            end

            -- Add status (e.g., "Up 3 hours", "Exited (0) 2 days ago").
            detail_parts[#detail_parts + 1] = status

            local detail = table.concat(detail_parts, "  ")

            local icon
            if is_running then
                icon = "▶"
            elseif status:match("Exited %(0%)") then
                icon = "⏹"
            else
                icon = "⚠"  -- non-zero exit
            end

            local actions = {}

            if is_running then
                -- Logs
                actions[#actions + 1] = docker_action("Logs (last 100)", { "logs", "--tail", "100", id })
                actions[#actions + 1] = docker_action("Follow Logs", { "logs", "-f", "--tail", "30", id })
                -- Exec
                actions[#actions + 1] = docker_action("Exec: bash", { "exec", "-it", id, "bash" })
                actions[#actions + 1] = docker_action("Exec: sh", { "exec", "-it", id, "sh" })
                -- Stats
                actions[#actions + 1] = docker_action("Live Stats", { "stats", id })
                -- Top (processes)
                actions[#actions + 1] = docker_action("Top (processes)", { "top", id })
                -- Lifecycle
                actions[#actions + 1] = docker_action("Stop", { "stop", id }, true)
                actions[#actions + 1] = docker_action("Restart", { "restart", id }, true)
                actions[#actions + 1] = docker_action("Kill", { "kill", id }, true)
                actions[#actions + 1] = docker_action("Pause", { "pause", id }, true)
            else
                -- Stopped container actions
                actions[#actions + 1] = docker_action("Logs (last 100)", { "logs", "--tail", "100", id })
                actions[#actions + 1] = docker_action("Start", { "start", id })
                actions[#actions + 1] = docker_action("Remove", { "rm", id }, true)
                actions[#actions + 1] = docker_action("Remove + Volumes", { "rm", "-v", id }, true)
            end

            -- Common actions
            actions[#actions + 1] = docker_action("Inspect (JSON)", { "inspect", id })
            actions[#actions + 1] = clipboard_action("Copy ID", short_id)
            actions[#actions + 1] = clipboard_action("Copy Name", name)
            actions[#actions + 1] = clipboard_action("Copy Image", image)

            items[#items + 1] = {
                label = name,
                detail = detail,
                icon = icon,
                copy_text = name,
                actions = actions,
            }

            ::next_container::
        end

        if #items == 0 then
            return {
                title = "Containers",
                items = { error_item({
                    label = "No containers found",
                    icon = "📭",
                    help_url = "https://docs.docker.com/engine/reference/commandline/docker/",
                }) },
            }
        end

        local title = "Containers — " .. running .. " running"
        if stopped > 0 then
            title = title .. ", " .. stopped .. " stopped"
        end
        return { title = title, items = items }
    end,
})
