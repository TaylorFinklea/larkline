-- Mail: Compose -- open a new Mail.app composer pre-filled from a form.
--
-- v1.0: simple two-field form (To, Subject). Body is left empty in the
-- composer for the user to type in Mail.app's native editor. After
-- composing, the user clicks Send in Mail.app. No programmatic SMTP --
-- this avoids both account-picking complexity and the moderation surface
-- of arbitrary outbound mail from a plugin.
--
-- The submit action creates a visible OutgoingMessage in Mail.app
-- (Mail.outgoingMessages.push) so the composer window appears in the
-- foreground for the user to complete. The action carries confirm=true
-- as belt-and-braces -- the dispatcher prompts before firing.

local function error_item(message, help_url)
    return { icon = "!", label = message, help_url = help_url, actions = {} }
end

local function js_str(s)
    local enc, _ = lark.json.encode(s)
    return enc or '""'
end

-- Build the JXA script that creates the Mail.app composer window.
-- visible:true causes Mail.app to bring the composer to the front.
local function build_compose_jxa(to, subject)
    return string.format([[
const Mail = Application("Mail");
const msg = Mail.OutgoingMessage({
    subject: %s,
    content: "",
    visible: true,
});
Mail.outgoingMessages.push(msg);
msg.toRecipients.push(Mail.ToRecipient({address: %s}));
JSON.stringify({opened: true});
]], js_str(subject or ""), js_str(to or ""))
end

lark.register({
    on_run = function()
        if lark.form_values then
            local to = (lark.form_values.to or ""):match("^%s*(.-)%s*$")
            local subject = (lark.form_values.subject or ""):match("^%s*(.-)%s*$")
            if to == "" then
                return {
                    title = "Compose",
                    items = { error_item("Recipient was empty — try again") },
                }
            end

            local script = build_compose_jxa(to, subject)
            return {
                title = "Compose",
                items = {
                    {
                        icon = "✉️ ",
                        label = "Open Mail.app composer to " .. to,
                        detail = subject ~= "" and ("Subject: " .. subject) or "(no subject)",
                        preview = "Subject: " .. subject .. "\nTo: " .. to ..
                            "\n\nPress Enter to open the Mail.app composer. Type the body in" ..
                            "\nMail.app and click Send when ready.",
                        actions = {
                            {
                                label = "Open composer in Mail.app",
                                kind = "shell",
                                args = { "osascript", "-l", "JavaScript", "-e", script },
                                confirm = true,
                            },
                            {
                                label = "Copy mailto: URL",
                                kind = "clipboard",
                                args = { "mailto:" .. to .. "?subject=" ..
                                    (subject ~= "" and subject:gsub(" ", "%%20") or "") },
                            },
                        },
                    },
                },
            }
        end

        return {
            title = "Compose",
            form = {
                fields = {
                    { id = "to", label = "To (recipient address)",
                      type = { kind = "text" }, placeholder = "name@example.com" },
                    { id = "subject", label = "Subject",
                      type = { kind = "text" }, placeholder = "subject line" },
                },
            },
        }
    end,
})
