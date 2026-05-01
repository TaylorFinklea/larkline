-- Docker: System — disk usage, info, and system-wide prune.
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

local function docker_action(label, docker_args, confirm_flag)
    local args = { "docker" }
    for _, arg in ipairs(docker_args) do
        args[#args + 1] = arg
    end
    return shell_action(label, args, confirm_flag)
end

lark.register({
    on_run = function()
        local err = check_docker("System")
        if err then return err end

        local items = {}

        -- Docker version info.
        local version = lark.exec("docker", { "version", "--format",
            "Client: {{.Client.Version}}  Server: {{.Server.Version}}"
        })
        if version and version ~= "" then
            local trimmed = trim(version)
            items[#items + 1] = {
                label = trimmed,
                detail = "Docker Engine version",
                icon = "ℹ",
                copy_text = trimmed,
            }
        end

        -- Disk usage summary.
        local df = lark.exec("docker", { "system", "df", "--format",
            "{{.Type}}\t{{.TotalCount}}\t{{.Size}}\t{{.Reclaimable}}"
        })
        if df and df ~= "" then
            for _, line in ipairs(split_lines(df)) do
                local rtype, count, size, reclaimable =
                    line:match("^(.-)%\t(.-)%\t(.-)%\t(.-)$")
                if rtype and type(rtype) == "string" then
                    local icon = "💾"
                    if rtype == "Images" then icon = "📦"
                    elseif rtype == "Containers" then icon = "📋"
                    elseif rtype == "Local Volumes" then icon = "💾"
                    elseif rtype == "Build Cache" then icon = "🔨"
                    end

                    items[#items + 1] = {
                        label = rtype .. ": " .. size,
                        detail = count .. " items  reclaimable: " .. reclaimable,
                        icon = icon,
                        copy_text = rtype .. ": " .. size,
                    }
                end
            end
        end

        -- Docker info (resource counts).
        local info = lark.exec("docker", { "info", "--format",
            "{{.Containers}} containers ({{.ContainersRunning}} running)  {{.Images}} images"
        })
        if info and info ~= "" then
            local trimmed = trim(info)
            items[#items + 1] = {
                label = trimmed,
                detail = "Resource summary",
                icon = "📊",
                copy_text = trimmed,
            }
        end

        -- Runtime info.
        local runtime = lark.exec("docker", { "info", "--format",
            "Runtime: {{.DefaultRuntime}}  OS: {{.OperatingSystem}}  Arch: {{.Architecture}}"
        })
        if runtime and runtime ~= "" then
            local trimmed = trim(runtime)
            items[#items + 1] = {
                label = trimmed,
                detail = "Docker daemon info",
                icon = "⚙",
                copy_text = trimmed,
            }
        end

        -- Prune actions.
        items[#items + 1] = {
            label = "Prune: containers (stopped)",
            detail = "Remove all stopped containers",
            icon = "🗑",
            actions = {
                docker_action("Prune Stopped Containers", { "container", "prune", "-f" }, true),
            },
        }
        items[#items + 1] = {
            label = "Prune: images (dangling)",
            detail = "Remove untagged images",
            icon = "🗑",
            actions = {
                docker_action("Prune Dangling Images", { "image", "prune", "-f" }, true),
            },
        }
        items[#items + 1] = {
            label = "Prune: images (all unused)",
            detail = "Remove all images not used by any container",
            icon = "🗑",
            actions = {
                docker_action("Prune All Unused Images", { "image", "prune", "-af" }, true),
            },
        }
        items[#items + 1] = {
            label = "Prune: volumes (unused)",
            detail = "Remove all volumes not used by any container",
            icon = "🗑",
            actions = {
                docker_action("Prune Unused Volumes", { "volume", "prune", "-f" }, true),
            },
        }
        items[#items + 1] = {
            label = "Prune: networks (unused)",
            detail = "Remove all networks not used by any container",
            icon = "🗑",
            actions = {
                docker_action("Prune Unused Networks", { "network", "prune", "-f" }, true),
            },
        }
        items[#items + 1] = {
            label = "System Prune (everything)",
            detail = "Remove all stopped containers, unused networks, dangling images, and build cache",
            icon = "💣",
            actions = {
                docker_action("System Prune", { "system", "prune", "-f" }, true),
                docker_action("System Prune (include volumes)", { "system", "prune", "-f", "--volumes" }, true),
            },
        }

        return { title = "Docker System", items = items }
    end,
})
