-- Bitwarden: Search Vault — list all items, filter with the live search bar.
-- Shared helpers copied from lib.lua (the Lark sandbox has no require).

local BW_HELP_URL = "https://bitwarden.com/help/cli/"
local BW_LOCKED_HELP_URL = "https://bitwarden.com/help/cli/#using-the-cli"

-- SHARED: error_item — canonical copy in examples/plugins/_shared/errors.lua.
local function error_item(opts)
    return {
        label = opts.label,
        detail = opts.detail,
        icon = opts.icon or "!",
        retry_action = opts.retry_action,
        help_url = opts.help_url,
    }
end

-- SHARED: from_exit — canonical copy in examples/plugins/_shared/errors.lua.
local function from_exit(stderr, hints)
    hints = hints or {}
    stderr = stderr or ""
    local lower = stderr:lower()

    if lower:find("command not found", 1, true)
        or lower:find("no such file or directory", 1, true) then
        local cli = hints.cli or "command"
        local detail
        if hints.install_url then
            detail = "Install: " .. hints.install_url
        else
            detail = "Check your $PATH"
        end
        return error_item({
            label = cli .. " not found",
            detail = detail,
            help_url = hints.install_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("401", 1, true)
        or lower:find("403", 1, true)
        or lower:find("unauthorized", 1, true)
        or lower:find("forbidden", 1, true)
        or lower:find("not authenticated", 1, true)
        or lower:find("not logged in", 1, true)
        or lower:find("authentication required", 1, true) then
        local detail
        if hints.login_command then
            detail = "Run `" .. hints.login_command .. "`"
        else
            detail = "Check credentials"
        end
        return error_item({
            label = (hints.service or "Service") .. " auth failed",
            detail = detail,
            help_url = hints.login_help_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("429", 1, true)
        or lower:find("rate limit", 1, true)
        or lower:find("too many requests", 1, true) then
        local retry_after = stderr:match("[Rr]etry%-[Aa]fter:?%s*(%d+)")
        local detail
        if retry_after then
            detail = "Rate limited — retry in " .. retry_after .. "s"
        else
            detail = "Rate limited — try again later"
        end
        return error_item({
            label = (hints.service or "Service") .. " rate limited",
            detail = detail,
            help_url = hints.login_help_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("could not resolve host", 1, true)
        or lower:find("getaddrinfo", 1, true)
        or lower:find("name or service not known", 1, true)
        or lower:find("connection refused", 1, true)
        or lower:find("network is unreachable", 1, true)
        or lower:find("no route to host", 1, true) then
        return error_item({
            label = "Network unreachable",
            detail = "Check your connection",
            retry_action = hints.retry_action,
        })
    end

    return nil
end

local function bw_help_url_for(msg)
    local lower = (msg or ""):lower()
    if lower:find("locked", 1, true)
        or lower:find("unlock", 1, true)
        or lower:find("bw_session", 1, true)
        or lower:find("not logged in", 1, true)
        or lower:find("login", 1, true) then
        return BW_LOCKED_HELP_URL
    end
    return BW_HELP_URL
end

local function bw_session(title)
    local session = lark.env("BW_SESSION")
    if session and session ~= "" then return session, nil end
    return nil, {
        title = title,
        items = {
            error_item({ label = "Bitwarden vault is locked", detail = "Run `export BW_SESSION=$(bw unlock --raw)` and re-open larkline", icon = "🔒", help_url = BW_LOCKED_HELP_URL }),
            error_item({ label = "Not logged in?", detail = "Run `bw login` first", icon = "→", help_url = BW_LOCKED_HELP_URL }),
        },
    }
end

-- bw --response wraps payloads in a discriminated envelope. Unwrap to the
-- useful payload so callers can work with it directly.
--   list:     { object = "list", data = [...] }
--   template: { object = "template", template = {...} }   (used by `status`)
--   string:   { object = "string", data = "..." }         (used by `get totp`)
--   item/folder/etc: { object = "item", ...fields inlined }
local function unwrap_bw(d)
    if type(d) ~= "table" then return d end
    if d.object == "list" then return d.data or {} end
    if d.object == "template" then return d.template or {} end
    if d.object == "string" then return d.data or "" end
    return d
end

local function run_bw(session, args, title)
    local full = { "--session", session, "--response" }
    for _, a in ipairs(args) do full[#full + 1] = a end
    local raw = lark.exec("bw", full)
    if not raw or raw == "" then
        local translated = from_exit(raw or "", {
            cli = "bw",
            service = "Bitwarden",
            install_url = BW_HELP_URL,
            login_help_url = BW_LOCKED_HELP_URL,
        })
        return nil, { title = title, items = { translated or error_item({ label = "No response from bw CLI", detail = "Is `bw` installed and on $PATH?", help_url = BW_HELP_URL }) } }
    end
    local ok, parsed = pcall(lark.json.decode, raw)
    if not ok or type(parsed) ~= "table" then
        return nil, { title = title, items = { error_item({ label = "Failed to parse bw response", detail = raw:sub(1, 120), help_url = BW_HELP_URL }) } }
    end
    if not parsed.success then
        local msg = parsed.message or "unknown"
        return nil, { title = title, items = { error_item({ label = "bw error: " .. msg, help_url = bw_help_url_for(msg) }) } }
    end
    return unwrap_bw(parsed.data), nil
end

local function verify_session(session, title)
    local data, err = run_bw(session, { "status" }, title)
    if err then return err end
    local status = (data and data.status) or "nil"
    local email = (data and data.userEmail) or ""
    if status ~= "unlocked" or email == "" then
        return {
            title = title,
            items = {
                error_item({
                    label = "Bitwarden session is not unlocked",
                    detail = string.format("bw reports status=%s, account=%s — run `export BW_SESSION=$(bw unlock --raw)` again and relaunch lark",
                                           status, email == "" and "(none)" or email),
                    icon = "🔒",
                    help_url = BW_LOCKED_HELP_URL,
                }),
            },
        }
    end
    return nil
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

-- SHARED: render_detail_markdown — build the markdown body for a vault item.
-- Used by both the TUI detail view (`raw_text`) and the lark.nvim Telescope
-- preview pane (`preview`). Always honors the `redact_secrets` setting so the
-- preview pane never leaks passwords/CVV/SSN to a screenshare. Caps output at
-- 5KB to keep the JSON payload small.
local PREVIEW_CAP = 5 * 1024
local function preview_truncate(s)
    if type(s) ~= "string" then return s end
    if #s <= PREVIEW_CAP then return s end
    return s:sub(1, PREVIEW_CAP) .. "\n\n…(truncated)"
end

local function render_detail_markdown(item)
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
    return preview_truncate(table.concat(lines, "\n"))
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
        -- Telescope preview pane (lark.nvim v0.14.0). `bw list items` already
        -- carries full bodies, so this is a free plumb. Honors the existing
        -- redact_secrets setting — never leaks passwords/CVV/SSN.
        preview = render_detail_markdown(item),
        actions = build_item_actions(item),
    }
end

local function detail_output(item)
    local panel = { item_to_row(item) }
    for _, a in ipairs(build_item_actions(item)) do
        if a.kind == "clipboard" then
            panel[#panel + 1] = { label = a.label, icon = "📋", actions = { a }, copy_text = a.args[1] }
        end
    end
    return {
        title = item.name or "Detail",
        raw_text = render_detail_markdown(item),
        output_format = "markdown",
        items = panel,
    }
end

lark.register({
    on_run = function()
        local session, err = bw_session("Search Vault")
        if err then return err end

        local verr = verify_session(session, "Search Vault")
        if verr then return verr end

        local data, rerr = run_bw(session, { "list", "items" }, "Search Vault")
        if rerr then return rerr end
        if type(data) ~= "table" then
            return { title = "Search Vault", items = { error_item({ label = "Empty response", help_url = BW_HELP_URL }) } }
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
                return { title = "TOTP", items = { error_item({ label = "No TOTP for this item", help_url = BW_HELP_URL }) } }
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
        return { title = "Bitwarden", items = { error_item({ label = "Unknown action: " .. callback_id, help_url = BW_HELP_URL }) } }
    end,
})
