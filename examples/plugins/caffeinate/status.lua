-- Caffeinate: Status — current caffeinate state with stop action.
-- Requires spotlight-caffeinate-cli in PATH.

lark.register({
    on_run = function()
        local raw = lark.exec("spotlight-caffeinate-cli", { "status" })

        if not raw or raw == "" then
            return {
                title = "Caffeinate",
                items = { {
                    label  = "spotlight-caffeinate-cli not found",
                    detail = "Install Spotlight Caffeinate",
                    icon   = "⚠",
                } },
            }
        end

        local ok, status = pcall(lark.json.decode, raw)
        if not ok then
            return {
                title = "Caffeinate",
                items = { { label = raw:gsub("%s+$", ""), icon = "☕" } },
            }
        end

        local items = {}
        local is_active = status.active or status.is_active or false

        if is_active then
            local remaining = status.remaining_minutes or status.remaining or "?"
            items[#items + 1] = {
                label  = "Active — " .. tostring(remaining) .. " min remaining",
                detail = "Mac is staying awake",
                icon   = "☕",
            }
            items[#items + 1] = {
                label   = "Stop",
                icon    = "⏹",
                actions = {
                    { label = "Stop caffeinate", kind = "shell", args = { "spotlight-caffeinate-cli", "stop" }, confirm = true },
                },
            }
        else
            items[#items + 1] = {
                label  = "Inactive",
                detail = "Mac will sleep normally — use Start to activate",
                icon   = "💤",
            }
        end

        return { title = "Caffeinate", items = items }
    end,
})
