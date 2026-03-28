-- Translate — translate text using translate-shell (trans) CLI.
-- Install: brew install translate-shell

lark.register({
    on_run = function()
        if lark.form_values then
            local text = lark.form_values.text or ""
            local target = lark.form_values.target or "es"

            if text == "" then
                return { title = "Translate", items = { { label = "No text entered", icon = "!" } } }
            end

            -- Check if trans is installed.
            local version = lark.exec("trans", { "-V" })
            if not version or version == "" then
                return {
                    title = "Translate",
                    items = {
                        { label = "translate-shell not installed", detail = "brew install translate-shell", icon = "!" },
                    },
                }
            end

            local result = lark.exec("trans", { "-brief", "-no-ansi", ":" .. target, text })
            if not result or result == "" then
                return { title = "Translate", items = { { label = "Translation failed", icon = "!" } } }
            end

            result = result:gsub("%s+$", "")
            return {
                title = "Translate",
                items = {
                    {
                        label = result,
                        detail = text .. " → " .. target,
                        icon = "🌍",
                        copy_text = result,
                        actions = { { label = "Copy translation", kind = "clipboard", args = { result } } },
                    },
                },
            }
        end

        return {
            title = "Translate",
            form = {
                fields = {
                    {
                        id = "text",
                        label = "Text",
                        type = { kind = "text" },
                        required = true,
                        placeholder = "Text to translate",
                    },
                    {
                        id = "target",
                        label = "Target Language",
                        type = { kind = "select", options = { "es", "fr", "de", "ja", "ko", "zh", "pt", "it", "ru", "ar" } },
                        default = "es",
                    },
                },
                submit_label = "Translate",
            },
        }
    end,
})
