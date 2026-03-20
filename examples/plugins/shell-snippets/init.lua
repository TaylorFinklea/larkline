-- Shell Snippets — run saved shell commands with confirmation.
-- Edit the snippets table below to customize.
-- Demonstrates shell actions with confirm = true.

-- Add your own snippets here. Each entry needs:
--   label:   display name
--   command: the executable to run
--   args:    arguments (optional, default empty)
local snippets = {
    { label = "Disk usage",     command = "df",     args = { "-h" } },
    { label = "Git status",     command = "git",    args = { "status", "--short" } },
    { label = "Docker containers", command = "docker", args = { "ps", "--format", "table {{.Names}}\t{{.Status}}\t{{.Ports}}" } },
    { label = "Homebrew update", command = "brew",   args = { "update" } },
    { label = "System uptime",  command = "uptime" },
}

lark.register({
    on_run = function()
        local items = {}
        for _, s in ipairs(snippets) do
            local cmd_args = { s.command }
            if s.args then
                for _, a in ipairs(s.args) do
                    table.insert(cmd_args, a)
                end
            end
            table.insert(items, {
                label = s.label,
                detail = s.command .. " " .. table.concat(s.args or {}, " "),
                icon = ">",
                actions = {
                    {
                        label = "Run: " .. s.label,
                        command = "shell",
                        args = cmd_args,
                        confirm = true,
                    },
                },
            })
        end

        return { title = "Shell Snippets", items = items }
    end,
})
