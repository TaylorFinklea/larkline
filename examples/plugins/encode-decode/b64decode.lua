-- Base64 Decode — decode Base64 text.

lark.register({
    on_run = function()
        if lark.form_values then
            local text = lark.form_values.text or ""
            if text == "" then
                return { title = "Base64 Decode", items = { { label = "No text entered", icon = "!" } } }
            end
            local result = lark.exec("sh", { "-c", "printf '%s' '" .. text:gsub("'", "'\\''") .. "' | base64 -d 2>&1" })
            if not result or result == "" then
                return { title = "Base64 Decode", items = { { label = "Decoding failed — invalid Base64?", icon = "!" } } }
            end
            result = result:gsub("%s+$", "")
            return {
                title = "Base64 Decode",
                items = {
                    { label = result, detail = "Decoded from: " .. text, icon = "📋", copy_text = result,
                      actions = { { label = "Copy", kind = "clipboard", args = { result } } } },
                },
            }
        end
        return {
            title = "Base64 Decode",
            form = {
                fields = { { id = "text", label = "Base64", type = { kind = "text" }, required = true, placeholder = "Base64 string to decode" } },
                submit_label = "Decode",
            },
        }
    end,
})
