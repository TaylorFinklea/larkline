-- Add Link — save a new bookmark via form.

lark.register({
    on_run = function()
        if lark.form_values then
            local name = lark.form_values.name or ""
            local url = lark.form_values.url or ""
            local icon = lark.form_values.icon or ""

            if name == "" or url == "" then
                return {
                    title = "Add Link",
                    items = { { label = "Name and URL are required", icon = "!" } },
                }
            end

            local data = lark.store.get("links") or "[]"
            local links = lark.json.decode(data) or {}

            links[#links + 1] = { name = name, url = url, icon = icon ~= "" and icon or nil }
            lark.store.set("links", lark.json.encode(links))

            return {
                title = "Add Link",
                items = {
                    { label = "Saved: " .. name, detail = url, icon = "✅" },
                },
            }
        end

        return {
            title = "Add Link",
            form = {
                fields = {
                    {
                        id = "name",
                        label = "Name",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "e.g. GitHub Dashboard",
                    },
                    {
                        id = "url",
                        label = "URL or Path",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "e.g. https://github.com or ~/Documents/notes",
                    },
                    {
                        id = "icon",
                        label = "Icon (optional)",
                        type = { kind = "text" },
                        required = false,
                        placeholder = "e.g. 🏠 or leave blank",
                    },
                },
                submit_label = "Save",
            },
        }
    end,
})
