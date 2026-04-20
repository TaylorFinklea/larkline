-- Bitwarden: Lock Vault — destroy the active session key.

lark.register({
    on_run = function()
        return {
            title = "Lock Vault",
            items = {
                {
                    label = "Lock Bitwarden vault",
                    detail = "Destroys the active session key. You'll need to unlock again.",
                    icon = "🔒",
                    actions = {
                        {
                            label = "Lock now",
                            kind = "shell",
                            args = { "bw", "lock" },
                            confirm = true,
                        },
                    },
                },
            },
        }
    end,
})
