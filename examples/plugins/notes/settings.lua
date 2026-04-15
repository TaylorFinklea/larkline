-- Notes: Settings — configure vault path.

lark.register({
    on_run = function()
        local current = lark.store.get("notes_vault_path") or ""

        -- Handle form submission.
        if lark.form_values and lark.form_values.vault_path then
            local new_path = lark.form_values.vault_path
            if new_path ~= "" then
                local check = lark.exec("test", { "-d", new_path })
                if check ~= nil then
                    lark.store.set("notes_vault_path", new_path)
                    current = new_path
                    return {
                        title = "Notes Settings",
                        items = {
                            { label = "Vault path updated", detail = new_path, icon = "✅" },
                        },
                        form = {
                            fields = { { id = "vault_path", label = "Vault path", type = "text" } },
                            submit_label = "Update",
                        },
                    }
                else
                    return {
                        title = "Notes Settings",
                        items = {
                            { label = "Directory not found", detail = new_path, icon = "!" },
                            { label = "Current: " .. (current ~= "" and current or "(none)"), icon = "📁" },
                        },
                        form = {
                            fields = { { id = "vault_path", label = "Vault path", type = "text" } },
                            submit_label = "Update",
                        },
                    }
                end
            end
        end

        local items = {}
        if current ~= "" then
            items[#items + 1] = { label = "Current vault", detail = current, icon = "📁" }

            -- Count notes in vault.
            local count_raw = lark.exec("sh", { "-c", "find '" .. current .. "' -name '*.md' -not -path '*/.*' | wc -l" })
            local count = tonumber(count_raw and count_raw:gsub("%s+", "") or "0") or 0
            items[#items + 1] = { label = count .. " markdown notes", icon = "📄" }
        else
            items[#items + 1] = { label = "No vault configured", detail = "Enter a path below", icon = "!" }
        end

        items[#items + 1] = { label = "Tip: use ~/Documents/Obsidian for default", icon = "💡" }

        return {
            title = "Notes Settings",
            items = items,
            form = {
                fields = { { id = "vault_path", label = "Vault path (absolute)", type = "text" } },
                submit_label = "Update vault path",
            },
        }
    end,
})
