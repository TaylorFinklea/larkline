-- Docker: Images — list local images with size.

lark.register({
    on_run = function()
        local raw = lark.exec("docker", { "images", "--format", "{{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.ID}}" })

        if not raw or raw == "" then
            return {
                title = "Images",
                items = { { label = "No images or Docker not running", icon = "📭" } },
            }
        end

        local items = {}
        for line in raw:gmatch("[^\n]+") do
            local repo, tag, size, id = line:match("^(.-)%\t(.-)%\t(.-)%\t(.-)$")
            if repo then
                local label = repo
                if tag and tag ~= "<none>" then
                    label = label .. ":" .. tag
                end

                items[#items + 1] = {
                    label = label,
                    detail = size .. "  " .. id:sub(1, 12),
                    icon = "📦",
                    copy_text = label,
                    actions = {
                        { label = "Remove image", kind = "shell", args = { "docker", "rmi", id }, confirm = true },
                        { label = "Copy name", kind = "clipboard", args = { label } },
                        { label = "Copy ID", kind = "clipboard", args = { id } },
                    },
                }
            end
        end

        return { title = "Images — " .. #items, items = items }
    end,
})
