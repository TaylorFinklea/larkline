-- Docker: Volumes — list volumes with inspect, remove, and prune actions.

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

        for line in raw:gmatch("[^\n]+") do
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
                        {
                            label = "Inspect",
                            kind = "shell",
                            args = { "docker", "volume", "inspect", name },
                        },
                        {
                            label = "Remove",
                            kind = "shell",
                            args = { "docker", "volume", "rm", name },
                            confirm = true,
                        },
                        {
                            label = "Copy Name",
                            kind = "clipboard",
                            args = { name },
                        },
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
                {
                    label = "Prune",
                    kind = "shell",
                    args = { "docker", "volume", "prune", "-f" },
                    confirm = true,
                },
            },
        }

        return { title = "Volumes — " .. (#items - 1), items = items }
    end,
})
