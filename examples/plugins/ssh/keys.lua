-- SSH: Keys — SSH keys in ~/.ssh with fingerprints and types.

lark.register({
    on_run = function()
        local home = lark.env("HOME") or "/tmp"
        local ssh_dir = home .. "/.ssh"

        local raw = lark.exec("ls", { "-1", ssh_dir })
        if not raw or raw == "" then
            return {
                title = "SSH Keys",
                items = { { label = "No ~/.ssh directory found", icon = "!" } },
            }
        end

        local items = {}
        for file in raw:gmatch("[^\n]+") do
            if file:match("%.pub$") then
                local path = ssh_dir .. "/" .. file
                local base_name = file:gsub("%.pub$", "")

                local fp_raw = lark.exec("ssh-keygen", { "-l", "-f", path })
                local bits, fingerprint, key_type
                if fp_raw then
                    bits = fp_raw:match("^(%d+)")
                    fingerprint = fp_raw:match("(SHA256:%S+)")
                    key_type = fp_raw:match("%((%S+)%)%s*$")
                end

                local detail_parts = {}
                if key_type then detail_parts[#detail_parts + 1] = key_type end
                if bits then detail_parts[#detail_parts + 1] = bits .. " bits" end

                local icon = "🔐"
                if key_type == "ED25519" then icon = "★"
                elseif key_type == "RSA" then icon = "◆"
                elseif key_type == "ECDSA" then icon = "◇"
                end

                items[#items + 1] = {
                    label = base_name,
                    detail = #detail_parts > 0 and table.concat(detail_parts, "  ") or nil,
                    icon = icon,
                    copy_text = fingerprint or base_name,
                    actions = {
                        { label = "Copy fingerprint", kind = "clipboard", args = { fingerprint or "?" } },
                        { label = "Copy public key (pbcopy)", kind = "shell", args = { "sh", "-c", "cat '" .. path .. "' | pbcopy" } },
                    },
                }
            end
        end

        local agent_raw = lark.exec("ssh-add", { "-l" })
        local loaded_count = 0
        if agent_raw and not agent_raw:match("no identities") then
            for _ in agent_raw:gmatch("[^\n]+") do
                loaded_count = loaded_count + 1
            end
        end

        if #items == 0 then
            return {
                title = "SSH Keys",
                items = { { label = "No SSH keys found", detail = "Run: ssh-keygen -t ed25519", icon = "📭" } },
            }
        end

        local title = "SSH Keys — " .. #items
        if loaded_count > 0 then
            title = title .. " (" .. loaded_count .. " in agent)"
        end

        return { title = title, items = items }
    end,
})
