-- Brew Update — run `brew update` then list outdated packages.

lark.register({
    on_run = function()
        -- Refresh Homebrew index first.
        lark.exec("brew", { "update" })

        local raw = lark.exec("brew", { "outdated", "--verbose" })

        if not raw or raw == "" then
            return {
                title = "Brew Update",
                items = { { label = "Everything is up to date", icon = "✅" } },
            }
        end

        local items = {}
        for line in raw:gmatch("[^\n]+") do
            -- Format: "package (installed) < available" or "package (installed) != available"
            local name, installed, available = line:match("^(%S+)%s+%((.-)%)%s+[<!=]+%s+(.+)$")
            if name then
                items[#items + 1] = {
                    label = name,
                    detail = installed .. " → " .. available,
                    icon = "⬆️",
                    copy_text = name,
                    actions = {
                        { label = "Upgrade", kind = "shell", args = { "brew", "upgrade", name }, confirm = true },
                        { label = "Info", kind = "shell", args = { "open", "https://formulae.brew.sh/formula/" .. name } },
                        { label = "Copy name", kind = "clipboard", args = { name } },
                    },
                }
            end
        end

        if #items == 0 then
            return {
                title = "Brew Update",
                items = { { label = "Everything is up to date", icon = "✅" } },
            }
        end

        return { title = "Brew Update — " .. #items .. " outdated", items = items }
    end,
})
