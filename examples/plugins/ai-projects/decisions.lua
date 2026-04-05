-- View a project's decisions.md — architecture decision records.
-- Helpers are inlined because the Lark sandbox has no require/dofile.
-- Canonical source: lib.lua — copy updated helpers from there when editing.

-- Helpers (copied from lib.lua):
local function discover_projects()
    local home = lark.env("HOME") or "/tmp"
    local git_dir = home .. "/git"
    local raw = lark.exec("ls", { "-1", git_dir })
    if not raw or raw == "" then return {} end
    local projects = {}
    for name in raw:gmatch("[^\n]+") do
        local base = git_dir .. "/" .. name
        local ls_new = lark.exec("ls", { base .. "/.docs/ai/current-state.md" })
        local ls_old = lark.exec("ls", { base .. "/docs/ai/current-state.md" })
        local ai_dir = nil
        if ls_new and ls_new:match("current%-state%.md") then
            ai_dir = base .. "/.docs/ai"
        elseif ls_old and ls_old:match("current%-state%.md") then
            ai_dir = base .. "/docs/ai"
        end
        if ai_dir then
            projects[#projects + 1] = { name = name, path = base, ai_dir = ai_dir }
        end
    end
    table.sort(projects, function(a, b) return a.name < b.name end)
    return projects
end

local function read_file(path)
    local content = lark.exec("cat", { path })
    if content and content ~= "" then return content end
    return nil
end

local function count_decisions(c)
    if not c then return 0 end
    local n = 0; for _ in c:gmatch("\n##%s+[^\n]") do n = n + 1 end; return n
end

lark.register({
    on_run = function()
        local projects = discover_projects()
        local items = {}
        for _, proj in ipairs(projects) do
            local content = read_file(proj.ai_dir .. "/decisions.md")
            local count = count_decisions(content)
            items[#items + 1] = {
                label = proj.name,
                detail = count .. " ADR" .. (count ~= 1 and "s" or ""),
                icon = "📝",
                action = "view:" .. proj.ai_dir .. "/decisions.md",
            }
        end
        return { title = "Decisions — pick a project", items = items }
    end,

    on_action = function(action)
        if not action then return end
        local path = action:match("^view:(.+)$")
        if not path then return end
        local content = read_file(path)
        if not content then
            return { title = "Decisions", items = {
                { label = "File not found", detail = path, icon = "⚠" },
            }}
        end

        local items = {}
        local in_entry = false
        local entry_title = nil
        local entry_lines = {}

        for line in content:gmatch("[^\n]*") do
            if line:match("^##%s+") then
                if in_entry and entry_title then
                    local summary = ""
                    for _, el in ipairs(entry_lines) do
                        local decision = el:match("^%*%*Decision%*%*:%s*(.+)")
                            or el:match("^%*%*Decision:%*%*%s*(.+)")
                        if decision then summary = decision; break end
                    end
                    if summary == "" then
                        for _, el in ipairs(entry_lines) do
                            local stripped = el:gsub("^%s*", ""):gsub("^%*%*Context%*%*:%s*", "")
                            if stripped ~= "" and not stripped:match("^<!%-%-") then
                                summary = stripped; break
                            end
                        end
                    end
                    if #summary > 100 then summary = summary:sub(1, 97) .. "..." end
                    items[#items + 1] = { label = entry_title, detail = summary, icon = "📝" }
                end
                entry_title = line:gsub("^##%s+", "")
                entry_lines = {}
                in_entry = true
            elseif in_entry and line ~= "" then
                entry_lines[#entry_lines + 1] = line
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
            items[#items + 1] = { label = entry_title, detail = summary, icon = "📝" }
        end

        if #items == 0 then
            items[#items + 1] = { label = "No decisions recorded", detail = "Add ## entries", icon = "📭" }
        end

        local proj_name = path:match("/git/([^/]+)/")
        return { title = (proj_name or "Project") .. " — Decisions", items = items }
    end,
})
