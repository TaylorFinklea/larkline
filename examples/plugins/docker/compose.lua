-- Docker: Compose — detect Compose projects and manage services.

lark.register({
    on_run = function()
        -- Check for docker compose (v2 plugin) or docker-compose (v1 standalone).
        local v2 = lark.exec("docker", { "compose", "version" })
        local use_v2 = v2 and v2:match("Docker Compose")

        if not use_v2 then
            local v1 = lark.exec("which", { "docker-compose" })
            if not v1 or not v1:match("docker%-compose") then
                return {
                    title = "Compose",
                    items = { { label = "Docker Compose not installed", icon = "!" } },
                }
            end
        end

        -- List running compose projects.
        local raw
        if use_v2 then
            raw = lark.exec("docker", {
                "compose", "ls", "--format", "table", "--all"
            })
        else
            raw = lark.exec("docker-compose", { "ls" })
        end

        if not raw or raw == "" then
            return {
                title = "Compose",
                items = { { label = "No Compose projects found", icon = "📭" } },
            }
        end

        local items = {}
        local first = true
        for line in raw:gmatch("[^\n]+") do
            -- Skip header row.
            if first then
                first = false
                goto next
            end

            -- Columns: NAME  STATUS  CONFIG FILES
            local name, status, config = line:match("^(%S+)%s+(%S+)%s+(.+)$")
            if name and type(name) == "string" and name ~= "" then
                local is_running = status and status:match("running")
                local icon = is_running and "▶" or "⏹"

                -- Trim config path for display.
                local detail = (status or "") .. "  " .. (config or "")
                if #detail > 80 then
                    detail = detail:sub(1, 77) .. "..."
                end

                local actions = {}
                if is_running then
                    if use_v2 then
                        actions[#actions + 1] = {
                            label = "Stop",
                            kind = "shell",
                            args = { "docker", "compose", "-p", name, "stop" },
                            confirm = true,
                        }
                        actions[#actions + 1] = {
                            label = "Restart",
                            kind = "shell",
                            args = { "docker", "compose", "-p", name, "restart" },
                            confirm = true,
                        }
                        actions[#actions + 1] = {
                            label = "Down",
                            kind = "shell",
                            args = { "docker", "compose", "-p", name, "down" },
                            confirm = true,
                        }
                        actions[#actions + 1] = {
                            label = "Logs (last 50)",
                            kind = "shell",
                            args = { "docker", "compose", "-p", name, "logs", "--tail", "50" },
                        }
                        actions[#actions + 1] = {
                            label = "PS (services)",
                            kind = "shell",
                            args = { "docker", "compose", "-p", name, "ps" },
                        }
                    else
                        actions[#actions + 1] = {
                            label = "Stop",
                            kind = "shell",
                            args = { "docker-compose", "-p", name, "stop" },
                            confirm = true,
                        }
                        actions[#actions + 1] = {
                            label = "Down",
                            kind = "shell",
                            args = { "docker-compose", "-p", name, "down" },
                            confirm = true,
                        }
                    end
                else
                    if use_v2 then
                        actions[#actions + 1] = {
                            label = "Up (detached)",
                            kind = "shell",
                            args = { "docker", "compose", "-p", name, "up", "-d" },
                        }
                    else
                        actions[#actions + 1] = {
                            label = "Up (detached)",
                            kind = "shell",
                            args = { "docker-compose", "-p", name, "up", "-d" },
                        }
                    end
                end

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
            ::next::
        end

        if #items == 0 then
            return {
                title = "Compose",
                items = { { label = "No Compose projects found", icon = "📭" } },
            }
        end

        return { title = "Compose — " .. #items .. " projects", items = items }
    end,
})
