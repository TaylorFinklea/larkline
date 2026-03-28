-- Base64 Encode — encode text to Base64 using the system base64 command.

lark.register({
    on_run = function()
        if lark.form_values then
            local text = lark.form_values.text or ""
            if text == "" then
                return { title = "Base64 Encode", items = { { label = "No text entered", icon = "!" } } }
            end
            local result = lark.exec("sh", { "-c", "printf '%s' '" .. text:gsub("'", "'\\''") .. "' | base64" })
            if not result or result == "" then
                return { title = "Base64 Encode", items = { { label = "Encoding failed", icon = "!" } } }
            end
            result = result:gsub("%s+$", "")
            return {
                title = "Base64 Encode",
                items = {
                    { label = result, detail = "Encoded from: " .. text, icon = "📋", copy_text = result,
                      actions = { { label = "Copy", kind = "clipboard", args = { result } } } },
                },
            }
        end
        return {
            title = "Base64 Encode",
            form = {
                fields = { { id = "text", label = "Text", type = { kind = "text" }, required = true, placeholder = "Text to encode" } },
                submit_label = "Encode",
            },
        }
    end,
})
