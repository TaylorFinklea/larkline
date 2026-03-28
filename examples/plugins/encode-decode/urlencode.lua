-- URL Encode — percent-encode text for URLs.

lark.register({
    on_run = function()
        if lark.form_values then
            local text = lark.form_values.text or ""
            if text == "" then
                return { title = "URL Encode", items = { { label = "No text entered", icon = "!" } } }
            end
            -- Pure Lua URL encoding.
            local result = text:gsub("([^%w%-%.%_%~])", function(c)
                return string.format("%%%02X", string.byte(c))
            end)
            return {
                title = "URL Encode",
                items = {
                    { label = result, detail = "Encoded from: " .. text, icon = "📋", copy_text = result,
                      actions = { { label = "Copy", kind = "clipboard", args = { result } } } },
                },
            }
        end
        return {
            title = "URL Encode",
            form = {
                fields = { { id = "text", label = "Text", type = { kind = "text" }, required = true, placeholder = "Text to URL-encode" } },
                submit_label = "Encode",
            },
        }
    end,
})
