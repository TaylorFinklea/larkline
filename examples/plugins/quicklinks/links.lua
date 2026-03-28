-- Quicklinks — list and open saved bookmarks.

lark.register({
    on_run = function()
        local data = lark.store.get("links") or "[]"
        local links = lark.json.decode(data)

        if not links or #links == 0 then
            return {
                title = "Quicklinks",
                items = {
                    { label = "No bookmarks yet", detail = "Use 'Add Link' to save one", icon = "📭" },
                },
            }
        end

        local items = {}
        for _, link in ipairs(links) do
            local is_url = link.url and (link.url:match("^https?://") or link.url:match("^http://"))
            items[#items + 1] = {
                label = link.name or link.url,
                detail = link.url,
                icon = link.icon or (is_url and "🌐" or "📁"),
                copy_text = link.url,
                actions = {
                    { label = "Open", kind = "shell", args = { "open", link.url } },
                    { label = "Copy URL", kind = "clipboard", args = { link.url } },
                    { label = "Delete", kind = "shell", args = { "lark", "invoke", "Quicklinks:delete:" .. link.name } },
                },
            }
        end

        return { title = "Quicklinks — " .. #items .. " links", items = items }
    end,
})
