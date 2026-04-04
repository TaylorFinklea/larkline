-- View a project's next-steps.md as a structured checklist.

local lib = require("lib")

lark.register({
    on_run = function()
        local projects = lib.discover_projects()

        local items = {}
        for _, proj in ipairs(projects) do
            local content = lib.read_file(proj.ai_dir .. "/next-steps.md")
            local open = lib.count_open_items(content)
            local done = lib.count_done_items(content)
            local detail = open .. " open"
            if done > 0 then detail = detail .. ", " .. done .. " done" end

            items[#items + 1] = {
                label = proj.name,
                detail = detail,
                icon = open > 0 and "✅" or "✨",
                action = "view:" .. proj.ai_dir .. "/next-steps.md",
            }
        end

        return {
            title = "Next Steps — pick a project",
            items = items,
        }
    end,

    on_action = function(action)
        if not action then return end
        local path = action:match("^view:(.+)$")
        if not path then return end

        local content = lib.read_file(path)
        if not content then
            return { title = "Next Steps", items = {
                { label = "File not found", detail = path, icon = "⚠" },
            }}
        end

        local items = {}
        local current_section = nil

        for line in content:gmatch("[^\n]*") do
            if line:match("^##%s+") or line:match("^###%s+") then
                current_section = line:gsub("^##+%s+", "")
            elseif line:match("^%s*%-%s*%[x%]") then
                local text = line:gsub("^%s*%-%s*%[x%]%s*", "")
                items[#items + 1] = {
                    label = text,
                    detail = (current_section or "") .. "  ·  done",
                    icon = "☑️",
                }
            elseif line:match("^%s*%-%s*%[%s%]") then
                local text = line:gsub("^%s*%-%s*%[%s%]%s*", "")
                items[#items + 1] = {
                    label = text,
                    detail = current_section or "",
                    icon = "⬜",
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
            title = (proj_name or "Project") .. " — Next Steps",
            items = items,
        }
    end,
})
