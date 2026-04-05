-- View a project's current-state.md with structured output.
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

local function extract_date(c)
    if not c then return nil end
    return c:match("(%d%d%d%d%-%d%d%-%d%d)")
end

local function extract_branch(c)
    if not c then return "?" end
    return c:match("##%s*Active Branch.-\n%s*`([^`]+)`")
        or c:match("##%s*Branch.-\n%s*.-`([^`]+)`") or "?"
end

local function recency(date_str)
    if not date_str then return "⚪" end
    local today_raw = lark.exec("date", { "+%Y-%m-%d" })
    if not today_raw then return "⚪" end
    local ty, tm, td = today_raw:gsub("%s+$", ""):match("(%d+)-(%d+)-(%d+)")
    local dy, dm, dd = date_str:match("(%d+)-(%d+)-(%d+)")
    if not ty or not dy then return "⚪" end
    local diff = (tonumber(ty)*10000+tonumber(tm)*100+tonumber(td))
              - (tonumber(dy)*10000+tonumber(dm)*100+tonumber(dd))
    if diff <= 2 then return "🟢"
    elseif diff <= 7 then return "🟡"
    else return "⚪" end
end

local MONTHS = {"Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"}
local function short_date(ds)
    if not ds then return "???" end
    local _, m, d = ds:match("(%d+)-(%d+)-(%d+)")
    if not m then return "???" end
    return MONTHS[tonumber(m) or 1] .. " " .. (d or "??")
end

lark.register({
    on_run = function()
        local projects = discover_projects()
        local items = {}
        for _, proj in ipairs(projects) do
            local content = read_file(proj.ai_dir .. "/current-state.md")
            local date = extract_date(content)
            local branch = extract_branch(content)
            local icon = recency(date)
            items[#items + 1] = {
                label = proj.name,
                detail = branch .. "  ·  " .. short_date(date),
                icon = icon,
                action = "view:" .. proj.ai_dir .. "/current-state.md",
            }
        end
        return { title = "Current State — pick a project", items = items }
    end,

    on_action = function(action)
        if not action then return end
        local path = action:match("^view:(.+)$")
        if not path then return end
        local content = read_file(path)
        if not content then
            return { title = "Current State", items = {
                { label = "File not found", detail = path, icon = "⚠" },
            }}
        end

        local items = {}
        local section = nil
        for line in content:gmatch("[^\n]*") do
            if line:match("^##%s+") then
                section = line:gsub("^##%s+", "")
            elseif line:match("^%s*$") or line:match("^>") or line:match("^#%s+") then
                -- skip
            else
                local cleaned = line:gsub("^%s*%-%s*", ""):gsub("^%*%*(.-)%*%*", "%1")
                if cleaned ~= "" then
                    local icon = "📄"
                    if section and section:match("[Bb]ranch") then icon = "🌿"
                    elseif section and section:match("[Pp]rogress") then icon = "📈"
                    elseif section and section:match("[Vv]alidation") then icon = "✅"
                    elseif section and section:match("[Bb]locker") then icon = "🚧"
                    elseif section and section:match("[Vv]ersion") then icon = "🏷️"
                    end
                    items[#items + 1] = { label = cleaned, detail = section or "", icon = icon }
                end
            end
        end

        local proj_name = path:match("/git/([^/]+)/")
        return { title = (proj_name or "Project") .. " — Current State", items = items }
    end,
})
