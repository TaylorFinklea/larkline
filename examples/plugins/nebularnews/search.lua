-- Nebular News: Search — search articles by keyword.

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
        if not lark.form_values or not lark.form_values.query or lark.form_values.query == "" then
            return {
                title = "Search",
                form = {
                    fields = {
                        {
                            id = "query",
                            label = "Search articles",
                            type = { kind = "text" },
                            required = true,
                            placeholder = "e.g. rust async, kubernetes, AI safety...",
                        },
                    },
                    submit_label = "Search",
                },
            }
        end

        local url, headers, err = get_auth()
        if not url then
            return { title = "Search", items = { { label = err, icon = "!" } } }
        end

        local query = lark.form_values.query
        local resp = lark.http.get(url .. "/api/mobile/articles?q=" .. query .. "&limit=20&sort=score_desc",
            { headers = headers, timeout = 10 })

        if resp.status ~= 200 then
            return { title = "Search", items = { { label = "HTTP " .. resp.status, icon = "!" } } }
        end

        local ok, data = pcall(lark.json.decode, resp.body)
        if not ok or not data.articles then
            return { title = "Search", items = { { label = "Parse error", icon = "!" } } }
        end

        if #data.articles == 0 then
            return { title = "Search: " .. query, items = { { label = "No results", icon = "📭" } } }
        end

        local items = {}
        for _, a in ipairs(data.articles) do
            local stars = score_stars(a.score)
            local source = a.source and a.source.name or ""
            local read_mark = a.is_read == 1 and "✓ " or ""
            local article_url = a.canonical_url or ""

            items[#items + 1] = {
                label = read_mark .. stars .. "  " .. (a.title or "Untitled"),
                detail = source,
                icon = "▲",
                url = article_url,
                copy_text = article_url,
                actions = {
                    { label = "Open in browser", kind = "open", args = { article_url } },
                    { label = "Copy URL", kind = "clipboard", args = { article_url } },
                },
            }
        end

        return { title = "Search: " .. query .. " — " .. (data.total or #items), items = items }
    end,
})
