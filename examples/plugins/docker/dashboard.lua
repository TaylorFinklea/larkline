-- Docker: Dashboard — mini app with two-pane split.
-- Left pane: container list. Right pane: detail for selected container.
-- Uses on_action to update the detail pane when a container is selected.

-- SHARED: error_item — canonical copy in examples/plugins/_shared/errors.lua.
local function error_item(opts)
    return {
        label = opts.label,
        detail = opts.detail,
        icon = opts.icon or "!",
        retry_action = opts.retry_action,
        help_url = opts.help_url,
    }
end

-- SHARED: from_exit — canonical copy in examples/plugins/_shared/errors.lua.
local function from_exit(stderr, hints)
    hints = hints or {}
    stderr = stderr or ""
    local lower = stderr:lower()

    if lower:find("command not found", 1, true)
        or lower:find("no such file or directory", 1, true) then
        local cli = hints.cli or "command"
        local detail
        if hints.install_url then
            detail = "Install: " .. hints.install_url
        else
            detail = "Check your $PATH"
        end
        return error_item({
            label = cli .. " not found",
            detail = detail,
            help_url = hints.install_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("401", 1, true)
        or lower:find("403", 1, true)
        or lower:find("unauthorized", 1, true)
        or lower:find("forbidden", 1, true)
        or lower:find("not authenticated", 1, true)
        or lower:find("not logged in", 1, true)
        or lower:find("authentication required", 1, true) then
        local detail
        if hints.login_command then
            detail = "Run `" .. hints.login_command .. "`"
        else
            detail = "Check credentials"
        end
        return error_item({
            label = (hints.service or "Service") .. " auth failed",
            detail = detail,
            help_url = hints.login_help_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("429", 1, true)
        or lower:find("rate limit", 1, true)
        or lower:find("too many requests", 1, true) then
        local retry_after = stderr:match("[Rr]etry%-[Aa]fter:?%s*(%d+)")
        local detail
        if retry_after then
            detail = "Rate limited — retry in " .. retry_after .. "s"
        else
            detail = "Rate limited — try again later"
        end
        return error_item({
            label = (hints.service or "Service") .. " rate limited",
            detail = detail,
            help_url = hints.login_help_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("could not resolve host", 1, true)
        or lower:find("getaddrinfo", 1, true)
        or lower:find("name or service not known", 1, true)
        or lower:find("connection refused", 1, true)
        or lower:find("network is unreachable", 1, true)
        or lower:find("no route to host", 1, true) then
        return error_item({
            label = "Network unreachable",
            detail = "Check your connection",
            retry_action = hints.retry_action,
        })
    end

    return nil
end

-- SHARED: check_docker() from lib.lua
local function check_docker(plugin_name)
    local which = lark.exec("which", { "docker" })
    if not which or not which:match("docker") then
        return {
            title = plugin_name,
            items = { error_item({
                label = "Docker not installed",
                detail = "Install: https://docs.docker.com/get-docker/",
                help_url = "https://docs.docker.com/get-docker/",
            }) },
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
        items[#items + 1] = error_item({
            label = "No containers found",
            detail = "Or Docker daemon may not be running",
            icon = "📭",
            help_url = "https://docs.docker.com/config/daemon/start/",
        })
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
