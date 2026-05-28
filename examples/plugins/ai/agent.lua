-- AI Agent — multi-turn agent that calls safe agent-callable plugins as tools.
--
-- Architecture:
--   1. First run shows a form (prompt + optional system override).
--   2. Form submit shells out to `lark agent-ask` via lark.exec_io. The
--      CLI builds the AgentHarness, dispatches tool cycles, streams.
--   3. Render the final answer (with metadata about turns + tokens) in
--      the preview pane.
--
-- v1.0 safety stance: this plugin does NOT pass --yes. Destructive
-- tools (manifest `destructive = true`) are blocked by the default
-- approval hook and the agent finishes verbally. Users who want
-- destructive automation can run `lark agent-ask --yes "..."` in their
-- terminal directly — the trade-off is intentional: the TUI plugin
-- ships without an approval modal in v1.0.
--
-- Like ask.lua, this plugin shells to the CLI for one code path. The
-- streaming UX inside the TUI is deferred to Phase 6.5 (in-Lua
-- streaming primitive).

-- SHARED: error_item from examples/plugins/_shared/errors.lua.
local function error_item(opts)
    return {
        label = opts.label,
        detail = opts.detail,
        icon = opts.icon or "!",
        retry_action = opts.retry_action,
        help_url = opts.help_url,
    }
end

lark.register({
    on_run = function()
        if lark.form_values then
            local prompt = lark.form_values.prompt or ""
            local system = lark.form_values.system or ""

            if prompt == "" then
                return {
                    title = "AI Agent",
                    items = {
                        error_item({
                            label = "No prompt provided",
                            detail = "Submit a prompt to invoke the agent",
                        }),
                    },
                }
            end

            local lark_bin = lark.env("LARK_BINARY") or "lark"
            local args = { "agent-ask" }
            if system ~= "" then
                table.insert(args, "--system")
                table.insert(args, system)
            end
            table.insert(args, prompt)

            local result = lark.exec_io(lark_bin, args)

            if result.exit_code ~= 0 then
                local stderr = result.stderr or ""
                local label = "Agent run failed"
                local detail = stderr ~= ""
                    and stderr
                    or ("Unknown error (exit " .. tostring(result.exit_code) .. ")")
                local lower = stderr:lower()
                if lower:find("api_key", 1, true) or lower:find("not set", 1, true) then
                    label = "AI provider key missing"
                    detail = "Run `lark secret set <KEY>`"
                elseif lower:find("rate", 1, true) and lower:find("limit", 1, true) then
                    label = "AI provider rate limited"
                    detail = "Try again or switch providers in config"
                end
                return {
                    title = "AI Agent",
                    items = {
                        error_item({
                            label = label,
                            detail = detail,
                            icon = "❌",
                            help_url = "https://github.com/TaylorFinklea/larkline/blob/main/docs/AI_INTEGRATION.md",
                        }),
                    },
                }
            end

            -- Render the answer as markdown in the output pane. The
            -- response is long-form prose (often with markdown), so a
            -- list row's single-line detail mangles it — raw_text +
            -- output_format = "markdown" is the right surface.
            -- A dimmed footer carries turn/token metadata parsed from
            -- the CLI's stderr line: "[turns=N tokens in=M out=K session=...]".
            local body = result.stdout or ""
            local turns, tok_in, tok_out, session =
                (result.stderr or ""):match("turns=(%d+) tokens in=(%d+) out=(%d+) session=(%S+)")
            if turns then
                body = body
                    .. "\n\n---\n"
                    .. "_Turns: " .. turns
                    .. " · Tokens: " .. tok_in .. " in / " .. tok_out .. " out"
                    .. " · Session: " .. (session or "?") .. "_"
            end

            return {
                title = "AI Agent",
                raw_text = body,
                output_format = "markdown",
                items = {},
            }
        end

        return {
            title = "AI Agent",
            form = {
                fields = {
                    {
                        id = "prompt",
                        label = "Prompt",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "What do you want the agent to do?",
                    },
                    {
                        id = "system",
                        label = "System prompt (optional)",
                        type = { kind = "text" },
                        placeholder = "Override the system prompt for this run",
                    },
                },
                submit_label = "Run agent",
            },
        }
    end,
})
