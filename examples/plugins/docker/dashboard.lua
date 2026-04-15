-- Docker: Dashboard — mini app with two-pane split.
-- Left pane: container list. Right pane: detail for selected container.
-- Uses on_action to update the detail pane when a container is selected.
-- SHARED: check_docker() from lib.lua

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

local function status_icon(state)
    if state == "running" then return "●" end
    if state == "exited" then return "○" end
    if state == "paused" then return "◐" end
    if state == "restarting" then return "⟳" end
    return "◌"
end

local function get_containers()
    local raw = lark.exec("docker", {
        "ps", "-a", "--format", "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.State}}\t{{.Ports}}",
    })
    if not raw or raw == "" then return {} end

    local containers = {}
    for line in raw:gmatch("[^\n]+") do
        local id, name, image, status, state, ports = line:match("^([^\t]+)\t([^\t]+)\t([^\t]+)\t([^\t]+)\t([^\t]+)\t(.*)$")
        if id then
            containers[#containers + 1] = {
                id = id, name = name, image = image,
                status = status, state = state, ports = ports,
            }
        end
    end
    return containers
end

local function build_container_items(containers)
    local items = {}
    for _, c in ipairs(containers) do
        items[#items + 1] = {
            label = c.name,
            detail = c.image .. " · " .. c.status,
            icon = status_icon(c.state),
            copy_text = c.id,
            actions = {
                {
                    label = "View details",
                    kind = "chain",
                    args = { "show_detail", c.name },
                },
                {
                    label = "View logs",
                    kind = "chain",
                    args = { "show_logs", c.name },
                },
            },
        }
    end
    if #items == 0 then
        items[#items + 1] = { label = "No containers found", icon = "📭" }
    end
    return items
end

lark.register({
    on_run = function()
        local err = check_docker("Docker Dashboard")
        if err then return err end

        local containers = get_containers()
        local container_items = build_container_items(containers)

        -- Hint text for the detail pane.
        local detail_hint = "Select a container and press Enter to view details."
        if #containers > 0 then
            detail_hint = detail_hint .. "\n\nContainers: " .. #containers
            local running = 0
            for _, c in ipairs(containers) do
                if c.state == "running" then running = running + 1 end
            end
            detail_hint = detail_hint .. "  |  Running: " .. running
        end

        return {
            title = "Docker Dashboard",
            layout = {
                kind = "split",
                direction = "horizontal",
                children = {
                    {
                        size = 40,
                        layout = {
                            kind = "pane",
                            id = "containers",
                            content = {
                                title = "Containers",
                                items = container_items,
                            },
                        },
                    },
                    {
                        size = 60,
                        layout = {
                            kind = "pane",
                            id = "detail",
                            content = {
                                title = "Detail",
                                raw_text = detail_hint,
                            },
                        },
                    },
                },
            },
        }
    end,

    on_action = function(callback_id, context)
        if callback_id == "show_detail" then
            local name = context
            local raw = lark.exec("docker", { "inspect", "--format",
                "Name: {{.Name}}\nImage: {{.Config.Image}}\nCreated: {{.Created}}\nState: {{.State.Status}}\nPid: {{.State.Pid}}\nPorts: {{range $p, $c := .NetworkSettings.Ports}}{{$p}} {{end}}\nMounts: {{range .Mounts}}{{.Source}} -> {{.Destination}}\n{{end}}",
                name,
            })
            return {
                title = "detail",
                raw_text = raw or ("No details for " .. name),
            }
        end

        if callback_id == "show_logs" then
            local name = context
            local raw = lark.exec("docker", { "logs", "--tail", "40", name })
            return {
                title = "detail",
                raw_text = "── Logs: " .. name .. " ──\n\n" .. (raw or "No logs"),
            }
        end

        return { title = "detail", raw_text = "Unknown action: " .. callback_id }
    end,
})
