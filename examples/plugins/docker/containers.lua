-- Docker: Containers — list all containers with start/stop/restart/logs/exec actions.

lark.register({
    on_run = function()
        local which = lark.exec("which", { "docker" })
        if not which or not which:match("docker") then
            return {
                title = "Containers",
                items = { { label = "Docker not installed", icon = "!" } },
            }
        end

        local raw = lark.exec("docker", {
            "ps", "-a", "--format",
            "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.State}}\t{{.Ports}}"
        })

        if not raw or raw == "" then
            return {
                title = "Containers",
                items = { { label = "No containers or Docker daemon not running", icon = "📭" } },
            }
        end

        local running = 0
        local stopped = 0
        local items = {}

        for line in raw:gmatch("[^\n]+") do
            local id, name, image, status, state, ports =
                line:match("^(.-)%\t(.-)%\t(.-)%\t(.-)%\t(.-)%\t(.*)$")
            if id and type(id) == "string" and id ~= "" then
                local is_running = state == "running"
                if is_running then running = running + 1 else stopped = stopped + 1 end

                local icon = is_running and "▶" or "⏹"
                local short_id = id:sub(1, 12)

                -- Build detail: image + ports if any.
                local detail = image .. "  " .. status
                if ports and ports ~= "" then
                    -- Shorten port mappings: "0.0.0.0:8080->80/tcp" → ":8080→80"
                    local short_ports = ports:gsub("0%.0%.0%.0:", ":"):gsub("%->", "→"):gsub("/tcp", "")
                    if #short_ports > 60 then
                        short_ports = short_ports:sub(1, 57) .. "..."
                    end
                    detail = detail .. "  " .. short_ports
                end

                local actions = {}
                if is_running then
                    actions[#actions + 1] = {
                        label = "View Logs (last 50)",
                        kind = "shell",
                        args = { "docker", "logs", "--tail", "50", id },
                    }
                    actions[#actions + 1] = {
                        label = "Follow Logs",
                        kind = "shell",
                        args = { "docker", "logs", "-f", "--tail", "20", id },
                    }
                    actions[#actions + 1] = {
                        label = "Shell (bash)",
                        kind = "shell",
                        args = { "docker", "exec", "-it", id, "bash" },
                    }
                    actions[#actions + 1] = {
                        label = "Shell (sh)",
                        kind = "shell",
                        args = { "docker", "exec", "-it", id, "sh" },
                    }
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
                else
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

                actions[#actions + 1] = {
                    label = "Inspect",
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

                items[#items + 1] = {
                    label = name,
                    detail = detail,
                    icon = icon,
                    copy_text = name,
                    actions = actions,
                }
            end
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
