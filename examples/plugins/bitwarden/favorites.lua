-- Bitwarden: Favorites — favorite items from your vault.
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

        local data, rerr = run_bw(session, { "list", "items", "--favorite" }, "Favorites")
        if rerr then return rerr end
        if type(data) ~= "table" then
            return { title = "Favorites", items = { { label = "Empty response", icon = "!" } } }
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
        return { title = "Bitwarden", items = { { label = "Unknown action", icon = "!" } } }
    end,
})
