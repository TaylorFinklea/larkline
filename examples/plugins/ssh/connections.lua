-- SSH: Connections — active SSH sessions on this machine.

lark.register({
    on_run = function()
        local raw = lark.exec("ps", { "-eo", "pid,etime,args" })
        if not raw or raw == "" then
            return {
                title = "SSH Connections",
                items = { { label = "Unable to list processes", icon = "!" } },
            }
        end

        local items = {}
        for line in raw:gmatch("[^\n]+") do
            local pid, elapsed, cmd = line:match("^%s*(%d+)%s+(%S+)%s+(ssh%s+.+)$")
            if pid and cmd and not cmd:match("ssh%-agent") and not cmd:match("sshd") then
                local target = cmd:match("ssh%s+%S*@?(%S+)$") or cmd:match("ssh%s+(%S+)") or "?"

                items[#items + 1] = {
                    label = target,
                    detail = "PID:" .. pid .. "  uptime:" .. elapsed,
                    icon = "▶",
                    copy_text = target,
                    actions = {
                        { label = "Kill connection", kind = "shell", args = { "kill", pid }, confirm = true },
                        { label = "Copy PID", kind = "clipboard", args = { pid } },
                        { label = "Copy host", kind = "clipboard", args = { target } },
                    },
                }
            end
        end

        if #items == 0 then
            return {
                title = "SSH Connections",
                items = { { label = "No active SSH connections", icon = "📭" } },
            }
        end

        return { title = "SSH Connections — " .. #items, items = items }
    end,
})
