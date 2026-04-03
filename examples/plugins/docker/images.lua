-- Docker: Images — list local images with remove, inspect, and pull actions.

lark.register({
    on_run = function()
        local which = lark.exec("which", { "docker" })
        if not which or not which:match("docker") then
            return {
                title = "Images",
                items = { { label = "Docker not installed", icon = "!" } },
            }
        end

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

        for line in raw:gmatch("[^\n]+") do
            local repo, tag, size, id, created =
                line:match("^(.-)%\t(.-)%\t(.-)%\t(.-)%\t(.-)$")
            if repo and type(repo) == "string" then
                local is_dangling = repo == "<none>"
                if is_dangling then
                    dangling = dangling + 1
                end

                local label = repo
                if tag and tag ~= "<none>" then
                    label = label .. ":" .. tag
                end

                local short_id = id:sub(1, 12)
                local detail = size .. "  " .. short_id .. "  " .. (created or "")

                local icon = is_dangling and "👻" or "📦"

                local actions = {
                    {
                        label = "Inspect",
                        kind = "shell",
                        args = { "docker", "inspect", id },
                    },
                    {
                        label = "Remove",
                        kind = "shell",
                        args = { "docker", "rmi", id },
                        confirm = true,
                    },
                    {
                        label = "Force Remove",
                        kind = "shell",
                        args = { "docker", "rmi", "-f", id },
                        confirm = true,
                    },
                }

                if not is_dangling then
                    -- Insert pull at the top for named images.
                    local pull_target = repo
                    if tag and tag ~= "<none>" then
                        pull_target = pull_target .. ":" .. tag
                    end
                    table.insert(actions, 1, {
                        label = "Pull Latest",
                        kind = "shell",
                        args = { "docker", "pull", pull_target },
                    })
                end

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
            end
        end

        local title = "Images — " .. #items
        if dangling > 0 then
            title = title .. " (" .. dangling .. " dangling)"
        end
        return { title = title, items = items }
    end,
})
