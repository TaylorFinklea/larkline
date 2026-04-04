-- View a project's current-state.md with structured output.

local lib = require("lib")

lark.register({
    on_run = function()
        local projects = lib.discover_projects()

        -- If launched from dashboard action, args has the path.
        -- Otherwise show project picker.
        local items = {}
        for _, proj in ipairs(projects) do
            local content = lib.read_file(proj.ai_dir .. "/current-state.md")
            local date = lib.extract_date(content)
            local branch = lib.extract_branch(content)
            local icon, _ = lib.recency(date)

            items[#items + 1] = {
                label = proj.name,
                detail = branch .. "  ·  " .. lib.short_date(date),
                icon = icon,
                action = "view:" .. proj.ai_dir .. "/current-state.md",
            }
        end

        return {
            title = "Current State — pick a project",
            items = items,
        }
    end,

    on_action = function(action)
        if not action then return end
        local path = action:match("^view:(.+)$")
        if not path then return end

        local content = lib.read_file(path)
        if not content then
            return { title = "Current State", items = {
                { label = "File not found", detail = path, icon = "⚠" },
            }}
        end

        -- Parse sections into items.
        local items = {}
        local current_section = nil

        for line in content:gmatch("[^\n]*") do
            if line:match("^##%s+") then
                current_section = line:gsub("^##%s+", "")
            elseif line:match("^%s*$") then
                -- skip blank lines
            elseif line:match("^>") then
                -- skip blockquotes (metadata lines)
            elseif line:match("^#%s+") then
                -- skip h1 title
            else
                local cleaned = line:gsub("^%s*%-%s*", ""):gsub("^%*%*(.-)%*%*", "%1")
                if cleaned ~= "" then
                    local icon = "📄"
                    if current_section and current_section:match("[Bb]ranch") then icon = "🌿"
                    elseif current_section and current_section:match("[Pp]rogress") then icon = "📈"
                    elseif current_section and current_section:match("[Vv]alidation") then icon = "✅"
                    elseif current_section and current_section:match("[Bb]locker") then icon = "🚧"
                    elseif current_section and current_section:match("[Vv]ersion") then icon = "🏷️"
                    end

                    items[#items + 1] = {
                        label = cleaned,
                        detail = current_section or "",
                        icon = icon,
                    }
                end
            end
        end

        -- Add edit action at the bottom.
        items[#items + 1] = {
            label = "Edit in $EDITOR",
            detail = path,
            icon = "✏️",
            action = "shell:edit:" .. path,
        }

        -- Extract project name from path for title.
        local proj_name = path:match("/git/([^/]+)/")
        return {
            title = (proj_name or "Project") .. " — Current State",
            items = items,
        }
    end,
})
