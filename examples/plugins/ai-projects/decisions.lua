-- View a project's decisions.md — architecture decision records.

local lib = require("lib")

lark.register({
    on_run = function()
        local projects = lib.discover_projects()

        local items = {}
        for _, proj in ipairs(projects) do
            local content = lib.read_file(proj.ai_dir .. "/decisions.md")
            local count = lib.count_decisions(content)
            local detail = count .. " ADR" .. (count ~= 1 and "s" or "")

            items[#items + 1] = {
                label = proj.name,
                detail = detail,
                icon = "📝",
                action = "view:" .. proj.ai_dir .. "/decisions.md",
            }
        end

        return {
            title = "Decisions — pick a project",
            items = items,
        }
    end,

    on_action = function(action)
        if not action then return end
        local path = action:match("^view:(.+)$")
        if not path then return end

        local content = lib.read_file(path)
        if not content then
            return { title = "Decisions", items = {
                { label = "File not found", detail = path, icon = "⚠" },
            }}
        end

        -- Parse ADR entries: each starts with "## " after the first heading.
        local items = {}
        local in_entry = false
        local entry_title = nil
        local entry_lines = {}

        for line in content:gmatch("[^\n]*") do
            if line:match("^##%s+") then
                -- Flush previous entry.
                if in_entry and entry_title then
                    local summary = ""
                    for _, el in ipairs(entry_lines) do
                        local decision = el:match("^%*%*Decision%*%*:%s*(.+)")
                            or el:match("^%*%*Decision:%*%*%s*(.+)")
                        if decision then
                            summary = decision
                            break
                        end
                    end
                    if summary == "" and #entry_lines > 0 then
                        -- Use first non-empty line as fallback.
                        for _, el in ipairs(entry_lines) do
                            local stripped = el:gsub("^%s*", ""):gsub("^%*%*Context%*%*:%s*", "")
                            if stripped ~= "" and not stripped:match("^<!%-%-") then
                                summary = stripped
                                break
                            end
                        end
                    end
                    if #summary > 100 then summary = summary:sub(1, 97) .. "..." end
                    items[#items + 1] = {
                        label = entry_title,
                        detail = summary,
                        icon = "📝",
                    }
                end

                entry_title = line:gsub("^##%s+", "")
                entry_lines = {}
                in_entry = true
            elseif in_entry then
                if line ~= "" then
                    entry_lines[#entry_lines + 1] = line
                end
            end
        end

        -- Flush last entry.
        if in_entry and entry_title then
            local summary = ""
            for _, el in ipairs(entry_lines) do
                local decision = el:match("^%*%*Decision%*%*:%s*(.+)")
                    or el:match("^%*%*Decision:%*%*%s*(.+)")
                if decision then summary = decision; break end
            end
            if #summary > 100 then summary = summary:sub(1, 97) .. "..." end
            items[#items + 1] = {
                label = entry_title,
                detail = summary,
                icon = "📝",
            }
        end

        if #items == 0 then
            items[#items + 1] = {
                label = "No decisions recorded",
                detail = "Add entries with ## headings",
                icon = "📭",
            }
        end

        items[#items + 1] = {
            label = "Edit in $EDITOR",
            detail = path,
            icon = "✏️",
            action = "shell:edit:" .. path,
        }

        local proj_name = path:match("/git/([^/]+)/")
        return {
            title = (proj_name or "Project") .. " — Decisions",
            items = items,
        }
    end,
})
