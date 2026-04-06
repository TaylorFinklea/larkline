-- Docker: Volumes — list volumes with inspect, remove, and prune actions.
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
        local err = check_docker("Volumes")
        if err then return err end

        local raw = lark.exec("docker", {
            "volume", "ls", "--format",
            "{{.Name}}\t{{.Driver}}\t{{.Mountpoint}}"
        })

        if not raw or raw == "" then
            return {
                title = "Volumes",
                items = { { label = "No volumes found", icon = "📭" } },
            }
        end

        local items = {}

        for _, line in ipairs(split_lines(raw)) do
            local name, driver, mount = line:match("^(.-)%\t(.-)%\t(.-)$")
            if name and type(name) == "string" and name ~= "" then
                -- Truncate long volume names (compose generates hashes).
                local display = name
                if #display > 50 then
                    display = display:sub(1, 47) .. "..."
                end

                items[#items + 1] = {
                    label = display,
                    detail = driver .. "  " .. mount,
                    icon = "💾",
                    copy_text = name,
                    actions = {
                        docker_action("Inspect", { "volume", "inspect", name }),
                        docker_action("Remove", { "volume", "rm", name }, true),
                        clipboard_action("Copy Name", name),
                    },
                }
            end
        end

        -- Add prune action as a special item at the end.
        items[#items + 1] = {
            label = "Prune unused volumes",
            detail = "Remove all volumes not used by any container",
            icon = "🗑",
            actions = {
                docker_action("Prune", { "volume", "prune", "-f" }, true),
            },
        }

        return { title = "Volumes — " .. (#items - 1), items = items }
    end,
})
