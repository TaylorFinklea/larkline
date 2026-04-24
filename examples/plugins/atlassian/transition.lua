-- Atlassian: Transition Issue — placeholder until Phase D.
lark.register({
    on_run = function()
        return {
            title = "Transition Issue",
            items = {
                { label = "Coming in v0.12.0 Phase D", detail = "Move an issue to a different workflow state", icon = "🚧" },
            },
        }
    end,
})
