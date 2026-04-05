-- Docker: Networks — list networks with inspect, remove, and prune.

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
        local err = check_docker("Networks")
        if err then return err end

        local raw = lark.exec("docker", {
            "network", "ls", "--format",
            "{{.ID}}\t{{.Name}}\t{{.Driver}}\t{{.Scope}}"
        })

        if not raw or raw == "" then
            return {
                title = "Networks",
                items = { { label = "No networks found", icon = "📭" } },
            }
        end

        local items = {}
        local builtin = { bridge = true, host = true, none = true }

        for line in raw:gmatch("[^\n]+") do
            local id, name, driver, scope = line:match("^(.-)%\t(.-)%\t(.-)%\t(.-)$")
            if not id or type(id) ~= "string" or id == "" then goto next_net end

            local short_id = id:sub(1, 12)
            local is_builtin = builtin[name] or false
            local icon = is_builtin and "🔒" or "🌐"
            local detail = driver .. "  " .. scope .. "  " .. short_id

            -- Count connected containers.
            local inspect_raw = lark.exec("docker", {
                "network", "inspect", id, "--format", "{{range .Containers}}{{.Name}} {{end}}"
            })
            local connected = {}
            if inspect_raw and inspect_raw ~= "" then
                for cname in inspect_raw:gmatch("%S+") do
                    connected[#connected + 1] = cname
                end
            end
            if #connected > 0 then
                detail = detail .. "  " .. #connected .. " containers"
            end

            local actions = {
                {
                    label = "Inspect (JSON)",
                    kind = "shell",
                    args = { "docker", "network", "inspect", id },
                },
            }

            if #connected > 0 then
                local list = table.concat(connected, ", ")
                if #list > 60 then list = list:sub(1, 57) .. "..." end
                actions[#actions + 1] = {
                    label = "Connected: " .. list,
                    kind = "clipboard",
                    args = { table.concat(connected, "\n") },
                }
            end

            if not is_builtin then
                actions[#actions + 1] = {
                    label = "Remove",
                    kind = "shell",
                    args = { "docker", "network", "rm", id },
                    confirm = true,
                }
            end

            actions[#actions + 1] = {
                label = "Copy Name",
                kind = "clipboard",
                args = { name },
            }
            actions[#actions + 1] = {
                label = "Copy ID",
                kind = "clipboard",
                args = { short_id },
            }

            items[#items + 1] = {
                label = name,
                detail = detail,
                icon = icon,
                copy_text = name,
                actions = actions,
            }

            ::next_net::
        end

        -- Prune action.
        items[#items + 1] = {
            label = "Prune unused networks",
            detail = "Remove all networks not used by any container",
            icon = "🗑",
            actions = {
                {
                    label = "Prune",
                    kind = "shell",
                    args = { "docker", "network", "prune", "-f" },
                    confirm = true,
                },
            },
        }

        return { title = "Networks — " .. (#items - 1), items = items }
    end,
})
