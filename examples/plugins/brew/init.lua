-- Brew — list installed Homebrew formulae with versions.

lark.register({
    on_run = function()
        local raw = lark.exec("brew", { "list", "--formula", "--versions" })

        if not raw or raw == "" then
            return {
                title = "Brew",
                items = { { label = "No packages or brew not found", icon = "📭" } },
            }
        end

        local items = {}
        for line in raw:gmatch("[^\n]+") do
            local name, versions = line:match("^(%S+)%s+(.+)$")
            if name then
                items[#items + 1] = {
                    label = name,
                    detail = versions,
                    icon = "📦",
                    copy_text = name,
                    actions = {
                        { label = "Upgrade", kind = "shell", args = { "brew", "upgrade", name }, confirm = true },
                        { label = "Uninstall", kind = "shell", args = { "brew", "uninstall", name }, confirm = true },
                        { label = "Info", kind = "shell", args = { "open", "https://formulae.brew.sh/formula/" .. name } },
                        { label = "Copy name", kind = "clipboard", args = { name } },
                    },
                }
            end
        end

        return { title = "Brew — " .. #items .. " packages", items = items }
    end,
})
