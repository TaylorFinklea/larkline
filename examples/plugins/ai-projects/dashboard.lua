-- AI Projects Dashboard — one row per project with status summary.

local lib = require("lib")

lark.register({
    on_run = function()
        local projects = lib.discover_projects()

        if #projects == 0 then
            return {
                title = "AI Projects",
                items = {
                    { label = "No projects found", detail = "No .docs/ai/ or docs/ai/ dirs in ~/git", icon = "📭" },
                },
            }
        end

        -- Sort by recency: fresh first, then recent, then stale.
        -- Within each tier, alphabetical.
        local order = { fresh = 1, recent = 2, stale = 3, unknown = 4 }
        local enriched = {}
        for _, proj in ipairs(projects) do
            local state_content = lib.read_file(proj.ai_dir .. "/current-state.md")
            local steps_content = lib.read_file(proj.ai_dir .. "/next-steps.md")

            local date = lib.extract_date(state_content)
            local branch = lib.extract_branch(state_content)
            local open = lib.count_open_items(steps_content)
            local icon, tier = lib.recency(date)

            enriched[#enriched + 1] = {
                proj = proj,
                date = date,
                branch = branch,
                open = open,
                icon = icon,
                tier = tier,
                sort_key = order[tier] or 4,
            }
        end

        table.sort(enriched, function(a, b)
            if a.sort_key ~= b.sort_key then return a.sort_key < b.sort_key end
            return a.proj.name < b.proj.name
        end)

        local items = {}
        local fresh_count = 0
        local total_open = 0

        for _, e in ipairs(enriched) do
            if e.tier == "fresh" or e.tier == "recent" then fresh_count = fresh_count + 1 end
            total_open = total_open + e.open

            local detail_parts = {}
            detail_parts[#detail_parts + 1] = e.branch
            detail_parts[#detail_parts + 1] = lib.short_date(e.date)
            if e.open > 0 then
                detail_parts[#detail_parts + 1] = e.open .. " next step" .. (e.open ~= 1 and "s" or "")
            end

            items[#items + 1] = {
                label = e.proj.name,
                detail = table.concat(detail_parts, "  ·  "),
                icon = e.icon,
                action = "open:" .. e.proj.name,
                metadata = {
                    columns = { "Branch", "Updated", "Open" },
                    branch = e.branch,
                    updated = lib.short_date(e.date),
                    open = tostring(e.open),
                },
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

        -- Find the project and show sub-commands.
        local projects = lib.discover_projects()
        local proj = nil
        for _, p in ipairs(projects) do
            if p.name == project_name then proj = p; break end
        end
        if not proj then
            return { title = project_name, items = {
                { label = "Project not found", icon = "⚠" },
            }}
        end

        local state_content = lib.read_file(proj.ai_dir .. "/current-state.md")
        local steps_content = lib.read_file(proj.ai_dir .. "/next-steps.md")
        local roadmap_content = lib.read_file(proj.ai_dir .. "/roadmap.md")
        local decisions_content = lib.read_file(proj.ai_dir .. "/decisions.md")

        local date = lib.extract_date(state_content)
        local branch = lib.extract_branch(state_content)
        local open_steps = lib.count_open_items(steps_content)
        local done_steps = lib.count_done_items(steps_content)
        local milestone = lib.extract_active_milestone(roadmap_content)
        local num_decisions = lib.count_decisions(decisions_content)

        local items = {}

        -- Current State row.
        local state_detail = branch .. "  ·  " .. lib.short_date(date)
        items[#items + 1] = {
            label = "Current State",
            detail = state_detail,
            icon = "📊",
            action = "view:" .. proj.ai_dir .. "/current-state.md",
        }

        -- Next Steps row.
        local steps_detail = open_steps .. " open"
        if done_steps > 0 then steps_detail = steps_detail .. ", " .. done_steps .. " done" end
        items[#items + 1] = {
            label = "Next Steps",
            detail = steps_detail,
            icon = "✅",
            action = "view:" .. proj.ai_dir .. "/next-steps.md",
        }

        -- Roadmap row.
        local roadmap_detail = milestone or "no active milestone"
        items[#items + 1] = {
            label = "Roadmap",
            detail = roadmap_detail,
            icon = "🗺️",
            action = "view:" .. proj.ai_dir .. "/roadmap.md",
        }

        -- Decisions row.
        local dec_detail = num_decisions .. " ADR" .. (num_decisions ~= 1 and "s" or "")
        items[#items + 1] = {
            label = "Decisions",
            detail = dec_detail,
            icon = "📝",
            action = "view:" .. proj.ai_dir .. "/decisions.md",
        }

        -- Open in editor action.
        items[#items + 1] = {
            label = "Open project in editor",
            detail = proj.path,
            icon = "📂",
            action = "shell:open:" .. proj.path,
        }

        return {
            title = project_name,
            items = items,
        }
    end,
})
