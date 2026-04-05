-- AI Projects Dashboard — one row per project with status summary.
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

local function count_open(c)
    if not c then return 0 end
    local n = 0; for _ in c:gmatch("%-%s%[%s%]") do n = n + 1 end; return n
end

local function count_done(c)
    if not c then return 0 end
    local n = 0; for _ in c:gmatch("%-%s%[x%]") do n = n + 1 end; return n
end

local function count_decisions(c)
    if not c then return 0 end
    local n = 0; for _ in c:gmatch("\n##%s+[^\n]") do n = n + 1 end; return n
end

local function recency(date_str)
    if not date_str then return "⚪", "unknown" end
    local today_raw = lark.exec("date", { "+%Y-%m-%d" })
    if not today_raw then return "⚪", "unknown" end
    local ty, tm, td = today_raw:gsub("%s+$", ""):match("(%d+)-(%d+)-(%d+)")
    local dy, dm, dd = date_str:match("(%d+)-(%d+)-(%d+)")
    if not ty or not dy then return "⚪", "unknown" end
    local diff = (tonumber(ty)*10000+tonumber(tm)*100+tonumber(td))
              - (tonumber(dy)*10000+tonumber(dm)*100+tonumber(dd))
    if diff <= 2 then return "🟢", "fresh"
    elseif diff <= 7 then return "🟡", "recent"
    else return "⚪", "stale" end
end

local MONTHS = {"Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"}
local function short_date(ds)
    if not ds then return "???" end
    local _, m, d = ds:match("(%d+)-(%d+)-(%d+)")
    if not m then return "???" end
    return MONTHS[tonumber(m) or 1] .. " " .. (d or "??")
end

local function extract_active_milestone(c)
    if not c then return nil end
    return c:match("###%s+(v%d[^\n]*)") or c:match("###%s+(M%d[^\n]*)")
        or c:match("###%s+(Phase%s+%d[^\n]*)")
end

-- Dashboard logic.

lark.register({
    on_run = function()
        local projects = discover_projects()
        if #projects == 0 then
            return { title = "AI Projects", items = {
                { label = "No projects found", detail = "No .docs/ai/ or docs/ai/ in ~/git", icon = "📭" },
            }}
        end

        local order = { fresh = 1, recent = 2, stale = 3, unknown = 4 }
        local enriched = {}
        for _, proj in ipairs(projects) do
            local state = read_file(proj.ai_dir .. "/current-state.md")
            local steps = read_file(proj.ai_dir .. "/next-steps.md")
            local date = extract_date(state)
            local branch = extract_branch(state)
            local open = count_open(steps)
            local icon, tier = recency(date)
            enriched[#enriched + 1] = {
                proj = proj, date = date, branch = branch,
                open = open, icon = icon, tier = tier,
                sort_key = order[tier] or 4,
            }
        end

        table.sort(enriched, function(a, b)
            if a.sort_key ~= b.sort_key then return a.sort_key < b.sort_key end
            return a.proj.name < b.proj.name
        end)

        local items = {}
        local total_open = 0
        for _, e in ipairs(enriched) do
            total_open = total_open + e.open
            local parts = { e.branch, short_date(e.date) }
            if e.open > 0 then
                parts[#parts + 1] = e.open .. " next step" .. (e.open ~= 1 and "s" or "")
            end
            items[#items + 1] = {
                label = e.proj.name,
                detail = table.concat(parts, "  ·  "),
                icon = e.icon,
                action = "open:" .. e.proj.name,
            }
        end

        return {
            title = "AI Projects — " .. #projects .. " projects, " .. total_open .. " open steps",
            items = items,
        }
    end,

    on_action = function(action)
        if not action then return end
        local project_name = action:match("^open:(.+)$")
        if not project_name then return end

        local projects = discover_projects()
        local proj = nil
        for _, p in ipairs(projects) do
            if p.name == project_name then proj = p; break end
        end
        if not proj then
            return { title = project_name, items = {
                { label = "Project not found", icon = "⚠" },
            }}
        end

        local state = read_file(proj.ai_dir .. "/current-state.md")
        local steps = read_file(proj.ai_dir .. "/next-steps.md")
        local roadmap = read_file(proj.ai_dir .. "/roadmap.md")
        local decs = read_file(proj.ai_dir .. "/decisions.md")

        local date = extract_date(state)
        local branch = extract_branch(state)
        local open = count_open(steps)
        local done = count_done(steps)
        local milestone = extract_active_milestone(roadmap)
        local num_decs = count_decisions(decs)

        local items = {}

        items[#items + 1] = {
            label = "Current State",
            detail = branch .. "  ·  " .. short_date(date),
            icon = "📊",
            action = "shell:cat " .. proj.ai_dir .. "/current-state.md",
        }

        local steps_detail = open .. " open"
        if done > 0 then steps_detail = steps_detail .. ", " .. done .. " done" end
        items[#items + 1] = {
            label = "Next Steps",
            detail = steps_detail,
            icon = "✅",
            action = "shell:cat " .. proj.ai_dir .. "/next-steps.md",
        }

        items[#items + 1] = {
            label = "Roadmap",
            detail = milestone or "no active milestone",
            icon = "🗺️",
            action = "shell:cat " .. proj.ai_dir .. "/roadmap.md",
        }

        local dec_detail = num_decs .. " ADR" .. (num_decs ~= 1 and "s" or "")
        items[#items + 1] = {
            label = "Decisions",
            detail = dec_detail,
            icon = "📝",
            action = "shell:cat " .. proj.ai_dir .. "/decisions.md",
        }

        items[#items + 1] = {
            label = "Open in editor",
            detail = proj.path,
            icon = "📂",
            action = "shell:open " .. proj.path,
        }

        return { title = project_name, items = items }
    end,
})
