-- Docker: System — disk usage, info, and system-wide prune.
-- Shared helpers copied from lib.lua.

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
