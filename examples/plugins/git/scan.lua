-- Git: Scan Directory — auto-discover git repos in a directory.

local function repo_name(path)
    return path:match("([^/]+)$") or path
end

lark.register({
    on_run = function()
        local repos = lark.store.get("repos") or {}

        if not lark.form_values or not lark.form_values.directory or lark.form_values.directory == "" then
            return {
                title = "Scan Directory",
                form = {
                    fields = {
                        {
                            id = "directory",
                            label = "Directory to scan",
                            type = { kind = "text" },
                            required = true,
                            placeholder = "~/git",
                        },
                    },
                    submit_label = "Scan",
                },
            }
        end

        local dir = lark.form_values.directory
        local home = lark.env("HOME") or ""
        dir = dir:gsub("^~", home)
        dir = dir:gsub("/$", "")

        -- Find git repos (look for .git directories up to 2 levels deep).
        local raw = lark.exec("find", { dir, "-maxdepth", "2", "-name", ".git", "-type", "d" })

        if not raw or raw == "" then
            return {
                title = "Scan Directory",
                items = { { label = "No git repos found in " .. dir, icon = "📭" } },
            }
        end

        -- Build a set of already-tracked repos for quick lookup.
        local tracked = {}
        for _, r in ipairs(repos) do tracked[r] = true end

        local discovered = {}
        for line in raw:gmatch("[^\n]+") do
            -- Strip /.git suffix to get repo root.
            local repo_path = line:gsub("/.git$", "")
            if repo_path ~= "" then
                discovered[#discovered + 1] = repo_path
            end
        end

        table.sort(discovered)

        if #discovered == 0 then
            return {
                title = "Scan Directory",
                items = { { label = "No git repos found in " .. dir, icon = "📭" } },
            }
        end

        local items = {}
        local new_count = 0

        for _, path in ipairs(discovered) do
            local already = tracked[path]
            if not already then new_count = new_count + 1 end

            items[#items + 1] = {
                label = (already and "✓ " or "  ") .. repo_name(path),
                detail = already and "already tracked" or path,
                icon = already and "✓" or "📁",
                copy_text = path,
            }
        end

        -- Add all untracked repos at once.
        if new_count > 0 then
            -- Build the combined list.
            local combined = {}
            for _, r in ipairs(repos) do combined[#combined + 1] = r end
            local combined_set = {}
            for _, r in ipairs(combined) do combined_set[r] = true end
            for _, path in ipairs(discovered) do
                if not combined_set[path] then
                    combined[#combined + 1] = path
                end
            end
            table.sort(combined)
            lark.store.set("repos", combined)

            table.insert(items, 1, {
                label = "Added " .. new_count .. " new repos (" .. #combined .. " total)",
                icon = "✅",
            })
        else
            table.insert(items, 1, {
                label = "All " .. #discovered .. " repos already tracked",
                icon = "✓",
            })
        end

        return { title = "Scan: " .. dir .. " — " .. #discovered .. " repos", items = items }
    end,
})
