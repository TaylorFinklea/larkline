-- Bitwarden: Generate Password — configurable password/passphrase generator.
-- Uses `bw generate` for policy-compliant output.

local BW_HELP_URL = "https://bitwarden.com/help/cli/"
local BW_LOCKED_HELP_URL = "https://bitwarden.com/help/cli/#using-the-cli"

-- SHARED: error_item — canonical copy in examples/plugins/_shared/errors.lua.
local function error_item(opts)
    return {
        label = opts.label,
        detail = opts.detail,
        icon = opts.icon or "!",
        retry_action = opts.retry_action,
        help_url = opts.help_url,
    }
end

-- SHARED: from_exit — canonical copy in examples/plugins/_shared/errors.lua.
local function from_exit(stderr, hints)
    hints = hints or {}
    stderr = stderr or ""
    local lower = stderr:lower()

    if lower:find("command not found", 1, true)
        or lower:find("no such file or directory", 1, true) then
        local cli = hints.cli or "command"
        local detail
        if hints.install_url then
            detail = "Install: " .. hints.install_url
        else
            detail = "Check your $PATH"
        end
        return error_item({
            label = cli .. " not found",
            detail = detail,
            help_url = hints.install_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("401", 1, true)
        or lower:find("403", 1, true)
        or lower:find("unauthorized", 1, true)
        or lower:find("forbidden", 1, true)
        or lower:find("not authenticated", 1, true)
        or lower:find("not logged in", 1, true)
        or lower:find("authentication required", 1, true) then
        local detail
        if hints.login_command then
            detail = "Run `" .. hints.login_command .. "`"
        else
            detail = "Check credentials"
        end
        return error_item({
            label = (hints.service or "Service") .. " auth failed",
            detail = detail,
            help_url = hints.login_help_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("429", 1, true)
        or lower:find("rate limit", 1, true)
        or lower:find("too many requests", 1, true) then
        local retry_after = stderr:match("[Rr]etry%-[Aa]fter:?%s*(%d+)")
        local detail
        if retry_after then
            detail = "Rate limited — retry in " .. retry_after .. "s"
        else
            detail = "Rate limited — try again later"
        end
        return error_item({
            label = (hints.service or "Service") .. " rate limited",
            detail = detail,
            help_url = hints.login_help_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("could not resolve host", 1, true)
        or lower:find("getaddrinfo", 1, true)
        or lower:find("name or service not known", 1, true)
        or lower:find("connection refused", 1, true)
        or lower:find("network is unreachable", 1, true)
        or lower:find("no route to host", 1, true) then
        return error_item({
            label = "Network unreachable",
            detail = "Check your connection",
            retry_action = hints.retry_action,
        })
    end

    return nil
end

lark.register({
    on_run = function()
        if lark.form_values then
            local fv = lark.form_values
            local mode = fv.mode or "password"

            local args = { "generate" }
            if mode == "passphrase" then
                table.insert(args, "--passphrase")
                local words = tonumber(fv.words) or 4
                table.insert(args, "--words")
                table.insert(args, tostring(words))
                table.insert(args, "--separator")
                table.insert(args, fv.separator or "-")
                if fv.capitalize == "true" then table.insert(args, "--capitalize") end
                if fv.include_number == "true" then table.insert(args, "--includeNumber") end
            else
                local length = tonumber(fv.length) or 20
                table.insert(args, "--length")
                table.insert(args, tostring(length))
                if fv.uppercase ~= "false" then table.insert(args, "--uppercase") end
                if fv.lowercase ~= "false" then table.insert(args, "--lowercase") end
                if fv.numbers ~= "false" then table.insert(args, "--number") end
                if fv.symbols == "true" then table.insert(args, "--special") end
                if fv.minnumber and tonumber(fv.minnumber) then
                    table.insert(args, "--minNumber"); table.insert(args, fv.minnumber)
                end
                if fv.minspecial and tonumber(fv.minspecial) then
                    table.insert(args, "--minSpecial"); table.insert(args, fv.minspecial)
                end
                if fv.ambiguous == "true" then table.insert(args, "--ambiguous") end
            end

            local res = lark.exec_io("bw", args)
            if res.exit_code ~= 0 or res.stdout == "" then
                local translated = from_exit(res.stderr, {
                    cli = "bw",
                    service = "Bitwarden",
                    login_command = "bw login",
                    install_url = BW_HELP_URL,
                    login_help_url = BW_LOCKED_HELP_URL,
                })
                return {
                    title = "Generate Password",
                    level = "warn",
                    items = { translated or error_item({ label = "bw generate returned no output", help_url = BW_HELP_URL }) },
                }
            end
            local pw = res.stdout:gsub("%s+$", "")

            -- Pack the exact bw args (minus the leading "generate" verb) into
            -- the chain context so "Generate another" repeats the user's
            -- options instead of falling back to defaults. The engine joins
            -- chain args[1..] with spaces into one context string, so encode
            -- as JSON to round-trip cleanly.
            local repeat_args = {}
            for i = 2, #args do repeat_args[#repeat_args + 1] = args[i] end
            local ctx_json, _ = lark.json.encode({ mode = mode, args = repeat_args })

            return {
                title = mode == "passphrase" and "Generated Passphrase" or "Generated Password",
                items = {
                    {
                        label = pw,
                        detail = mode == "passphrase"
                            and (fv.words .. " words, separator `" .. (fv.separator or "-") .. "`")
                            or  (fv.length .. " chars"),
                        icon = "🔑",
                        copy_text = pw,
                        actions = { { label = "Copy to clipboard", kind = "clipboard", args = { pw } } },
                    },
                    {
                        label = "Generate another",
                        detail = "Re-run with the same settings",
                        icon = "🔄",
                        actions = { { label = "Regenerate", kind = "chain", args = { "regenerate", ctx_json or "{}" } } },
                    },
                },
            }
        end

        return {
            title = "Generate Password",
            form = {
                fields = {
                    { id = "mode", label = "Mode",
                      type = { kind = "select", options = { "password", "passphrase" } },
                      default_value = "password" },
                    { id = "length", label = "Password length (8–128)",
                      type = { kind = "text" }, default_value = "20" },
                    { id = "uppercase", label = "Include uppercase (A-Z)",
                      type = { kind = "toggle" }, default_value = "true" },
                    { id = "lowercase", label = "Include lowercase (a-z)",
                      type = { kind = "toggle" }, default_value = "true" },
                    { id = "numbers", label = "Include numbers (0-9)",
                      type = { kind = "toggle" }, default_value = "true" },
                    { id = "symbols", label = "Include symbols (!@#$%^&*)",
                      type = { kind = "toggle" }, default_value = "false" },
                    { id = "ambiguous", label = "Allow ambiguous chars (l1Io0)",
                      type = { kind = "toggle" }, default_value = "false" },
                    { id = "minnumber", label = "Min numbers (optional)",
                      type = { kind = "text" }, placeholder = "blank = default" },
                    { id = "minspecial", label = "Min symbols (optional)",
                      type = { kind = "text" }, placeholder = "blank = default" },
                    { id = "words", label = "Passphrase word count",
                      type = { kind = "text" }, default_value = "4" },
                    { id = "separator", label = "Passphrase separator",
                      type = { kind = "text" }, default_value = "-" },
                    { id = "capitalize", label = "Capitalize passphrase words",
                      type = { kind = "toggle" }, default_value = "false" },
                    { id = "include_number", label = "Include number in passphrase",
                      type = { kind = "toggle" }, default_value = "false" },
                },
                submit_label = "Generate",
            },
        }
    end,

    on_action = function(callback_id, context)
        if callback_id == "regenerate" then
            -- Decode the original options packed in the chain context. Falls
            -- back to bw defaults only when the context is missing/garbled.
            local ok, decoded = pcall(lark.json.decode, context or "{}")
            local mode = "password"
            local bw_args = { "generate" }
            if ok and type(decoded) == "table" and type(decoded.args) == "table" then
                mode = decoded.mode or "password"
                for _, a in ipairs(decoded.args) do bw_args[#bw_args + 1] = a end
            else
                -- Legacy fallback: 20-char password with standard charset.
                for _, a in ipairs({ "--length", "20", "--uppercase", "--lowercase", "--number" }) do
                    bw_args[#bw_args + 1] = a
                end
            end

            local res = lark.exec_io("bw", bw_args)
            local pw = (res.stdout or ""):gsub("%s+$", "")
            if res.exit_code ~= 0 or pw == "" then
                local translated = from_exit(res.stderr, {
                    cli = "bw",
                    service = "Bitwarden",
                    login_command = "bw login",
                    install_url = BW_HELP_URL,
                    login_help_url = BW_LOCKED_HELP_URL,
                })
                return { title = "Regenerate", level = "warn", items = { translated or error_item({ label = "bw generate returned no output", help_url = BW_HELP_URL }) } }
            end

            -- Derive a detail line that reflects the actual options used so
            -- the user can see at a glance that their settings were honored.
            local detail
            if mode == "passphrase" then
                local words, sep
                for i, a in ipairs(bw_args) do
                    if a == "--words" then words = bw_args[i + 1] end
                    if a == "--separator" then sep = bw_args[i + 1] end
                end
                detail = (words or "?") .. " words, separator `" .. (sep or "-") .. "`"
            else
                local length
                for i, a in ipairs(bw_args) do
                    if a == "--length" then length = bw_args[i + 1] end
                end
                detail = (length or "?") .. " chars"
            end

            return {
                title = mode == "passphrase" and "Generated Passphrase" or "Generated Password",
                items = {
                    {
                        label = pw,
                        detail = detail,
                        icon = "🔑",
                        copy_text = pw,
                        actions = { { label = "Copy to clipboard", kind = "clipboard", args = { pw } } },
                    },
                    {
                        label = "Generate another",
                        detail = "Re-run with the same settings",
                        icon = "🔄",
                        actions = { { label = "Regenerate", kind = "chain", args = { "regenerate", context or "{}" } } },
                    },
                },
            }
        end
        return { title = "Generate Password", items = { error_item({ label = "Unknown action", help_url = BW_HELP_URL }) } }
    end,
})
