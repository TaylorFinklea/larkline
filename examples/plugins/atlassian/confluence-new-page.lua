-- Atlassian: New Confluence Page — placeholder until Phase E.
lark.register({
    on_run = function()
        return {
            title = "New Page",
            items = {
                { label = "Coming in v0.12.0 Phase E", detail = "Create a new Confluence page", icon = "🚧" },
            },
        }
    end,
})
