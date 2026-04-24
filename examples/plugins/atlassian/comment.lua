-- Atlassian: Comment on Issue — placeholder until Phase D.
lark.register({
    on_run = function()
        return {
            title = "Comment on Issue",
            items = {
                { label = "Coming in v0.12.0 Phase D", detail = "Add a comment to a Jira issue", icon = "🚧" },
            },
        }
    end,
})
