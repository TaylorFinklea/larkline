-- Docker: Compose — manage Compose stacks with logs, services, lifecycle.

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
                    items = { { label = "Docker Compose not installed", icon = "!" } },
                }
            end
        end

        -- Helper: build a shell action for a compose command.
        local function action(label, project, subcmd, extra, confirm_flag)
            if use_v2 then
                local args = { "compose", "-p", project, subcmd }
                if extra then
                    for _, e in ipairs(extra) do args[#args + 1] = e end
                end
                return { label = label, kind = "shell", args = { "docker", args }, confirm = confirm_flag }
            else
                local args = { "-p", project, subcmd }
                if extra then
                    for _, e in ipairs(extra) do args[#args + 1] = e end
                end
                return { label = label, kind = "shell", args = { "docker-compose", args }, confirm = confirm_flag }
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
                items = { { label = "No Compose projects found", icon = "📭" } },
            }
        end

        local items = {}
        local first = true
        for line in raw:gmatch("[^\n]+") do
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
                    for _ in ps:gmatch("[^\n]+") do svc_count = svc_count + 1 end
                end
            end

            local detail = (status or "")
            if svc_count > 0 then
                detail = detail .. "  " .. svc_count .. " services"
            end
            if config then
                local short = config:gsub("^%s+", ""):gsub("%s+$", "")
                if #short > 50 then short = "…" .. short:sub(-49) end
                detail = detail .. "  " .. short
            end

            local actions = {}

            if is_running then
                actions[#actions + 1] = action("Logs (last 100)", name, "logs", { "--tail", "100" })
                actions[#actions + 1] = action("Follow Logs", name, "logs", { "-f", "--tail", "30" })
                actions[#actions + 1] = action("Services (ps)", name, "ps")
                actions[#actions + 1] = action("Stop", name, "stop", nil, true)
                actions[#actions + 1] = action("Restart", name, "restart", nil, true)
                actions[#actions + 1] = action("Down", name, "down", nil, true)
                actions[#actions + 1] = action("Down + Remove Volumes", name, "down", { "-v" }, true)
                actions[#actions + 1] = action("Pull Latest Images", name, "pull")
            else
                actions[#actions + 1] = action("Up (detached)", name, "up", { "-d" })
                actions[#actions + 1] = action("Pull Latest Images", name, "pull")
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

            ::next::
        end

        if #items == 0 then
            return {
                title = "Compose",
                items = { { label = "No Compose projects found", icon = "📭" } },
            }
        end

        return { title = "Compose — " .. #items .. " stacks", items = items }
    end,
})
