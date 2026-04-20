-- Bitwarden: Folders — browse by folder, drill in to see items.
-- Shared helpers copied from lib.lua.

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
        return nil, { title = title, items = { { label = "No response from bw CLI", icon = "!" } } }
    end
    local ok, parsed = pcall(lark.json.decode, raw)
    if not ok or type(parsed) ~= "table" then
        return nil, { title = title, items = { { label = "Failed to parse bw response", icon = "!" } } }
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
            items[#items + 1] = { label = "No folders", detail = "Create folders in the Bitwarden app", icon = "!" }
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
        return { title = "Bitwarden", items = { { label = "Unknown action", icon = "!" } } }
    end,
})
