-- Docker: Images — list local images with pull, remove, prune, and inspect.
-- Shared helpers copied from lib.lua.

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
        local err = check_docker("Images")
        if err then return err end

        local raw = lark.exec("docker", {
            "images", "--format",
            "{{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.ID}}\t{{.CreatedSince}}"
        })

        if not raw or raw == "" then
            return {
                title = "Images",
                items = { error_item({
                    label = "No images or Docker daemon not running",
                    detail = "Start Docker Desktop or run `docker info` to verify",
                    icon = "📭",
                    help_url = "https://docs.docker.com/config/daemon/start/",
                }) },
            }
        end

        local items = {}
        local dangling = 0
        local total_count = 0

        for _, line in ipairs(split_lines(raw)) do
            local repo, tag, size, id, created =
                line:match("^(.-)%\t(.-)%\t(.-)%\t(.-)%\t(.-)$")
            if not repo or type(repo) ~= "string" then goto next_image end

            total_count = total_count + 1
            local is_dangling = repo == "<none>"
            if is_dangling then dangling = dangling + 1 end

            local label = repo
            if tag and tag ~= "<none>" then
                label = label .. ":" .. tag
            end

            local short_id = id:sub(1, 12)
            local detail = size .. "  " .. short_id .. "  " .. (created or "")

            local icon = is_dangling and "👻" or "📦"

            local actions = {}

            if not is_dangling then
                local pull_target = repo
                if tag and tag ~= "<none>" then
                    pull_target = pull_target .. ":" .. tag
                end
                actions[#actions + 1] = docker_action("Pull Latest", { "pull", pull_target })
            end

            actions[#actions + 1] = docker_action("Inspect (JSON)", { "inspect", id })
            actions[#actions + 1] = docker_action("History (layers)", { "history", id })
            actions[#actions + 1] = docker_action("Remove", { "rmi", id }, true)
            actions[#actions + 1] = docker_action("Force Remove", { "rmi", "-f", id }, true)
            actions[#actions + 1] = clipboard_action("Copy Name", label)
            actions[#actions + 1] = clipboard_action("Copy ID", short_id)

            items[#items + 1] = {
                label = label,
                detail = detail,
                icon = icon,
                copy_text = label,
                actions = actions,
            }

            ::next_image::
        end

        -- Prune actions at the end.
        if dangling > 0 then
            items[#items + 1] = {
                label = "Prune dangling images (" .. dangling .. ")",
                detail = "Remove untagged images not used by any container",
                icon = "🗑",
                actions = {
                    docker_action("Prune Dangling", { "image", "prune", "-f" }, true),
                },
            }
        end

        items[#items + 1] = {
            label = "Prune all unused images",
            detail = "Remove all images not used by any container (including tagged)",
            icon = "🗑",
            actions = {
                docker_action("Prune All Unused", { "image", "prune", "-af" }, true),
            },
        }

        local title = "Images — " .. total_count
        if dangling > 0 then
            title = title .. " (" .. dangling .. " dangling)"
        end
        return { title = title, items = items }
    end,
})
