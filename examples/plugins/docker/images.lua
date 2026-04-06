-- Docker: Images — list local images with pull, remove, prune, and inspect.
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
        local err = check_docker("Images")
        if err then return err end

        local raw = lark.exec("docker", {
            "images", "--format",
            "{{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.ID}}\t{{.CreatedSince}}"
        })

        if not raw or raw == "" then
            return {
                title = "Images",
                items = { { label = "No images or Docker daemon not running", icon = "📭" } },
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
