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

            local raw = lark.exec("bw", args)
            if not raw or raw == "" then
                local translated = from_exit(raw or "", {
                    cli = "bw",
                    service = "Bitwarden",
                    install_url = BW_HELP_URL,
                    login_help_url = BW_LOCKED_HELP_URL,
                })
                return {
                    title = "Generate Password",
                    items = { translated or error_item({ label = "bw generate returned no output", help_url = BW_HELP_URL }) },
                }
            end
            local pw = raw:gsub("%s+$", "")

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
                        actions = { { label = "Regenerate", kind = "chain", args = { "regenerate", "" } } },
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

    on_action = function(callback_id, _context)
        if callback_id == "regenerate" then
            local raw = lark.exec("bw", { "generate", "--length", "20", "--uppercase", "--lowercase", "--number" })
            local pw = (raw or ""):gsub("%s+$", "")
            if pw == "" then
                local translated = from_exit(raw or "", {
                    cli = "bw",
                    service = "Bitwarden",
                    install_url = BW_HELP_URL,
                    login_help_url = BW_LOCKED_HELP_URL,
                })
                return { title = "Regenerate", items = { translated or error_item({ label = "bw generate returned no output", help_url = BW_HELP_URL }) } }
            end
            return {
                title = "Generated Password",
                items = {
                    {
                        label = pw,
                        detail = "20 chars",
                        icon = "🔑",
                        copy_text = pw,
                        actions = { { label = "Copy to clipboard", kind = "clipboard", args = { pw } } },
                    },
                    {
                        label = "Generate another",
                        icon = "🔄",
                        actions = { { label = "Regenerate", kind = "chain", args = { "regenerate", "" } } },
                    },
                },
            }
        end
        return { title = "Generate Password", items = { error_item({ label = "Unknown action", help_url = BW_HELP_URL }) } }
    end,
})
