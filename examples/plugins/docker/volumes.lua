-- Docker: Volumes — list volumes with inspect, remove, and prune actions.
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
        local err = check_docker("Volumes")
        if err then return err end

        local res = lark.exec_io("docker", {
            "volume", "ls", "--format",
            "{{.Name}}\t{{.Driver}}\t{{.Mountpoint}}"
        })

        if res.exit_code ~= 0 or res.stdout == "" then
            return {
                title = "Volumes",
                level = "warn",
                items = { from_exit(res.stderr, {
                    cli = "docker",
                    install_url = "https://docs.docker.com/get-docker/",
                }) or error_item({
                    label = "No volumes found",
                    detail = "Or Docker daemon may not be running",
                    icon = "📭",
                    help_url = "https://docs.docker.com/engine/reference/commandline/docker/",
                }) },
            }
        end
        local raw = res.stdout

        local items = {}

        for _, line in ipairs(split_lines(raw)) do
            local name, driver, mount = line:match("^(.-)%\t(.-)%\t(.-)$")
            if name and type(name) == "string" and name ~= "" then
                -- Truncate long volume names (compose generates hashes).
                local display = name
                if #display > 50 then
                    display = display:sub(1, 47) .. "..."
                end

                items[#items + 1] = {
                    label = display,
                    detail = driver .. "  " .. mount,
                    icon = "💾",
                    copy_text = name,
                    actions = {
                        docker_action("Inspect", { "volume", "inspect", name }),
                        docker_action("Remove", { "volume", "rm", name }, true),
                        clipboard_action("Copy Name", name),
                    },
                }
            end
        end

        -- Add prune action as a special item at the end.
        items[#items + 1] = {
            label = "Prune unused volumes",
            detail = "Remove all volumes not used by any container",
            icon = "🗑",
            actions = {
                docker_action("Prune", { "volume", "prune", "-f" }, true),
            },
        }

        return { title = "Volumes — " .. (#items - 1), items = items }
    end,
})
