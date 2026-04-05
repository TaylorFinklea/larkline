-- Docker: Images — list local images with pull, remove, prune, and inspect.

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

        for line in raw:gmatch("[^\n]+") do
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
                actions[#actions + 1] = {
                    label = "Pull Latest",
                    kind = "shell",
                    args = { "docker", "pull", pull_target },
                }
            end

            actions[#actions + 1] = {
                label = "Inspect (JSON)",
                kind = "shell",
                args = { "docker", "inspect", id },
            }
            actions[#actions + 1] = {
                label = "History (layers)",
                kind = "shell",
                args = { "docker", "history", id },
            }
            actions[#actions + 1] = {
                label = "Remove",
                kind = "shell",
                args = { "docker", "rmi", id },
                confirm = true,
            }
            actions[#actions + 1] = {
                label = "Force Remove",
                kind = "shell",
                args = { "docker", "rmi", "-f", id },
                confirm = true,
            }
            actions[#actions + 1] = {
                label = "Copy Name",
                kind = "clipboard",
                args = { label },
            }
            actions[#actions + 1] = {
                label = "Copy ID",
                kind = "clipboard",
                args = { short_id },
            }

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
                    {
                        label = "Prune Dangling",
                        kind = "shell",
                        args = { "docker", "image", "prune", "-f" },
                        confirm = true,
                    },
                },
            }
        end

        items[#items + 1] = {
            label = "Prune all unused images",
            detail = "Remove all images not used by any container (including tagged)",
            icon = "🗑",
            actions = {
                {
                    label = "Prune All Unused",
                    kind = "shell",
                    args = { "docker", "image", "prune", "-af" },
                    confirm = true,
                },
            },
        }

        local title = "Images — " .. total_count
        if dangling > 0 then
            title = title .. " (" .. dangling .. " dangling)"
        end
        return { title = title, items = items }
    end,
})
