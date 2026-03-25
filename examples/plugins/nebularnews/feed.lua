-- Nebular News: Feed — latest articles sorted by score.

local function get_auth()
    local url = lark.env("NEBULARNEWS_URL")
    local token = lark.env("NEBULARNEWS_TOKEN")
    if not url then return nil, nil, "NEBULARNEWS_URL not set — add it to ~/.config/larkline/.env" end
    if not token then return nil, nil, "NEBULARNEWS_TOKEN not set — generate one in your NebularNews web UI" end
    return url, { Authorization = "Bearer " .. token, Accept = "application/json" }, nil
end

local function score_stars(score)
    if not score then return "     " end
    return string.rep("★", math.min(score, 5)) .. string.rep("☆", 5 - math.min(score, 5))
end

local function time_ago(ms)
    if not ms then return "" end
    local now_s = tonumber(lark.exec("date", { "+%s" })) or 0
    local diff = now_s - math.floor(ms / 1000)
    if diff < 3600 then return math.floor(diff / 60) .. "m ago" end
    if diff < 86400 then return math.floor(diff / 3600) .. "h ago" end
    return math.floor(diff / 86400) .. "d ago"
end

lark.register({
    on_run = function()
        local url, headers, err = get_auth()
        if not url then
            return { title = "Feed", items = { { label = err, icon = "!" } } }
        end

        local resp = lark.http.get(url .. "/api/mobile/articles?limit=20&read=unread&sort=score_desc",
            { headers = headers, timeout = 10 })

        if resp.status ~= 200 then
            return { title = "Feed", items = { { label = "API error: HTTP " .. resp.status, icon = "!" } } }
        end

        local ok, data = pcall(lark.json.decode, resp.body)
        if not ok or not data.articles then
            return { title = "Feed", items = { { label = "Failed to parse response", icon = "!" } } }
        end

        if #data.articles == 0 then
            return { title = "Feed", items = { { label = "No unread articles", icon = "✅" } } }
        end

        local items = {}
        for _, a in ipairs(data.articles) do
            local stars = score_stars(a.score)
            local source = a.source and a.source.name or "Unknown"
            local ago = time_ago(a.published_at)
            local words = a.word_count and (a.word_count .. " words") or ""
            local detail_parts = {}
            if source ~= "Unknown" then detail_parts[#detail_parts + 1] = source end
            if ago ~= "" then detail_parts[#detail_parts + 1] = ago end
            if words ~= "" then detail_parts[#detail_parts + 1] = words end

            local article_url = a.canonical_url or ""

            items[#items + 1] = {
                label = stars .. "  " .. (a.title or "Untitled"),
                detail = table.concat(detail_parts, " · "),
                icon = "▲",
                url = article_url,
                copy_text = article_url,
                actions = {
                    { label = "Open in browser", kind = "open", args = { article_url } },
                    { label = "Copy URL", kind = "clipboard", args = { article_url } },
                },
            }
        end

        return { title = "Feed — " .. (data.total or #items) .. " unread", items = items }
    end,
})
