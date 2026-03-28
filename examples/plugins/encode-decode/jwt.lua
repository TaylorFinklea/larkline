-- JWT Decode — decode a JWT token's header and payload (no verification).

lark.register({
    on_run = function()
        if lark.form_values then
            local token = lark.form_values.token or ""
            if token == "" then
                return { title = "JWT Decode", items = { { label = "No token entered", icon = "!" } } }
            end

            local parts = {}
            for part in token:gmatch("[^%.]+") do
                parts[#parts + 1] = part
            end

            if #parts < 2 then
                return { title = "JWT Decode", items = { { label = "Invalid JWT — expected 3 dot-separated parts", icon = "!" } } }
            end

            local items = {}
            local labels = { "Header", "Payload" }
            for i = 1, 2 do
                -- Base64url to Base64: replace - with +, _ with /, pad with =.
                local b64 = parts[i]:gsub("-", "+"):gsub("_", "/")
                local pad = (4 - #b64 % 4) % 4
                b64 = b64 .. ("="):rep(pad)

                local decoded = lark.exec("sh", { "-c", "printf '%s' '" .. b64:gsub("'", "'\\''") .. "' | base64 -d 2>/dev/null" })
                if decoded and decoded ~= "" then
                    local pretty = lark.exec("sh", { "-c", "printf '%s' '" .. decoded:gsub("'", "'\\''") .. "' | python3 -m json.tool 2>/dev/null" })
                    local display = (pretty and pretty ~= "") and pretty or decoded
                    items[#items + 1] = {
                        label = labels[i],
                        detail = display:gsub("%s+$", ""),
                        icon = i == 1 and "📋" or "📦",
                        copy_text = display:gsub("%s+$", ""),
                        actions = { { label = "Copy " .. labels[i], kind = "clipboard", args = { display:gsub("%s+$", "") } } },
                    }
                else
                    items[#items + 1] = { label = labels[i] .. " — decode failed", icon = "!" }
                end
            end

            return { title = "JWT Decode", items = items }
        end

        return {
            title = "JWT Decode",
            form = {
                fields = { { id = "token", label = "JWT Token", type = { kind = "text" }, required = true, placeholder = "eyJhbG..." } },
                submit_label = "Decode",
            },
        }
    end,
})
