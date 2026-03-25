-- Hacker News — top 15 stories from the Firebase API.

local HN_API = "https://hacker-news.firebaseio.com/v0"

lark.register({
    on_run = function()
        local resp = lark.http.get(HN_API .. "/topstories.json", { timeout = 5 })
        if resp.status ~= 200 then
            return {
                title = "Hacker News",
                items = { { label = "Failed to fetch top stories", detail = "HTTP " .. resp.status, icon = "!" } },
            }
        end

        local ok, ids = pcall(lark.json.decode, resp.body)
        if not ok or type(ids) ~= "table" then
            return {
                title = "Hacker News",
                items = { { label = "Failed to parse story IDs", icon = "!" } },
            }
        end

        local items = {}
        local count = math.min(15, #ids)
        for i = 1, count do
            local item_resp = lark.http.get(HN_API .. "/item/" .. ids[i] .. ".json", { timeout = 5 })
            if item_resp.status == 200 then
                local item_ok, story = pcall(lark.json.decode, item_resp.body)
                if item_ok and story then
                    local comments = story.descendants or 0
                    local score = story.score or 0
                    local url = story.url or ("https://news.ycombinator.com/item?id=" .. ids[i])
                    local hn_url = "https://news.ycombinator.com/item?id=" .. ids[i]

                    items[#items + 1] = {
                        label = story.title or "Untitled",
                        detail = score .. " pts  " .. comments .. " comments",
                        icon = "▲",
                        url = url,
                        copy_text = url,
                        actions = {
                            { label = "Open article", kind = "open", args = { url } },
                            { label = "Open HN comments", kind = "open", args = { hn_url } },
                            { label = "Copy URL", kind = "clipboard", args = { url } },
                        },
                    }
                end
            end
        end

        return { title = "Hacker News — Top " .. #items, items = items }
    end,
})
