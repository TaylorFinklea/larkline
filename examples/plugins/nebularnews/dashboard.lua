-- Nebular News: Dashboard — reading stats and scoring overview.

local function get_auth()
    local url = lark.env("NEBULARNEWS_URL")
    local token = lark.env("NEBULARNEWS_TOKEN")
    if not url then return nil, nil, "NEBULARNEWS_URL not set" end
    if not token then return nil, nil, "NEBULARNEWS_TOKEN not set" end
    return url, { Authorization = "Bearer " .. token, Accept = "application/json" }, nil
end

lark.register({
    on_run = function()
        local url, headers, err = get_auth()
        if not url then
            return { title = "Dashboard", items = { { label = err, icon = "!" } } }
        end

        local resp = lark.http.get(url .. "/api/mobile/dashboard", { headers = headers, timeout = 10 })

        if resp.status ~= 200 then
            return { title = "Dashboard", items = { { label = "HTTP " .. resp.status, icon = "!" } } }
        end

        local ok, data = pcall(lark.json.decode, resp.body)
        if not ok then
            return { title = "Dashboard", items = { { label = "Parse error", icon = "!" } } }
        end

        local stats = data.stats or {}
        local scoring = data.scoring or {}

        local items = {}

        items[#items + 1] = {
            label = tostring(stats.unreadCount or 0) .. " unread",
            detail = tostring(stats.totalArticles or 0) .. " total articles",
            icon = "📬",
        }
        items[#items + 1] = {
            label = tostring(stats.todayArticles or 0) .. " today",
            detail = tostring(stats.thisWeekArticles or 0) .. " this week",
            icon = "📅",
        }

        local reactions = stats.reactionsCount or {}
        items[#items + 1] = {
            label = "👍 " .. tostring(reactions.up or 0) .. "  👎 " .. tostring(reactions.down or 0),
            detail = "Your reactions",
            icon = "💬",
        }

        if scoring.averageScore then
            items[#items + 1] = {
                label = string.format("Average score: %.1f / 5", scoring.averageScore),
                detail = "Across all scored articles",
                icon = "★",
            }
        end

        local dist = scoring.scoreDistribution
        if dist then
            local parts = {}
            for s = 5, 1, -1 do
                local count = dist[tostring(s)] or 0
                if count > 0 then
                    parts[#parts + 1] = string.rep("★", s) .. ": " .. tostring(count)
                end
            end
            if #parts > 0 then
                items[#items + 1] = {
                    label = "Score distribution",
                    detail = table.concat(parts, "  "),
                    icon = "📊",
                }
            end
        end

        local activity = data.recentActivity or {}
        items[#items + 1] = {
            label = tostring(activity.feedsCount or 0) .. " feeds",
            detail = tostring(activity.tagsCount or 0) .. " tags",
            icon = "📡",
        }

        return { title = "Dashboard", items = items }
    end,
})
