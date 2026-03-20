-- Hello World — minimal Lua plugin.
-- Demonstrates the lark.register() + on_run pattern.
-- lark.run_command() below uses tokio::process::Command with explicit args (safe, no shell).

lark.register({
    on_run = function()
        return {
            title = "Hello from Lua!",
            items = {
                {
                    label = "Greeting",
                    detail = "This plugin runs inside an embedded Lua 5.4 VM",
                    icon = "L",
                },
                {
                    label = "Hostname",
                    detail = lark.exec("hostname"),
                    icon = "H",
                },
            },
        }
    end,
})
