-- Bitwarden: Folders — browse by folder, drill in to see items.
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
    end
    return actions
end

local function item_to_row(item)
    local detail_parts = { type_label(item.type) }
    if item.type == 1 and item.login and item.login.username and item.login.username ~= "" then
        detail_parts[#detail_parts + 1] = item.login.username
    end
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
        local session, err = bw_session("Folders")
        if err then return err end

        local verr = verify_session(session, "Folders")
        if verr then return verr end

        local folders_data, ferr = run_bw(session, { "list", "folders" }, "Folders")
        if ferr then return ferr end

        local items_data, ierr = run_bw(session, { "list", "items" }, "Folders")
        if ierr then return ierr end

        -- Count items per folder ID (including "No Folder" / null).
        local counts = { [""] = 0 }
        if type(items_data) == "table" then
            for _, it in ipairs(items_data) do
                local fid = it.folderId or ""
                counts[fid] = (counts[fid] or 0) + 1
            end
        end

        local items = {}
        if type(folders_data) == "table" then
            for _, f in ipairs(folders_data) do
                local fid = f.id or ""
                local count = counts[fid] or 0
                items[#items + 1] = {
                    label = f.name or "Untitled",
                    detail = string.format("%d item%s", count, count == 1 and "" or "s"),
                    icon = "📁",
                    actions = {
                        { label = "Open folder", kind = "chain", args = { "open_folder", fid } },
                    },
                }
            end
        end
        -- "No Folder" bucket — items with folderId == null.
        local no_folder_count = counts[""] or 0
        if no_folder_count > 0 then
            items[#items + 1] = {
                label = "No Folder",
                detail = string.format("%d item%s", no_folder_count, no_folder_count == 1 and "" or "s"),
                icon = "📂",
                actions = { { label = "Open folder", kind = "chain", args = { "open_folder", "__none__" } } },
            }
        end

        if #items == 0 then
            items[#items + 1] = error_item({ label = "No folders", detail = "Create folders in the Bitwarden app", help_url = BW_HELP_URL })
        end

        return {
            title = string.format("Folders — %d", #items),
            items = items,
        }
    end,

    on_action = function(callback_id, context)
        if callback_id == "open_folder" then
            local session, err = bw_session("Folder")
            if err then return err end

            local args = { "list", "items" }
            local target_none = context == "__none__"
            if not target_none then
                table.insert(args, "--folderid")
                table.insert(args, context)
            end

            local data, rerr = run_bw(session, args, "Folder")
            if rerr then return rerr end

            local items = {}
            if type(data) == "table" then
                for _, it in ipairs(data) do
                    if target_none then
                        if not it.folderId then items[#items + 1] = item_to_row(it) end
                    else
                        items[#items + 1] = item_to_row(it)
                    end
                end
            end
            if #items == 0 then
                items[#items + 1] = { label = "No items in this folder", icon = "📭" }
            end
            return {
                title = string.format("Folder — %d item%s", #items, #items == 1 and "" or "s"),
                items = items,
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
