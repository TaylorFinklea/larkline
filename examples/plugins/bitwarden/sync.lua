-- Bitwarden: Sync Vault — show status + sync action.

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
    local full = { "--session", session, "--response" }
    for _, a in ipairs(args) do full[#full + 1] = a end
    local res = lark.exec_io("bw", full)
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

local function locked_output(status)
    local state = (status and status.status) or "nil"
    local email = (status and status.userEmail) or ""
    return {
        title = "Bitwarden Status",
        items = {
            error_item({
                label = "Bitwarden session is not unlocked",
                detail = string.format("bw reports status=%s, account=%s — run `export BW_SESSION=$(bw unlock --raw)` again and relaunch lark",
                                       state, email == "" and "(none)" or email),
                icon = "🔒",
                help_url = BW_LOCKED_HELP_URL,
            }),
        },
    }
end

lark.register({
    on_run = function()
        local session, err = bw_session("Sync Vault")
        if err then return err end

        local status, serr = run_bw(session, { "status" }, "Sync Vault")
        if serr then return serr end

        local account = (status and status.userEmail) or ""
        local state   = (status and status.status) or ""
        if state ~= "unlocked" or account == "" then
            return locked_output(status)
        end

        local server = (status and status.serverUrl) or "bitwarden.com"
        local synced = (status and (status.lastSync or status.lastsync)) or "never"

        local last_sync, lerr = run_bw(session, { "sync", "--last" }, "Sync Vault")
        if not lerr and last_sync and last_sync ~= "" then
            synced = tostring(last_sync):gsub('^"', ''):gsub('"$', '')
        end

        return {
            title = "Bitwarden Status",
            items = {
                { label = "Account", detail = account, icon = "👤" },
                { label = "Server", detail = server, icon = "🌐" },
                { label = "Vault state", detail = state, icon = state == "unlocked" and "🔓" or "🔒" },
                { label = "Last sync", detail = synced, icon = "⏱" },
                {
                    label = "Sync now",
                    detail = "Pull latest vault data from the server",
                    icon = "🔄",
                    actions = { { label = "Sync", kind = "chain", args = { "do_sync", "" } } },
                },
            },
        }
    end,

    on_action = function(callback_id, _context)
        if callback_id == "do_sync" then
            local session, err = bw_session("Sync")
            if err then return err end
            local _, serr = run_bw(session, { "sync" }, "Sync")
            if serr then return serr end
            return {
                title = "Bitwarden",
                items = { { label = "Vault synced", detail = "Latest data pulled from server", icon = "✅" } },
            }
        end
        return { title = "Bitwarden", items = { error_item({ label = "Unknown action", help_url = BW_HELP_URL }) } }
    end,
})
