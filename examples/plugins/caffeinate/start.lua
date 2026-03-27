-- Caffeinate: Start — start caffeinate for a user-specified number of minutes.

lark.register({
    on_run = function()
        if lark.form_values then
            local raw_min = lark.form_values.minutes or ""
            local minutes = raw_min:match("^%s*(%d+)%s*$")
            if not minutes or tonumber(minutes) <= 0 then
                return {
                    title = "Start Caffeinate",
                    items = { { label = "Enter a positive number of minutes", icon = "⚠" } },
                }
            end

            lark.exec("spotlight-caffeinate-cli", { "start", minutes })

            return {
                title = "Start Caffeinate",
                items = { {
                    label  = "Started for " .. minutes .. " minutes",
                    detail = "Mac will stay awake",
                    icon   = "☕",
                } },
            }
        end

        return {
            title = "Start Caffeinate",
            form = {
                fields = {
                    {
                        id           = "minutes",
                        label        = "Duration (minutes)",
                        type         = { kind = "text" },
                        required     = true,
                        placeholder  = "e.g. 30, 60, 90",
                        default_value = "30",
                    },
                },
                submit_label = "Start",
            },
        }
    end,
})
