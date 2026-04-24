-- Atlassian: Triage Queue — placeholder until Phase D.
lark.register({
    on_run = function()
        return {
            title = "Triage Queue",
            items = {
                { label = "Coming in v0.12.0 Phase D", detail = "Unassigned To-Do issues in the default project", icon = "🚧" },
            },
        }
    end,
})
