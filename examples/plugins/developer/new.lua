-- Developer: New Plugin — scaffold a new Lark plugin via form.

lark.register({
    on_run = function()
        if lark.form_values then
            local name = lark.form_values.name or ""
            local kind = lark.form_values.kind or "lua"
            local multi = lark.form_values.multi or "false"

            if name == "" then
                return { title = "New Plugin", items = { { label = "Name is required", icon = "!" } } }
            end

            local args = { "init-plugin", name }
            if kind == "shell" then
                args[#args + 1] = "--shell"
            end
            if multi == "true" then
                args[#args + 1] = "--multi"
            end

            local result = lark.exec("lark", args)
            if result and result ~= "" then
                return {
                    title = "New Plugin",
                    items = {
                        { label = "Created: " .. name, icon = "✅" },
                        { label = result:gsub("%s+$", ""), icon = "📁" },
                        { label = "Press R in lark to refresh and see it", icon = "💡" },
                    },
                }
            else
                return {
                    title = "New Plugin",
                    items = { { label = "Failed to create plugin (does it already exist?)", icon = "!" } },
                }
            end
        end

        return {
            title = "New Plugin",
            form = {
                fields = {
                    {
                        id = "name",
                        label = "Plugin Name",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "my-plugin",
                    },
                    {
                        id = "kind",
                        label = "Type",
                        type = { kind = "select", options = { "lua", "shell" } },
                        default = "lua",
                    },
                    {
                        id = "multi",
                        label = "Multi-command",
                        type = { kind = "toggle" },
                        default = "false",
                    },
                },
                submit_label = "Create Plugin",
            },
        }
    end,
})
