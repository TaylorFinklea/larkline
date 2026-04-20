-- Shared helpers for Bitwarden plugin.
-- The Lark sandbox does not expose require/dofile/loadfile, so each command
-- file copies the helpers it needs inline. This file is the canonical source;
-- edit here, then sync to the command files.
--
-- SYNC INSTRUCTIONS:
--   items.lua, favorites.lua, folders.lua use: bw_session(), run_bw(),
--     icon_for_type(), redact(), build_login_actions(), build_note_actions(),
--     build_card_actions(), build_identity_actions(), item_detail_lines(),
--     item_actions().
--   generate.lua uses: bw_session() (only for context messaging).
--   sync.lua, lock.lua use: bw_session(), run_bw().

-- Look up the session key. Prefer BW_SESSION from env/secrets so the user can
-- run `bw unlock` once and export it. Returns (session, err) where err is an
-- OutputItem-shaped error payload when the vault is locked.
local function bw_session(title)
    local session = lark.env("BW_SESSION")
    if session and session ~= "" then return session, nil end
    return nil, {
        title = title,
        items = {
            {
                label = "Bitwarden vault is locked",
                detail = "Run `export BW_SESSION=$(bw unlock --raw)` and re-open larkline",
                icon = "🔒",
            },
            {
                label = "Not logged in?",
                detail = "Run `bw login` first",
                icon = "→",
            },
        },
    }
end

-- Invoke `bw` with a session key via lark's sandboxed process API (no shell).
-- Returns (data, error_payload_or_nil).
local function run_bw(session, args, title)
    local full = { "--session", session, "--response" }
    for _, a in ipairs(args) do full[#full + 1] = a end
    local raw = lark.exec("bw", full)
    if not raw or raw == "" then
        return nil, {
            title = title,
            items = { { label = "No response from bw CLI", detail = "Is `bw` installed and on $PATH?", icon = "!" } },
        }
    end
    -- --response returns { success: bool, data: ..., message: ... }
    local ok, parsed = pcall(lark.json.decode, raw)
    if not ok or type(parsed) ~= "table" then
        return nil, {
            title = title,
            items = { { label = "Failed to parse bw response", detail = raw:sub(1, 120), icon = "!" } },
        }
    end
    if not parsed.success then
        local msg = parsed.message or "unknown error"
        return nil, {
            title = title,
            items = { { label = "bw error: " .. msg, icon = "!" } },
        }
    end
    return parsed.data, nil
end

-- Preflight the session by asking bw for its status. bw never errors for a
-- stale session — it silently reports status="locked", userEmail=null — so we
-- must verify the unlocked state before trusting list output.
local function verify_session(session, title)
    local data, err = run_bw(session, { "status" }, title)
    if err then return err end
    local state = (data and data.status) or "nil"
    local email = (data and data.userEmail) or ""
    if state ~= "unlocked" or email == "" then
        return {
            title = title,
            items = {
                {
                    label = "Bitwarden session is not unlocked",
                    detail = string.format("bw reports status=%s, account=%s — run `export BW_SESSION=$(bw unlock --raw)` again and relaunch lark",
                                           state, email == "" and "(none)" or email),
                    icon = "🔒",
                },
            },
        }
    end
    return nil
end

local function icon_for_type(t)
    if t == 1 then return "🔑" end  -- login
    if t == 2 then return "📝" end  -- secure note
    if t == 3 then return "💳" end  -- card
    if t == 4 then return "👤" end  -- identity
    return "•"
end

local function redact(s)
    if not s or s == "" then return "" end
    return string.rep("•", math.min(#s, 12))
end

-- Settings helper: redact_secrets toggle (default true).
local function redact_enabled()
    local v = lark.store.get("redact_secrets")
    if v == "false" or v == false then return false end
    return true
end

-- Extract primary URI (first one) for a login item.
local function primary_uri(item)
    if item.type ~= 1 or not item.login or not item.login.uris then return nil end
    local u = item.login.uris[1]
    if u and u.uri and u.uri ~= "" then return u.uri end
    return nil
end

-- Build copy/open actions for a login (type 1) item.
local function build_login_actions(item)
    local actions = {}
    if item.login then
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
    end
    return actions
end

local function build_note_actions(item)
    local actions = {}
    if item.notes and item.notes ~= "" then
        actions[#actions + 1] = { label = "Copy Note", kind = "clipboard", args = { item.notes } }
    end
    return actions
end

local function build_card_actions(item)
    local actions = {}
    if not item.card then return actions end
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
    if item.card.brand and item.card.brand ~= "" then
        actions[#actions + 1] = { label = "Copy Brand", kind = "clipboard", args = { item.card.brand } }
    end
    return actions
end

local function build_identity_actions(item)
    local actions = {}
    if not item.identity then return actions end
    local id = item.identity
    local pairs_list = {
        { "Email", id.email },
        { "Username", id.username },
        { "Phone", id.phone },
        { "SSN", id.ssn },
        { "Passport Number", id.passportNumber },
        { "License Number", id.licenseNumber },
    }
    for _, kv in ipairs(pairs_list) do
        local name, value = kv[1], kv[2]
        if value and value ~= "" then
            actions[#actions + 1] = { label = "Copy " .. name, kind = "clipboard", args = { value } }
        end
    end
    return actions
end

-- Build a unified action list for any item type plus custom fields at the end.
local function item_actions(item)
    local actions
    if item.type == 1 then actions = build_login_actions(item)
    elseif item.type == 2 then actions = build_note_actions(item)
    elseif item.type == 3 then actions = build_card_actions(item)
    elseif item.type == 4 then actions = build_identity_actions(item)
    else actions = {} end

    -- Custom fields (any item type).
    if item.fields then
        for _, f in ipairs(item.fields) do
            if f.name and f.value and f.value ~= "" then
                actions[#actions + 1] = {
                    label = "Copy " .. f.name,
                    kind = "clipboard",
                    args = { f.value },
                }
            end
        end
    end

    -- Common actions available for everything.
    actions[#actions + 1] = { label = "View Details", kind = "chain", args = { "show_detail", item.id } }
    if item.notes and item.notes ~= "" and item.type ~= 2 then
        actions[#actions + 1] = { label = "Copy Note", kind = "clipboard", args = { item.notes } }
    end
    return actions
end

-- Build markdown-style detail lines for an item. Redacts secrets by default.
local function item_detail_lines(item, redact_on)
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
            for i, u in ipairs(item.login.uris) do
                kv("URL " .. i, u.uri)
            end
        end
    elseif item.type == 2 then
        kv("Kind", "Secure Note")
    elseif item.type == 3 and item.card then
        kv("Cardholder", item.card.cardholderName)
        kv("Brand", item.card.brand)
        kvr("Number", item.card.number)
        kvr("CVV", item.card.code)
        if item.card.expMonth and item.card.expYear then
            kv("Expires", string.format("%02d/%s", tonumber(item.card.expMonth) or 0, item.card.expYear))
        end
    elseif item.type == 4 and item.identity then
        kv("First Name", item.identity.firstName)
        kv("Middle Name", item.identity.middleName)
        kv("Last Name", item.identity.lastName)
        kv("Email", item.identity.email)
        kv("Phone", item.identity.phone)
        kv("Username", item.identity.username)
        kvr("SSN", item.identity.ssn)
        kvr("Passport", item.identity.passportNumber)
        kvr("License", item.identity.licenseNumber)
        kv("Address 1", item.identity.address1)
        kv("Address 2", item.identity.address2)
        kv("City", item.identity.city)
        kv("State", item.identity.state)
        kv("Postal Code", item.identity.postalCode)
        kv("Country", item.identity.country)
    end

    if item.fields and #item.fields > 0 then
        lines[#lines + 1] = ""
        lines[#lines + 1] = "## Custom Fields"
        for _, f in ipairs(item.fields) do
            if f.name then
                -- Bitwarden field types: 0=text, 1=hidden, 2=boolean, 3=linked
                if f.type == 1 then
                    kvr(f.name, f.value)
                else
                    kv(f.name, f.value)
                end
            end
        end
    end

    if item.notes and item.notes ~= "" then
        lines[#lines + 1] = ""
        lines[#lines + 1] = "## Notes"
        lines[#lines + 1] = ""
        lines[#lines + 1] = item.notes
    end

    return lines
end

-- Detail renderer reused from on_action handlers. Returns a PluginOutput.
local function render_detail(item, title)
    local redact_on = redact_enabled()
    local lines = item_detail_lines(item, redact_on)
    local items = {}
    local acts = item_actions(item)
    items[#items + 1] = {
        label = item.name or "Untitled",
        detail = primary_uri(item) or "",
        icon = icon_for_type(item.type),
        actions = acts,
    }
    for _, a in ipairs(acts) do
        if a.kind == "clipboard" then
            items[#items + 1] = {
                label = a.label,
                icon = "📋",
                actions = { a },
                copy_text = a.args[1],
            }
        end
    end
    return {
        title = title or (item.name or "Detail"),
        raw_text = table.concat(lines, "\n"),
        output_format = "markdown",
        items = items,
    }
end
