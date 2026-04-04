-- View a project's roadmap.md — vision, milestones, backlog.

local lib = require("lib")

lark.register({
    on_run = function()
        local projects = lib.discover_projects()

        local items = {}
        for _, proj in ipairs(projects) do
            local content = lib.read_file(proj.ai_dir .. "/roadmap.md")
            local milestone = lib.extract_active_milestone(content)
            local vision = lib.extract_vision(content)
            local detail = milestone or vision or "no roadmap data"

            items[#items + 1] = {
                label = proj.name,
                detail = detail,
                icon = "🗺️",
                action = "view:" .. proj.ai_dir .. "/roadmap.md",
            }
        end

        return {
            title = "Roadmap — pick a project",
            items = items,
        }
    end,

    on_action = function(action)
        if not action then return end
        local path = action:match("^view:(.+)$")
        if not path then return end

        local content = lib.read_file(path)
        if not content then
            return { title = "Roadmap", items = {
                { label = "File not found", detail = path, icon = "⚠" },
            }}
        end

        -- Parse headings and checklist items into a navigable list.
        local items = {}
        local current_h2 = nil
        local current_h3 = nil

        for line in content:gmatch("[^\n]*") do
            if line:match("^##%s+") and not line:match("^###") then
                current_h2 = line:gsub("^##%s+", "")
                current_h3 = nil
                -- Skip common non-content headings.
                if not current_h2:match("^Constraints") and not current_h2:match("^Non%-Goals") then
                    items[#items + 1] = {
                        label = current_h2,
                        detail = "",
                        icon = "📋",
                    }
                end
            elseif line:match("^###%s+") then
                current_h3 = line:gsub("^###%s+", "")
                items[#items + 1] = {
                    label = current_h3,
                    detail = current_h2 or "",
                    icon = "📌",
                }
            elseif line:match("^%s*%-%s*%[x%]") then
                local text = line:gsub("^%s*%-%s*%[x%]%s*", "")
                items[#items + 1] = {
                    label = text,
                    detail = (current_h3 or current_h2 or "") .. "  ·  done",
                    icon = "☑️",
                }
            elseif line:match("^%s*%-%s*%[%s%]") then
                local text = line:gsub("^%s*%-%s*%[%s%]%s*", "")
                items[#items + 1] = {
                    label = text,
                    detail = current_h3 or current_h2 or "",
                    icon = "⬜",
                }
            elseif line:match("^%s*%-%s*%*%*") then
                -- Bold bullet: "- **Distribution:** crates.io ..."
                local text = line:gsub("^%s*%-%s*", "")
                items[#items + 1] = {
                    label = text:gsub("%*%*", ""),
                    detail = current_h3 or current_h2 or "",
                    icon = "📎",
                }
            end
        end

        items[#items + 1] = {
            label = "Edit in $EDITOR",
            detail = path,
            icon = "✏️",
            action = "shell:edit:" .. path,
        }

        local proj_name = path:match("/git/([^/]+)/")
        return {
            title = (proj_name or "Project") .. " — Roadmap",
            items = items,
        }
    end,
})
