-- Atlassian: Confluence Recent — placeholder until Phase E.
lark.register({
    on_run = function()
        return {
            title = "Recent Pages",
            items = {
                { label = "Coming in v0.12.0 Phase E", detail = "Recently updated Confluence pages", icon = "🚧" },
            },
        }
    end,
})
