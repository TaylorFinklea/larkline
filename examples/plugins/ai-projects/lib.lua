-- Shared helpers for AI Projects plugin.
-- This file is NOT loaded by require(). Instead, each command file
-- copies the helpers it needs inline, since the Lark sandbox does not
-- expose require/dofile/loadfile. This file serves as the canonical
-- source — edit here, then sync to the command files.

-- Scan ~/git for directories containing .docs/ai/ or docs/ai/.
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

local function extract_date(content)
    if not content then return nil end
    return content:match("(%d%d%d%d%-%d%d%-%d%d)")
end

local function extract_branch(content)
    if not content then return "?" end
    return content:match("##%s*Active Branch.-\n%s*`([^`]+)`")
        or content:match("##%s*Branch.-\n%s*.-`([^`]+)`")
        or "?"
end

local function count_open(content)
    if not content then return 0 end
    local n = 0
    for _ in content:gmatch("%-%s%[%s%]") do n = n + 1 end
    return n
end

local function count_done(content)
    if not content then return 0 end
    local n = 0
    for _ in content:gmatch("%-%s%[x%]") do n = n + 1 end
    return n
end

local function count_decisions(content)
    if not content then return 0 end
    local n = 0
    for _ in content:gmatch("\n##%s+[^\n]") do n = n + 1 end
    return n
end

local function recency(date_str)
    if not date_str then return "⚪", "unknown" end
    local today_raw = lark.exec("date", { "+%Y-%m-%d" })
    if not today_raw then return "⚪", "unknown" end
    local today = today_raw:gsub("%s+$", "")
    local ty, tm, td = today:match("(%d+)-(%d+)-(%d+)")
    local dy, dm, dd = date_str:match("(%d+)-(%d+)-(%d+)")
    if not ty or not dy then return "⚪", "unknown" end
    local today_n = tonumber(ty) * 10000 + tonumber(tm) * 100 + tonumber(td)
    local date_n = tonumber(dy) * 10000 + tonumber(dm) * 100 + tonumber(dd)
    local diff = today_n - date_n
    if diff <= 2 then return "🟢", "fresh"
    elseif diff <= 7 then return "🟡", "recent"
    else return "⚪", "stale" end
end

local MONTHS = { "Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec" }
local function short_date(date_str)
    if not date_str then return "???" end
    local _, m, d = date_str:match("(%d+)-(%d+)-(%d+)")
    if not m then return "???" end
    return MONTHS[tonumber(m) or 1] .. " " .. (d or "??")
end

local function extract_active_milestone(content)
    if not content then return nil end
    return content:match("###%s+(v%d[^\n]*)")
        or content:match("###%s+(M%d[^\n]*)")
        or content:match("###%s+(Phase%s+%d[^\n]*)")
end
