-- GitHub search — type a query, open the GitHub search results page.
-- Form pattern mirrors caffeinate/start.lua; open-action shape mirrors
-- github/my-prs.lua and hackernews/init.lua (kind = "open", args = { url }).
-- mlua sandbox has no os/require, so urlencode is inlined per-file.

-- SHARED: urlencode — percent-encode everything except unreserved [A-Za-z0-9_.~-];
-- spaces become %20. Canonical copy duplicated across web-search-shortcuts/*.lua.
local function urlencode(s)
    return (s:gsub("[^A-Za-z0-9_.~-]", function(c)
        return string.format("%%%02X", string.byte(c))
    end))
end

local TITLE = "GitHub"
local BASE = "https://github.com/search?q="

lark.register({
    on_run = function()
        if lark.form_values then
            local query = lark.form_values.query or ""
            if query:match("^%s*$") then
                return {
                    title = TITLE,
                    items = { { label = "Enter a search query", icon = "⚠" } },
                }
            end

            local url = BASE .. urlencode(query)
            return {
                title = TITLE,
                items = { {
                    label = "Search " .. TITLE .. ": " .. query,
                    detail = url,
                    icon = "🔍",
                    url = url,
                    copy_text = url,
                    actions = {
                        { label = "Open in browser", kind = "open", args = { url } },
                        { label = "Copy URL", kind = "clipboard", args = { url } },
                    },
                } },
            }
        end

        return {
            title = TITLE,
            form = {
                fields = {
                    {
                        id = "query",
                        label = "Search query",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "What do you want to search for?",
                    },
                },
                submit_label = "Search",
            },
        }
    end,
})
