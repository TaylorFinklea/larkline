-- AI Ask — single-shot prompt against the configured [ai] provider.
--
-- Architecture:
--   1. First run shows a form (prompt + optional system override).
--   2. Form submit shells out to `lark ai-ask` via lark.exec_io, which
--      streams the response from the Provider trait. We block on the
--      whole response (no in-plugin streaming yet — that's Phase 6.5).
--   3. Render the response as a copyable item, with token usage as a
--      secondary detail row and a re-ask action.
--
-- The CLI handles all the provider logic (key resolution, streaming,
-- usage tracking). This plugin is intentionally thin — its job is the
-- TUI form + result rendering.

-- SHARED: error_item from examples/plugins/_shared/errors.lua.
-- Inlined because the lark sandbox has no require/dofile.
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
        ----------------------------------------------------------------
        -- Step 1 — form submission path.
        ----------------------------------------------------------------
        if lark.form_values then
            local prompt = lark.form_values.prompt or ""
            local system = lark.form_values.system or ""

            if prompt == "" then
                return {
                    title = "AI Ask",
                    items = {
                        error_item({
                            label = "No prompt provided",
                            detail = "Submit a prompt to ask the AI",
                        }),
                    },
                }
            end

            -- Shell out to `lark ai-ask` so we reuse one code path for
            -- provider/key/streaming. We resolve the running binary via
            -- LARK_BINARY (injected at startup; works in dev + brew).
            local lark_bin = lark.env("LARK_BINARY") or "lark"
            local args = { "ai-ask" }
            if system ~= "" then
                table.insert(args, "--system")
                table.insert(args, system)
            end
            table.insert(args, prompt)

            local result = lark.exec_io(lark_bin, args)

            -- Error path: distinct icon + help_url to AI_INTEGRATION.md.
            -- Matches the v0.15.0 error UX pattern (error_item from
            -- _shared/errors.lua, status-aware docs link).
            if result.exit_code ~= 0 then
                local stderr = result.stderr or ""
                local label = "AI request failed"
                local detail = stderr ~= ""
                    and stderr
                    or ("Unknown error (exit " .. tostring(result.exit_code) .. ")")
                -- Friendly label for common cases — keeps the row scannable.
                local lower = stderr:lower()
                if lower:find("api_key", 1, true) or lower:find("not set", 1, true) then
                    label = "AI provider key missing"
                    detail = "Run `lark secret set <KEY>` (see help URL)"
                elseif lower:find("rate", 1, true) and lower:find("limit", 1, true) then
                    label = "AI provider rate limited"
                    detail = "Try again later, or switch providers in config"
                end
                return {
                    title = "AI Ask",
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

            -- Happy path: one row with the response. The prompt is the
            -- label so the result list reads as a chat history. The
            -- response goes into `detail` (rendered in the preview pane).
            -- `copy_text` makes Cmd-C / `c` copy the full response.
            -- Token usage is shown as a secondary dim row so successful
            -- runs are visibly accounted-for without being noisy.
            local items = {
                {
                    label = prompt,
                    icon = "🤖",
                    detail = result.stdout,
                    copy_text = result.stdout,
                },
            }
            -- Token usage goes to stderr from the CLI (one line:
            -- "[tokens in=N out=N]"). Parse and surface as a small row.
            local in_tok, out_tok = (result.stderr or ""):match("tokens in=(%d+) out=(%d+)")
            if in_tok and out_tok then
                table.insert(items, {
                    label = "Tokens: " .. in_tok .. " in / " .. out_tok .. " out",
                    icon = "📊",
                    detail = "Reported by the AI provider",
                })
            end

            return {
                title = "AI Ask",
                items = items,
            }
        end

        ----------------------------------------------------------------
        -- Step 2 — first run: show the form.
        --
        -- Minimal field set: prompt (required) + optional system
        -- override. Model + max_tokens omitted by design — advanced
        -- users can `lark ai-ask --model X --max-tokens N "prompt"` in
        -- the terminal directly.
        ----------------------------------------------------------------
        return {
            title = "AI Ask",
            form = {
                fields = {
                    {
                        id = "prompt",
                        label = "Prompt",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "What do you want to ask?",
                    },
                    {
                        id = "system",
                        label = "System prompt (optional)",
                        type = { kind = "text" },
                        placeholder = "Override the system prompt for this call",
                    },
                },
                submit_label = "Ask",
            },
        }
    end,
})
