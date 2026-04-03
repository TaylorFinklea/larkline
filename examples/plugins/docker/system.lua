-- Docker: System — disk usage, info, and system-wide prune.

lark.register({
    on_run = function()
        local which = lark.exec("which", { "docker" })
        if not which or not which:match("docker") then
            return {
                title = "System",
                items = { { label = "Docker not installed", icon = "!" } },
            }
        end

        local items = {}

        -- Docker version info.
        local version = lark.exec("docker", { "version", "--format",
            "Client: {{.Client.Version}}  Server: {{.Server.Version}}"
        })
        if version and version ~= "" then
            items[#items + 1] = {
                label = version:gsub("%s+$", ""),
                detail = "Docker Engine version",
                icon = "ℹ",
                copy_text = version:gsub("%s+$", ""),
            }
        end

        -- Disk usage summary.
        local df = lark.exec("docker", { "system", "df", "--format",
            "{{.Type}}\t{{.TotalCount}}\t{{.Size}}\t{{.Reclaimable}}"
        })
        if df and df ~= "" then
            for line in df:gmatch("[^\n]+") do
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
            items[#items + 1] = {
                label = info:gsub("%s+$", ""),
                detail = "Resource summary",
                icon = "📊",
                copy_text = info:gsub("%s+$", ""),
            }
        end

        -- Runtime info.
        local runtime = lark.exec("docker", { "info", "--format",
            "Runtime: {{.DefaultRuntime}}  OS: {{.OperatingSystem}}  Arch: {{.Architecture}}"
        })
        if runtime and runtime ~= "" then
            items[#items + 1] = {
                label = runtime:gsub("%s+$", ""),
                detail = "Docker daemon info",
                icon = "⚙",
                copy_text = runtime:gsub("%s+$", ""),
            }
        end

        -- Prune actions.
        items[#items + 1] = {
            label = "Prune: containers (stopped)",
            detail = "Remove all stopped containers",
            icon = "🗑",
            actions = {
                {
                    label = "Prune Stopped Containers",
                    kind = "shell",
                    args = { "docker", "container", "prune", "-f" },
                    confirm = true,
                },
            },
        }
        items[#items + 1] = {
            label = "Prune: images (dangling)",
            detail = "Remove untagged images",
            icon = "🗑",
            actions = {
                {
                    label = "Prune Dangling Images",
                    kind = "shell",
                    args = { "docker", "image", "prune", "-f" },
                    confirm = true,
                },
            },
        }
        items[#items + 1] = {
            label = "Prune: images (all unused)",
            detail = "Remove all images not used by any container",
            icon = "🗑",
            actions = {
                {
                    label = "Prune All Unused Images",
                    kind = "shell",
                    args = { "docker", "image", "prune", "-af" },
                    confirm = true,
                },
            },
        }
        items[#items + 1] = {
            label = "Prune: volumes (unused)",
            detail = "Remove all volumes not used by any container",
            icon = "🗑",
            actions = {
                {
                    label = "Prune Unused Volumes",
                    kind = "shell",
                    args = { "docker", "volume", "prune", "-f" },
                    confirm = true,
                },
            },
        }
        items[#items + 1] = {
            label = "Prune: networks (unused)",
            detail = "Remove all networks not used by any container",
            icon = "🗑",
            actions = {
                {
                    label = "Prune Unused Networks",
                    kind = "shell",
                    args = { "docker", "network", "prune", "-f" },
                    confirm = true,
                },
            },
        }
        items[#items + 1] = {
            label = "System Prune (everything)",
            detail = "Remove all stopped containers, unused networks, dangling images, and build cache",
            icon = "💣",
            actions = {
                {
                    label = "System Prune",
                    kind = "shell",
                    args = { "docker", "system", "prune", "-f" },
                    confirm = true,
                },
                {
                    label = "System Prune (include volumes)",
                    kind = "shell",
                    args = { "docker", "system", "prune", "-f", "--volumes" },
                    confirm = true,
                },
            },
        }

        return { title = "Docker System", items = items }
    end,
})
