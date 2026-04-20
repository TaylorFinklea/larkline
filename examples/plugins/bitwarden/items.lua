-- Bitwarden: Search Vault — list all items, filter with the live search bar.
-- Shared helpers copied from lib.lua (the Lark sandbox has no require).

local function bw_session(title)
    local session = lark.env("BW_SESSION")
    if session and session ~= "" then return session, nil end
    return nil, {
        title = title,
        items = {
            { label = "Bitwarden vault is locked", detail = "Run `export BW_SESSION=$(bw unlock --raw)` and re-open larkline", icon = "🔒" },
            { label = "Not logged in?", detail = "Run `bw login` first", icon = "→" },
        },
    }
end

local function run_bw(session, args, title)
    local full = { "--session", session, "--response" }
    for _, a in ipairs(args) do full[#full + 1] = a end
    local raw = lark.exec("bw", full)
    if not raw or raw == "" then
        return nil, { title = title, items = { { label = "No response from bw CLI", icon = "!" } } }
    end
    local ok, parsed = pcall(lark.json.decode, raw)
    if not ok or type(parsed) ~= "table" then
        return nil, { title = title, items = { { label = "Failed to parse bw response", detail = raw:sub(1, 120), icon = "!" } } }
    end
    if not parsed.success then
        return nil, { title = title, items = { { label = "bw error: " .. (parsed.message or "unknown"), icon = "!" } } }
    end
    return parsed.data, nil
end

local function icon_for_type(t)
    if t == 1 then return "🔑" end
    if t == 2 then return "📝" end
    if t == 3 then return "💳" end
    if t == 4 then return "👤" end
    return "•"
end

local function redact(s)
    if not s or s == "" then return "" end
    return string.rep("•", math.min(#s, 12))
end

local function redact_enabled()
    local v = lark.store.get("redact_secrets")
    if v == "false" or v == false then return false end
    return true
end

local function primary_uri(item)
    if item.type ~= 1 or not item.login or not item.login.uris then return nil end
    local u = item.login.uris[1]
    if u and u.uri and u.uri ~= "" then return u.uri end
    return nil
end

local function build_item_actions(item)
    local actions = {}
    if item.type == 1 and item.login then
        if item.login.password and item.login.password ~= "" then
            actions[#actions + 1] = { label = "Copy Password", kind = "clipboard", args = { item.login.password } }
        end
        if item.login.username and item.login.username ~= "" then
            actions[#actions + 1] = { label = "Copy Username", kind = "clipboard", args = { item.login.username } }
        end
        if item.login.totp and item.login.totp ~= "" then
            actions[#actions + 1] = { label = "Copy TOTP Code", kind = "chain", args = { "copy_totp", item.id } }
        end
        if item.login.uris then
            for _, u in ipairs(item.login.uris) do
                if u.uri and u.uri ~= "" then
                    actions[#actions + 1] = { label = "Open " .. u.uri, kind = "open", args = { u.uri } }
                    actions[#actions + 1] = { label = "Copy URL", kind = "clipboard", args = { u.uri } }
                    break
                end
            end
        end
    elseif item.type == 2 and item.notes and item.notes ~= "" then
        actions[#actions + 1] = { label = "Copy Note", kind = "clipboard", args = { item.notes } }
    elseif item.type == 3 and item.card then
        if item.card.number and item.card.number ~= "" then
            actions[#actions + 1] = { label = "Copy Card Number", kind = "clipboard", args = { item.card.number } }
        end
        if item.card.code and item.card.code ~= "" then
            actions[#actions + 1] = { label = "Copy CVV", kind = "clipboard", args = { item.card.code } }
        end
        if item.card.cardholderName and item.card.cardholderName ~= "" then
            actions[#actions + 1] = { label = "Copy Cardholder Name", kind = "clipboard", args = { item.card.cardholderName } }
        end
        if item.card.expMonth and item.card.expYear then
            local exp = string.format("%02d/%s", tonumber(item.card.expMonth) or 0, item.card.expYear)
            actions[#actions + 1] = { label = "Copy Expiration", kind = "clipboard", args = { exp } }
        end
    elseif item.type == 4 and item.identity then
        local id = item.identity
        local pairs_list = {
            { "Email", id.email }, { "Username", id.username }, { "Phone", id.phone },
            { "SSN", id.ssn }, { "Passport Number", id.passportNumber }, { "License Number", id.licenseNumber },
        }
        for _, kv in ipairs(pairs_list) do
            if kv[2] and kv[2] ~= "" then
                actions[#actions + 1] = { label = "Copy " .. kv[1], kind = "clipboard", args = { kv[2] } }
            end
        end
    end

    if item.fields then
        for _, f in ipairs(item.fields) do
            if f.name and f.value and f.value ~= "" then
                actions[#actions + 1] = { label = "Copy " .. f.name, kind = "clipboard", args = { f.value } }
            end
        end
    end

    actions[#actions + 1] = { label = "View Details", kind = "chain", args = { "show_detail", item.id } }
    if item.notes and item.notes ~= "" and item.type ~= 2 then
        actions[#actions + 1] = { label = "Copy Note", kind = "clipboard", args = { item.notes } }
    end
    return actions
end

local function type_label(t)
    if t == 1 then return "Login" end
    if t == 2 then return "Note" end
    if t == 3 then return "Card" end
    if t == 4 then return "Identity" end
    return "Item"
end

local function item_to_row(item)
    local detail_parts = { type_label(item.type) }
    if item.type == 1 and item.login then
        if item.login.username and item.login.username ~= "" then
            detail_parts[#detail_parts + 1] = item.login.username
        end
        local uri = primary_uri(item)
        if uri then detail_parts[#detail_parts + 1] = uri end
    elseif item.type == 3 and item.card then
        if item.card.brand and item.card.brand ~= "" then
            detail_parts[#detail_parts + 1] = item.card.brand
        end
    end
    if item.favorite then detail_parts[#detail_parts + 1] = "⭐" end

    return {
        label = item.name or "Untitled",
        detail = table.concat(detail_parts, "  ·  "),
        icon = icon_for_type(item.type),
        copy_text = (item.type == 1 and item.login and item.login.password) or item.name,
        actions = build_item_actions(item),
    }
end

local function detail_output(item)
    local redact_on = redact_enabled()
    local lines = {}
    local function kv(k, v) if v and v ~= "" then lines[#lines + 1] = "- **" .. k .. ":** " .. v end end
    local function kvr(k, v)
        if v and v ~= "" then
            lines[#lines + 1] = "- **" .. k .. ":** " .. (redact_on and redact(v) or v)
        end
    end
    lines[#lines + 1] = "# " .. (item.name or "Untitled")
    lines[#lines + 1] = ""
    if item.type == 1 and item.login then
        kv("Username", item.login.username)
        kvr("Password", item.login.password)
        if item.login.totp and item.login.totp ~= "" then
            lines[#lines + 1] = "- **TOTP:** configured"
        end
        if item.login.uris then
            for i, u in ipairs(item.login.uris) do kv("URL " .. i, u.uri) end
        end
    elseif item.type == 3 and item.card then
        kv("Cardholder", item.card.cardholderName)
        kv("Brand", item.card.brand)
        kvr("Number", item.card.number)
        kvr("CVV", item.card.code)
        if item.card.expMonth and item.card.expYear then
            kv("Expires", string.format("%02d/%s", tonumber(item.card.expMonth) or 0, item.card.expYear))
        end
    elseif item.type == 4 and item.identity then
        local id = item.identity
        kv("First Name", id.firstName) kv("Middle Name", id.middleName) kv("Last Name", id.lastName)
        kv("Email", id.email) kv("Phone", id.phone) kv("Username", id.username)
        kvr("SSN", id.ssn) kvr("Passport", id.passportNumber) kvr("License", id.licenseNumber)
        kv("Address 1", id.address1) kv("Address 2", id.address2)
        kv("City", id.city) kv("State", id.state) kv("Postal Code", id.postalCode) kv("Country", id.country)
    end
    if item.fields and #item.fields > 0 then
        lines[#lines + 1] = ""
        lines[#lines + 1] = "## Custom Fields"
        for _, f in ipairs(item.fields) do
            if f.name then
                if f.type == 1 then kvr(f.name, f.value) else kv(f.name, f.value) end
            end
        end
    end
    if item.notes and item.notes ~= "" then
        lines[#lines + 1] = ""
        lines[#lines + 1] = "## Notes"
        lines[#lines + 1] = ""
        lines[#lines + 1] = item.notes
    end

    local panel = { item_to_row(item) }
    for _, a in ipairs(build_item_actions(item)) do
        if a.kind == "clipboard" then
            panel[#panel + 1] = { label = a.label, icon = "📋", actions = { a }, copy_text = a.args[1] }
        end
    end
    return {
        title = item.name or "Detail",
        raw_text = table.concat(lines, "\n"),
        output_format = "markdown",
        items = panel,
    }
end

lark.register({
    on_run = function()
        local session, err = bw_session("Search Vault")
        if err then return err end

        local data, rerr = run_bw(session, { "list", "items" }, "Search Vault")
        if rerr then return rerr end
        if type(data) ~= "table" then
            return { title = "Search Vault", items = { { label = "Empty response", icon = "!" } } }
        end

        local max_raw = lark.store.get("max_results")
        local max = tonumber(max_raw) or 100
        local items = {}
        for i, it in ipairs(data) do
            if i > max then break end
            items[#items + 1] = item_to_row(it)
        end
        if #items == 0 then
            items[#items + 1] = { label = "No items in vault", icon = "📭" }
        end

        return {
            title = string.format("Vault — %d item%s", #items, #items == 1 and "" or "s"),
            items = items,
        }
    end,

    on_action = function(callback_id, context)
        if callback_id == "show_detail" then
            local session, err = bw_session("Detail")
            if err then return err end
            local data, rerr = run_bw(session, { "get", "item", context }, "Detail")
            if rerr then return rerr end
            return detail_output(data)
        elseif callback_id == "copy_totp" then
            local session, err = bw_session("TOTP")
            if err then return err end
            local data, rerr = run_bw(session, { "get", "totp", context }, "TOTP")
            if rerr then return rerr end
            local code = tostring(data or ""):gsub("%s+$", "")
            if code == "" then
                return { title = "TOTP", items = { { label = "No TOTP for this item", icon = "!" } } }
            end
            return {
                title = "TOTP Code",
                items = {
                    {
                        label = code,
                        detail = "Current one-time code",
                        icon = "🔢",
                        copy_text = code,
                        actions = { { label = "Copy Code", kind = "clipboard", args = { code } } },
                    },
                },
            }
        end
        return { title = "Bitwarden", items = { { label = "Unknown action: " .. callback_id, icon = "!" } } }
    end,
})
