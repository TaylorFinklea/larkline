-- Docker: Containers — full container management with logs, exec, stats, lifecycle.

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

lark.register({
    on_run = function()
        local err = check_docker("Containers")
        if err then return err end

        -- Get container list with all details.
        local raw = lark.exec("docker", {
            "ps", "-a", "--format",
            "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.State}}\t{{.Ports}}\t{{.Size}}"
        })

        if not raw or raw == "" then
            return {
                title = "Containers",
                items = { { label = "No containers or Docker daemon not running", icon = "📭" } },
            }
        end

        -- Get live stats for running containers (non-streaming).
        local stats_map = {}
        local stats_raw = lark.exec("docker", {
            "stats", "--no-stream", "--format",
            "{{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}"
        })
        if stats_raw and stats_raw ~= "" then
            for line in stats_raw:gmatch("[^\n]+") do
                local sname, cpu, mem, net = line:match("^(.-)%\t(.-)%\t(.-)%\t(.-)$")
                if sname then
                    stats_map[sname] = { cpu = cpu, mem = mem, net = net }
                end
            end
        end

        local running = 0
        local stopped = 0
        local items = {}

        for line in raw:gmatch("[^\n]+") do
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
                actions[#actions + 1] = {
                    label = "Logs (last 100)",
                    kind = "shell",
                    args = { "docker", "logs", "--tail", "100", id },
                }
                actions[#actions + 1] = {
                    label = "Follow Logs",
                    kind = "shell",
                    args = { "docker", "logs", "-f", "--tail", "30", id },
                }
                -- Exec
                actions[#actions + 1] = {
                    label = "Exec: bash",
                    kind = "shell",
                    args = { "docker", "exec", "-it", id, "bash" },
                }
                actions[#actions + 1] = {
                    label = "Exec: sh",
                    kind = "shell",
                    args = { "docker", "exec", "-it", id, "sh" },
                }
                -- Stats
                actions[#actions + 1] = {
                    label = "Live Stats",
                    kind = "shell",
                    args = { "docker", "stats", id },
                }
                -- Top (processes)
                actions[#actions + 1] = {
                    label = "Top (processes)",
                    kind = "shell",
                    args = { "docker", "top", id },
                }
                -- Lifecycle
                actions[#actions + 1] = {
                    label = "Stop",
                    kind = "shell",
                    args = { "docker", "stop", id },
                    confirm = true,
                }
                actions[#actions + 1] = {
                    label = "Restart",
                    kind = "shell",
                    args = { "docker", "restart", id },
                    confirm = true,
                }
                actions[#actions + 1] = {
                    label = "Kill",
                    kind = "shell",
                    args = { "docker", "kill", id },
                    confirm = true,
                }
                actions[#actions + 1] = {
                    label = "Pause",
                    kind = "shell",
                    args = { "docker", "pause", id },
                    confirm = true,
                }
            else
                -- Stopped container actions
                actions[#actions + 1] = {
                    label = "Logs (last 100)",
                    kind = "shell",
                    args = { "docker", "logs", "--tail", "100", id },
                }
                actions[#actions + 1] = {
                    label = "Start",
                    kind = "shell",
                    args = { "docker", "start", id },
                }
                actions[#actions + 1] = {
                    label = "Remove",
                    kind = "shell",
                    args = { "docker", "rm", id },
                    confirm = true,
                }
                actions[#actions + 1] = {
                    label = "Remove + Volumes",
                    kind = "shell",
                    args = { "docker", "rm", "-v", id },
                    confirm = true,
                }
            end

            -- Common actions
            actions[#actions + 1] = {
                label = "Inspect (JSON)",
                kind = "shell",
                args = { "docker", "inspect", id },
            }
            actions[#actions + 1] = {
                label = "Copy ID",
                kind = "clipboard",
                args = { short_id },
            }
            actions[#actions + 1] = {
                label = "Copy Name",
                kind = "clipboard",
                args = { name },
            }
            actions[#actions + 1] = {
                label = "Copy Image",
                kind = "clipboard",
                args = { image },
            }

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
                items = { { label = "No containers found", icon = "📭" } },
            }
        end

        local title = "Containers — " .. running .. " running"
        if stopped > 0 then
            title = title .. ", " .. stopped .. " stopped"
        end
        return { title = title, items = items }
    end,
})
