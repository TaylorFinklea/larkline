-- Kubernetes: Contexts — list and switch kubectl contexts.

lark.register({
    on_run = function()
        local raw = lark.exec("kubectl", { "config", "get-contexts", "--no-headers" })

        if not raw or raw == "" then
            return {
                title = "Kubernetes Contexts",
                items = { {
                    label = "No kubectl contexts found",
                    icon = "⚠",
                    detail = "Run: kubectl config set-context",
                } },
            }
        end

        local items = {}
        for line in raw:gmatch("[^\n]+") do
            if not line:match("%S") then goto continue end

            local is_current = line:sub(1, 1) == "*"
            -- Strip * marker and leading whitespace, then take first word as name
            local rest = line:gsub("^%*?%s+", "")
            local name = rest:match("^(%S+)")

            if name then
                items[#items + 1] = {
                    label = name,
                    detail = is_current and "current context" or "",
                    icon = is_current and "★" or "○",
                    copy_text = name,
                    actions = {
                        { label = "Use Context", kind = "shell", args = { "kubectl", "config", "use-context", name }, confirm = false },
                        { label = "Copy Name",   kind = "clipboard", args = { name } },
                    },
                }
            end

            ::continue::
        end

        if #items == 0 then
            return { title = "Kubernetes Contexts", items = { { label = "No contexts found", icon = "📭" } } }
        end

        return { title = "Contexts — " .. #items, items = items }
    end,
})
