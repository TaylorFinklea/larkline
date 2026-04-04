-- Shared helpers for AI Projects plugin.

local M = {}

-- Scan ~/git for directories containing .docs/ai/ or docs/ai/.
function M.discover_projects()
    local home = lark.env("HOME") or "/tmp"
    local git_dir = home .. "/git"

    local raw = lark.exec("ls", { "-1", git_dir })
    if not raw or raw == "" then return {} end

    local projects = {}
    for name in raw:gmatch("[^\n]+") do
        local base = git_dir .. "/" .. name
        -- Check .docs/ai/ first (new convention), fall back to docs/ai/
        local new_path = base .. "/.docs/ai"
        local old_path = base .. "/docs/ai"

        local ls_new = lark.exec("ls", { new_path .. "/current-state.md" })
        local ls_old = lark.exec("ls", { old_path .. "/current-state.md" })

        local ai_dir = nil
        if ls_new and ls_new:match("current%-state%.md") then
            ai_dir = new_path
        elseif ls_old and ls_old:match("current%-state%.md") then
            ai_dir = old_path
        end

        if ai_dir then
            projects[#projects + 1] = {
                name = name,
                path = base,
                ai_dir = ai_dir,
            }
        end
    end

    table.sort(projects, function(a, b) return a.name < b.name end)
    return projects
end

-- Read a file and return its contents (or nil).
function M.read_file(path)
    local content = lark.exec("cat", { path })
    if content and content ~= "" then return content end
    return nil
end

-- Extract the date from current-state.md.
-- Looks for patterns like "> Updated: YYYY-MM-DD" or "**Date**: YYYY-MM-DD"
-- or "# Current State (YYYY-MM-DD)" or "*Last updated: YYYY-MM-DD*"
function M.extract_date(content)
    if not content then return nil end
    return content:match("(%d%d%d%d%-%d%d%-%d%d)")
end

-- Extract branch from current-state.md.
function M.extract_branch(content)
    if not content then return "?" end
    local branch = content:match("##%s*Active Branch.-\n%s*`([^`]+)`")
        or content:match("##%s*Branch.-\n%s*.-`([^`]+)`")
    return branch or "?"
end

-- Count open checklist items (- [ ]) in a file's content.
function M.count_open_items(content)
    if not content then return 0 end
    local count = 0
    for _ in content:gmatch("%-%s%[%s%]") do
        count = count + 1
    end
    return count
end

-- Count completed checklist items (- [x]) in a file's content.
function M.count_done_items(content)
    if not content then return 0 end
    local count = 0
    for _ in content:gmatch("%-%s%[x%]") do
        count = count + 1
    end
    return count
end

-- Count ADR entries in decisions.md.
function M.count_decisions(content)
    if not content then return 0 end
    local count = 0
    for _ in content:gmatch("\n##%s+[^\n]") do
        count = count + 1
    end
    return count
end

-- Determine recency status based on date string.
-- Returns icon, label: fresh (<=2 days), recent (<=7 days), stale (>7 days).
function M.recency(date_str)
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

    if diff <= 2 then
        return "🟢", "fresh"
    elseif diff <= 7 then
        return "🟡", "recent"
    else
        return "⚪", "stale"
    end
end

-- Extract the first meaningful line after "## Vision" in a roadmap.
function M.extract_vision(content)
    if not content then return nil end
    local after = content:match("##%s*Vision%s*\n(.-)\n##")
        or content:match("##%s*Vision%s*\n(.+)")
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

-- Extract what looks like the active milestone/version from a roadmap.
function M.extract_active_milestone(content)
    if not content then return nil end
    return content:match("###%s+(v%d[^\n]*)")
        or content:match("###%s+(M%d[^\n]*)")
        or content:match("###%s+(Phase%s+%d[^\n]*)")
end

-- Format "2026-04-03" as "Apr 03".
function M.short_date(date_str)
    if not date_str then return "???" end
    local months = {
        "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    }
    local _, m, d = date_str:match("(%d+)-(%d+)-(%d+)")
    if not m then return "???" end
    return months[tonumber(m) or 1] .. " " .. (d or "??")
end

return M
