-- Bitwarden: Generate Password — configurable password/passphrase generator.
-- Uses `bw generate` for policy-compliant output.

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
                return {
                    title = "Generate Password",
                    items = { { label = "bw generate returned no output", icon = "!" } },
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
                return { title = "Regenerate", items = { { label = "bw generate returned no output", icon = "!" } } }
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
        return { title = "Generate Password", items = { { label = "Unknown action", icon = "!" } } }
    end,
})
