-- Nebular News: Saved — your saved articles.

local function get_auth()
    local url = lark.env("NEBULARNEWS_URL")
    local token = lark.env("NEBULARNEWS_TOKEN")
    if not url then return nil, nil, "NEBULARNEWS_URL not set" end
    if not token then return nil, nil, "NEBULARNEWS_TOKEN not set" end
    return url, { Authorization = "Bearer " .. token, Accept = "application/json" }, nil
end

local function score_stars(score)
    if not score then return "     " end
    return string.rep("★", math.min(score, 5)) .. string.rep("☆", 5 - math.min(score, 5))
end

lark.register({
    on_run = function()
        local url, headers, err = get_auth()
        if not url then
            return { title = "Saved", items = { { label = err, icon = "!" } } }
        end

        local resp = lark.http.get(url .. "/api/mobile/articles?saved=true&limit=20&sort=newest",
            { headers = headers, timeout = 10 })

        if resp.status ~= 200 then
            return { title = "Saved", items = { { label = "HTTP " .. resp.status, icon = "!" } } }
        end

        local ok, data = pcall(lark.json.decode, resp.body)
        if not ok or not data.articles then
            return { title = "Saved", items = { { label = "Parse error", icon = "!" } } }
        end

        if #data.articles == 0 then
            return { title = "Saved", items = { { label = "No saved articles", icon = "📭" } } }
        end

        local items = {}
        for _, a in ipairs(data.articles) do
            local stars = score_stars(a.score)
            local source = a.source and a.source.name or ""
            local article_url = a.canonical_url or ""

            items[#items + 1] = {
                label = stars .. "  " .. (a.title or "Untitled"),
                detail = source,
                icon = "🔖",
                url = article_url,
                copy_text = article_url,
                actions = {
                    { label = "Open in browser", kind = "open", args = { article_url } },
                    { label = "Copy URL", kind = "clipboard", args = { article_url } },
                },
            }
        end

        return { title = "Saved — " .. (data.total or #items), items = items }
    end,
})
