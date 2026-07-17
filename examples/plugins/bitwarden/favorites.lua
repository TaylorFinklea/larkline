-- Bitwarden: Favorites — favorite items from your vault.
-- Shared helpers copied from lib.lua.

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
        },
    }
end

-- bw --response wraps payloads: list={object,data}, template={object,template},
-- string={object,data}, item/folder={object,...fields}. Unwrap inline.
local function unwrap_bw(d)
    if type(d) ~= "table" then return d end
    if d.object == "list" then return d.data or {} end
    if d.object == "template" then return d.template or {} end
    if d.object == "string" then return d.data or "" end
    return d
end

local function run_bw(session, args, title)
    -- Session via env, not argv: argv is visible in `ps` for the child's
    -- lifetime; BW_SESSION in the environment is not.
    local full = { "--response" }
    for _, a in ipairs(args) do full[#full + 1] = a end
    local res = lark.exec_io("bw", full, { env = { BW_SESSION = session } })
    if res.exit_code ~= 0 or res.stdout == "" then
        local translated = from_exit(res.stderr, {
            cli = "bw",
            service = "Bitwarden",
            login_command = "bw login",
            install_url = BW_HELP_URL,
            login_help_url = BW_LOCKED_HELP_URL,
        })
        return nil, { title = title, level = "warn", items = { translated or error_item({ label = "No response from bw CLI", detail = "Is `bw` installed and on $PATH?", help_url = BW_HELP_URL }) } }
    end
    local raw = res.stdout
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

local function primary_uri(item)
    if item.type ~= 1 or not item.login or not item.login.uris then return nil end
    local u = item.login.uris[1]
    if u and u.uri and u.uri ~= "" then return u.uri end
    return nil
end

local function type_label(t)
    if t == 1 then return "Login" end
    if t == 2 then return "Note" end
    if t == 3 then return "Card" end
    if t == 4 then return "Identity" end
    return "Item"
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
        local uri = primary_uri(item)
        if uri then
            actions[#actions + 1] = { label = "Open " .. uri, kind = "open", args = { uri } }
            actions[#actions + 1] = { label = "Copy URL", kind = "clipboard", args = { uri } }
        end
    elseif item.type == 3 and item.card then
        if item.card.number and item.card.number ~= "" then
            actions[#actions + 1] = { label = "Copy Card Number", kind = "clipboard", args = { item.card.number } }
        end
        if item.card.code and item.card.code ~= "" then
            actions[#actions + 1] = { label = "Copy CVV", kind = "clipboard", args = { item.card.code } }
        end
    end
    actions[#actions + 1] = { label = "View Details", kind = "chain", args = { "show_detail", item.id } }
    return actions
end

local function item_to_row(item)
    local detail_parts = { type_label(item.type) }
    if item.type == 1 and item.login and item.login.username and item.login.username ~= "" then
        detail_parts[#detail_parts + 1] = item.login.username
    end
    local uri = primary_uri(item)
    if uri then detail_parts[#detail_parts + 1] = uri end
    return {
        label = item.name or "Untitled",
        detail = table.concat(detail_parts, "  ·  "),
        icon = icon_for_type(item.type),
        copy_text = (item.type == 1 and item.login and item.login.password) or item.name,
        actions = build_item_actions(item),
    }
end

lark.register({
    on_run = function()
        local session, err = bw_session("Favorites")
        if err then return err end

        local verr = verify_session(session, "Favorites")
        if verr then return verr end

        local data, rerr = run_bw(session, { "list", "items", "--favorite" }, "Favorites")
        if rerr then return rerr end
        if type(data) ~= "table" then
            return { title = "Favorites", items = { error_item({ label = "Empty response", help_url = BW_HELP_URL }) } }
        end

        local items = {}
        for _, it in ipairs(data) do
            items[#items + 1] = item_to_row(it)
        end
        if #items == 0 then
            items[#items + 1] = { label = "No favorites yet", detail = "Mark items as favorite in Bitwarden", icon = "⭐" }
        end

        return {
            title = string.format("Favorites — %d item%s", #items, #items == 1 and "" or "s"),
            items = items,
        }
    end,

    on_action = function(callback_id, context)
        if callback_id == "show_detail" then
            local session, err = bw_session("Detail")
            if err then return err end
            local data, rerr = run_bw(session, { "get", "item", context }, "Detail")
            if rerr then return rerr end
            local title = (data and data.name) or "Detail"
            local lines = { "# " .. title, "" }
            if data.type == 1 and data.login then
                if data.login.username and data.login.username ~= "" then
                    lines[#lines + 1] = "- **Username:** " .. data.login.username
                end
                if data.login.uris then
                    for i, u in ipairs(data.login.uris) do
                        if u.uri and u.uri ~= "" then lines[#lines + 1] = "- **URL " .. i .. ":** " .. u.uri end
                    end
                end
            end
            if data.notes and data.notes ~= "" then
                lines[#lines + 1] = ""
                lines[#lines + 1] = "## Notes"
                lines[#lines + 1] = ""
                lines[#lines + 1] = data.notes
            end
            return {
                title = title,
                raw_text = table.concat(lines, "\n"),
                output_format = "markdown",
                items = { item_to_row(data) },
            }
        elseif callback_id == "copy_totp" then
            local session, err = bw_session("TOTP")
            if err then return err end
            local totp_raw, rerr = run_bw(session, { "get", "totp", context }, "TOTP")
            if rerr then return rerr end
            local code = tostring(totp_raw or ""):gsub("%s+$", "")
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
        return { title = "Bitwarden", items = { error_item({ label = "Unknown action", help_url = BW_HELP_URL }) } }
    end,
})
