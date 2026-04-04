-- View a project's next-steps.md as a structured checklist.

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

local function count_open(c)
    if not c then return 0 end
    local n = 0; for _ in c:gmatch("%-%s%[%s%]") do n = n + 1 end; return n
end

local function count_done(c)
    if not c then return 0 end
    local n = 0; for _ in c:gmatch("%-%s%[x%]") do n = n + 1 end; return n
end

lark.register({
    on_run = function()
        local projects = discover_projects()
        local items = {}
        for _, proj in ipairs(projects) do
            local content = read_file(proj.ai_dir .. "/next-steps.md")
            local open = count_open(content)
            local done = count_done(content)
            local detail = open .. " open"
            if done > 0 then detail = detail .. ", " .. done .. " done" end
            items[#items + 1] = {
                label = proj.name,
                detail = detail,
                icon = open > 0 and "✅" or "✨",
                action = "view:" .. proj.ai_dir .. "/next-steps.md",
            }
        end
        return { title = "Next Steps — pick a project", items = items }
    end,

    on_action = function(action)
        if not action then return end
        local path = action:match("^view:(.+)$")
        if not path then return end
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

        local proj_name = path:match("/git/([^/]+)/")
        return { title = (proj_name or "Project") .. " — Next Steps", items = items }
    end,
})
