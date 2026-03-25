-- Git: Manage Repos — add or remove tracked repositories.

local function repo_name(path)
    return path:match("([^/]+)$") or path
end

lark.register({
    on_run = function()
        local repos = lark.store.get("repos") or {}

        -- Handle form submission: add a new repo.
        if lark.form_values and lark.form_values.path and lark.form_values.path ~= "" then
            local path = lark.form_values.path

            -- Expand ~ to HOME.
            local home = lark.env("HOME") or ""
            path = path:gsub("^~", home)

            -- Remove trailing slash.
            path = path:gsub("/$", "")

            -- Validate it's a git repo.
            local check = lark.exec("git", { "-C", path, "rev-parse", "--git-dir" })
            if not check or check == "" then
                return {
                    title = "Manage Repos",
                    items = { { label = "Not a git repo: " .. path, icon = "!" } },
                }
            end

            -- Deduplicate.
            for _, existing in ipairs(repos) do
                if existing == path then
                    return {
                        title = "Manage Repos",
                        items = { { label = "Already tracked: " .. repo_name(path), icon = "✓" } },
                    }
                end
            end

            repos[#repos + 1] = path
            table.sort(repos)
            lark.store.set("repos", repos)

            return {
                title = "Manage Repos",
                items = {
                    { label = "Added: " .. repo_name(path), detail = path, icon = "✅" },
                    { label = tostring(#repos) .. " repos tracked", icon = "📊" },
                },
            }
        end

        -- Handle remove action (passed via form_values.remove).
        if lark.form_values and lark.form_values.remove and lark.form_values.remove ~= "" then
            local remove_path = lark.form_values.remove
            local new_repos = {}
            for _, r in ipairs(repos) do
                if r ~= remove_path then
                    new_repos[#new_repos + 1] = r
                end
            end
            lark.store.set("repos", new_repos)
            repos = new_repos
        end

        -- Show current repos + add form.
        local items = {}

        if #repos == 0 then
            items[#items + 1] = {
                label = "No repos tracked",
                detail = "Add a path below or use Scan Directory",
                icon = "📭",
            }
        else
            for _, path in ipairs(repos) do
                items[#items + 1] = {
                    label = repo_name(path),
                    detail = path,
                    icon = "📁",
                    copy_text = path,
                    actions = {
                        { label = "Remove", kind = "clipboard", args = { "Removed " .. repo_name(path) } },
                        { label = "Copy path", kind = "clipboard", args = { path } },
                    },
                }
            end
            items[#items + 1] = {
                label = tostring(#repos) .. " repos tracked",
                icon = "📊",
            }
        end

        return {
            title = "Manage Repos",
            items = items,
            form = {
                fields = {
                    {
                        id = "path",
                        label = "Add repo path",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "/Users/tfinklea/git/my-project",
                    },
                },
                submit_label = "Add Repo",
            },
        }
    end,
})
