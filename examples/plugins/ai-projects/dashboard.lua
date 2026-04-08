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

local function project_title(path, suffix)
    local proj_name = path:match("/git/([^/]+)/")
    return (proj_name or "Project") .. " — " .. suffix
end

local function view_current_state(path)
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

    return { title = project_title(path, "Current State"), items = items }
end

local function view_next_steps(path)
    local content = read_file(path)
    if not content then
        return { title = "Next Steps", items = {
            { label = "File not found", detail = path, icon = "⚠" },
        }}
    end

    local items = {}
    local section = nil
    for line in content:gmatch("[^\n]*") do
        if line:match("^##%s+") or line:match("^###%s+") then
            section = line:gsub("^##+%s+", "")
        elseif line:match("^%s*%-%s*%[x%]") then
            local text = line:gsub("^%s*%-%s*%[x%]%s*", "")
            items[#items + 1] = {
                label = text,
                detail = (section or "") .. "  ·  done",
                icon = "☑️",
            }
        elseif line:match("^%s*%-%s*%[%s%]") then
            local text = line:gsub("^%s*%-%s*%[%s%]%s*", "")
            items[#items + 1] = {
                label = text,
                detail = section or "",
                icon = "⬜",
            }
        end
    end

    return { title = project_title(path, "Next Steps"), items = items }
end

local function view_roadmap(path)
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

    return { title = project_title(path, "Roadmap"), items = items }
end

local function view_decisions(path)
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
                            summary = stripped
                            break
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

    return { title = project_title(path, "Decisions"), items = items }
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
        local view_kind, view_path = action:match("^view:([^:]+):(.+)$")
        if view_kind and view_path then
            if view_kind == "current-state" then
                return view_current_state(view_path)
            elseif view_kind == "next-steps" then
                return view_next_steps(view_path)
            elseif view_kind == "roadmap" then
                return view_roadmap(view_path)
            elseif view_kind == "decisions" then
                return view_decisions(view_path)
            end
        end

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
            action = "view:current-state:" .. proj.ai_dir .. "/current-state.md",
        }

        local steps_detail = open .. " open"
        if done > 0 then steps_detail = steps_detail .. ", " .. done .. " done" end
        items[#items + 1] = {
            label = "Next Steps",
            detail = steps_detail,
            icon = "✅",
            action = "view:next-steps:" .. proj.ai_dir .. "/next-steps.md",
        }

        items[#items + 1] = {
            label = "Roadmap",
            detail = milestone or "no active milestone",
            icon = "🗺️",
            action = "view:roadmap:" .. proj.ai_dir .. "/roadmap.md",
        }

        local dec_detail = num_decs .. " ADR" .. (num_decs ~= 1 and "s" or "")
        items[#items + 1] = {
            label = "Decisions",
            detail = dec_detail,
            icon = "📝",
            action = "view:decisions:" .. proj.ai_dir .. "/decisions.md",
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
