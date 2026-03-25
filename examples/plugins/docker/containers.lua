-- Docker: Containers — list all containers with start/stop/restart actions.

lark.register({
    on_run = function()
        local raw = lark.exec("docker", { "ps", "-a", "--format", "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.State}}" })

        if not raw or raw == "" then
            return {
                title = "Containers",
                items = { { label = "No containers or Docker not running", icon = "📭" } },
            }
        end

        local items = {}
        for line in raw:gmatch("[^\n]+") do
            local id, name, image, status, state = line:match("^(.-)%\t(.-)%\t(.-)%\t(.-)%\t(.-)$")
            if id then
                local icon = state == "running" and "▶" or "⏹"
                local actions = {}

                if state == "running" then
                    actions[#actions + 1] = { label = "Stop", kind = "shell", args = { "docker", "stop", id }, confirm = true }
                    actions[#actions + 1] = { label = "Restart", kind = "shell", args = { "docker", "restart", id }, confirm = true }
                else
                    actions[#actions + 1] = { label = "Start", kind = "shell", args = { "docker", "start", id }, confirm = false }
                    actions[#actions + 1] = { label = "Remove", kind = "shell", args = { "docker", "rm", id }, confirm = true }
                end
                actions[#actions + 1] = { label = "Copy ID", kind = "clipboard", args = { id } }

                items[#items + 1] = {
                    label = name,
                    detail = image .. "  " .. status,
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

        return { title = "Containers — " .. #items, items = items }
    end,
})
