-- Atlassian: Confluence Search — placeholder until Phase E.
lark.register({
    on_run = function()
        return {
            title = "Search Confluence",
            items = {
                { label = "Coming in v0.12.0 Phase E", detail = "Full-text search across Confluence (CQL)", icon = "🚧" },
            },
        }
    end,
})
