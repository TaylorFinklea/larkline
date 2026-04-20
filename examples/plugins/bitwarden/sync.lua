-- Bitwarden: Sync Vault — show status + sync action.

local function bw_session(title)
    local session = lark.env("BW_SESSION")
    if session and session ~= "" then return session, nil end
    return nil, {
        title = title,
        items = {
            { label = "Bitwarden vault is locked", detail = "Run `export BW_SESSION=$(bw unlock --raw)` and re-open larkline", icon = "🔒" },
        },
    }
end

local function run_bw(session, args, title)
    local full = { "--session", session, "--response" }
    for _, a in ipairs(args) do full[#full + 1] = a end
    local raw = lark.exec("bw", full)
    if not raw or raw == "" then
        return nil, { title = title, items = { { label = "No response from bw CLI", detail = "Is `bw` installed and on $PATH?", icon = "!" } } }
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

local function locked_output(status)
    local state = (status and status.status) or "nil"
    local email = (status and status.userEmail) or ""
    return {
        title = "Bitwarden Status",
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
        return { title = "Bitwarden", items = { { label = "Unknown action", icon = "!" } } }
    end,
})
