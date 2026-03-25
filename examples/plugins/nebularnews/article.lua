-- Nebular News: Article — full content rendered as markdown.

local function get_auth()
    local url = lark.env("NEBULARNEWS_URL")
    local token = lark.env("NEBULARNEWS_TOKEN")
    if not url then return nil, nil, "NEBULARNEWS_URL not set" end
    if not token then return nil, nil, "NEBULARNEWS_TOKEN not set" end
    return url, { Authorization = "Bearer " .. token, Accept = "application/json" }, nil
end

local function score_stars(score)
    if not score then return "unscored" end
    return string.rep("★", math.min(score, 5)) .. string.rep("☆", 5 - math.min(score, 5))
end

lark.register({
    on_run = function()
        local article_id = lark.form_values and lark.form_values.article_id
        if not article_id or article_id == "" then
            return {
                title = "Article",
                form = {
                    fields = {
                        {
                            id = "article_id",
                            label = "Article ID",
                            type = { kind = "text" },
                            required = true,
                            placeholder = "Enter article UUID...",
                        },
                    },
                    submit_label = "View Article",
                },
            }
        end

        local url, headers, err = get_auth()
        if not url then
            return { title = "Article", items = { { label = err, icon = "!" } } }
        end

        local resp = lark.http.get(url .. "/api/mobile/articles/" .. article_id,
            { headers = headers, timeout = 10 })

        if resp.status ~= 200 then
            return { title = "Article", items = { { label = "HTTP " .. resp.status, icon = "!" } } }
        end

        local ok, data = pcall(lark.json.decode, resp.body)
        if not ok then
            return { title = "Article", items = { { label = "Parse error", icon = "!" } } }
        end

        local a = data.article or {}
        local title = a.title or "Untitled"
        local author = a.author or "Unknown author"
        local source = data.preferredSource and data.preferredSource.sourceName or "Unknown source"

        local pub_date = ""
        if a.published_at then
            pub_date = lark.exec("date", { "-r", tostring(math.floor(a.published_at / 1000)), "+%B %d, %Y" }) or ""
            pub_date = pub_date:gsub("%s+$", "")
        end

        -- Build markdown.
        local md = {}
        md[#md + 1] = "# " .. title
        md[#md + 1] = "*" .. source .. " · " .. author .. " · " .. pub_date .. "*"
        md[#md + 1] = ""

        if data.score and data.score.score then
            local line = "**Score: " .. score_stars(data.score.score) .. " (" .. data.score.score .. "/5)**"
            if data.score.label then line = line .. " — " .. data.score.label end
            md[#md + 1] = line
            md[#md + 1] = ""
        end

        if data.summary and data.summary.summary_text then
            md[#md + 1] = "> " .. data.summary.summary_text
            md[#md + 1] = ""
        end

        if data.keyPoints and data.keyPoints.key_points_json then
            local kp_ok, points = pcall(lark.json.decode, data.keyPoints.key_points_json)
            if kp_ok and type(points) == "table" and #points > 0 then
                md[#md + 1] = "**Key Points:**"
                for _, p in ipairs(points) do
                    md[#md + 1] = "- " .. tostring(p)
                end
                md[#md + 1] = ""
            end
        end

        md[#md + 1] = "---"
        md[#md + 1] = ""

        local content = a.content_text or a.content_html or a.excerpt or "No content available."
        if a.content_html and not a.content_text then
            content = content:gsub("<[^>]+>", "")
        end
        md[#md + 1] = content

        local article_url = a.canonical_url or ""

        return {
            title = title,
            raw_text = table.concat(md, "\n"),
            output_format = "markdown",
            items = {},
        }
    end,
})
