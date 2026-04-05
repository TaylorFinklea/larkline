-- View a project's roadmap.md — vision, milestones, backlog.
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

local function extract_active_milestone(c)
    if not c then return nil end
    return c:match("###%s+(v%d[^\n]*)") or c:match("###%s+(M%d[^\n]*)")
        or c:match("###%s+(Phase%s+%d[^\n]*)")
end

local function extract_vision(c)
    if not c then return nil end
    local after = c:match("##%s*Vision%s*\n(.-)\n##") or c:match("##%s*Vision%s*\n(.+)")
    if not after then return nil end
    for line in after:gmatch("[^\n]+") do
        local stripped = line:gsub("^%s*>?%s*", ""):gsub("<!%-%-.*%-%->", ""):gsub("^%s+", "")
        if stripped ~= "" and not stripped:match("^#") then
            if #stripped > 80 then stripped = stripped:sub(1, 77) .. "..." end
            return stripped
        end
    end
    return nil
end

lark.register({
    on_run = function()
        local projects = discover_projects()
        local items = {}
        for _, proj in ipairs(projects) do
            local content = read_file(proj.ai_dir .. "/roadmap.md")
            local milestone = extract_active_milestone(content)
            local vision = extract_vision(content)
            items[#items + 1] = {
                label = proj.name,
                detail = milestone or vision or "no roadmap data",
                icon = "🗺️",
                action = "view:" .. proj.ai_dir .. "/roadmap.md",
            }
        end
        return { title = "Roadmap — pick a project", items = items }
    end,

    on_action = function(action)
        if not action then return end
        local path = action:match("^view:(.+)$")
        if not path then return end
        local content = read_file(path)
        if not content then
            return { title = "Roadmap", items = {
                { label = "File not found", detail = path, icon = "⚠" },
            }}
        end

        local items = {}
        local h2, h3 = nil, nil
        for line in content:gmatch("[^\n]*") do
            if line:match("^##%s+") and not line:match("^###") then
                h2 = line:gsub("^##%s+", "")
                h3 = nil
                if not h2:match("^Constraints") and not h2:match("^Non%-Goals") then
                    items[#items + 1] = { label = h2, detail = "", icon = "📋" }
                end
            elseif line:match("^###%s+") then
                h3 = line:gsub("^###%s+", "")
                items[#items + 1] = { label = h3, detail = h2 or "", icon = "📌" }
            elseif line:match("^%s*%-%s*%[x%]") then
                local text = line:gsub("^%s*%-%s*%[x%]%s*", "")
                items[#items + 1] = { label = text, detail = (h3 or h2 or "") .. "  ·  done", icon = "☑️" }
            elseif line:match("^%s*%-%s*%[%s%]") then
                local text = line:gsub("^%s*%-%s*%[%s%]%s*", "")
                items[#items + 1] = { label = text, detail = h3 or h2 or "", icon = "⬜" }
            elseif line:match("^%s*%-%s*%*%*") then
                local text = line:gsub("^%s*%-%s*", ""):gsub("%*%*", "")
                items[#items + 1] = { label = text, detail = h3 or h2 or "", icon = "📎" }
            end
        end

        local proj_name = path:match("/git/([^/]+)/")
        return { title = (proj_name or "Project") .. " — Roadmap", items = items }
    end,
})
