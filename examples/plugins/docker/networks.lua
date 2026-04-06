-- Docker: Networks — list networks with inspect, remove, and prune.
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

        for _, line in ipairs(split_lines(raw)) do
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

            local actions = { docker_action("Inspect (JSON)", { "network", "inspect", id }) }

            if #connected > 0 then
                local list = table.concat(connected, ", ")
                if #list > 60 then list = list:sub(1, 57) .. "..." end
                actions[#actions + 1] = clipboard_action("Connected: " .. list, table.concat(connected, "\n"))
            end

            if not is_builtin then
                actions[#actions + 1] = docker_action("Remove", { "network", "rm", id }, true)
            end

            actions[#actions + 1] = clipboard_action("Copy Name", name)
            actions[#actions + 1] = clipboard_action("Copy ID", short_id)

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
                docker_action("Prune", { "network", "prune", "-f" }, true),
            },
        }

        return { title = "Networks — " .. (#items - 1), items = items }
    end,
})
