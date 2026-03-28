-- URL Decode — decode percent-encoded text.

lark.register({
    on_run = function()
        if lark.form_values then
            local text = lark.form_values.text or ""
            if text == "" then
                return { title = "URL Decode", items = { { label = "No text entered", icon = "!" } } }
            end
            -- Pure Lua URL decoding.
            local result = text:gsub("%%(%x%x)", function(hex)
                return string.char(tonumber(hex, 16))
            end)
            return {
                title = "URL Decode",
                items = {
                    { label = result, detail = "Decoded from: " .. text, icon = "📋", copy_text = result,
                      actions = { { label = "Copy", kind = "clipboard", args = { result } } } },
                },
            }
        end
        return {
            title = "URL Decode",
            form = {
                fields = { { id = "text", label = "Encoded", type = { kind = "text" }, required = true, placeholder = "URL-encoded string to decode" } },
                submit_label = "Decode",
            },
        }
    end,
})
