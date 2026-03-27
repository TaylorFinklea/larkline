-- Caffeinate: Extend — extend the current caffeinate session by N minutes.

lark.register({
    on_run = function()
        if lark.form_values then
            local raw_min = lark.form_values.minutes or ""
            local minutes = raw_min:match("^%s*(%d+)%s*$")
            if not minutes or tonumber(minutes) <= 0 then
                return {
                    title = "Extend Caffeinate",
                    items = { { label = "Enter a positive number of minutes", icon = "⚠" } },
                }
            end

            lark.exec("spotlight-caffeinate-cli", { "extend", minutes })

            return {
                title = "Extend Caffeinate",
                items = { {
                    label  = "Extended by " .. minutes .. " minutes",
                    detail = "Session time has been extended",
                    icon   = "☕",
                } },
            }
        end

        return {
            title = "Extend Caffeinate",
            form = {
                fields = {
                    {
                        id           = "minutes",
                        label        = "Extend by (minutes)",
                        type         = { kind = "text" },
                        required     = true,
                        placeholder  = "e.g. 30, 60, 90",
                        default_value = "30",
                    },
                },
                submit_label = "Extend",
            },
        }
    end,
})
