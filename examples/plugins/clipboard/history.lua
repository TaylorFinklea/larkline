-- Clipboard: History — reads clipboard history from Maccy's local database.

local DB_PATH_SUFFIX = "/Library/Containers/org.p0deje.Maccy/Data/Library/Application Support/Maccy/Storage.sqlite"

lark.register({
    on_run = function()
        local home = lark.env("HOME")
        local db = home .. DB_PATH_SUFFIX

        -- Use Python to query Maccy's CoreData SQLite store and return JSON.
        local script = string.format([[
import sqlite3, json, os, sys
db = %q
if not os.path.exists(db):
    print(json.dumps({"error": "not_found"}))
    sys.exit(0)
try:
    conn = sqlite3.connect(db)
    rows = conn.execute("""
        SELECT hi.ZTITLE, hc.ZVALUE, hi.ZAPPLICATION
        FROM ZHISTORYITEM hi
        JOIN ZHISTORYITEMCONTENT hc ON hc.ZITEM = hi.Z_PK
        WHERE hc.ZTYPE = 'public.utf8-plain-text'
        ORDER BY hi.ZLASTCOPIEDAT DESC LIMIT 25
    """).fetchall()
    result = []
    for title, value, app in rows:
        if isinstance(value, bytes):
            value = value.decode("utf-8", errors="replace")
        result.append({"title": title or "", "value": value or title or "", "app": app or ""})
    print(json.dumps({"items": result}))
except Exception as e:
    print(json.dumps({"error": str(e)}))
]], db)

        local raw = lark.exec("python3", { "-c", script })

        if not raw or raw == "" then
            return {
                title = "Clipboard History",
                items = { { label = "Failed to read Maccy history", icon = "⚠" } },
            }
        end

        local ok, data = pcall(lark.json.decode, raw)
        if not ok or not data then
            return {
                title = "Clipboard History",
                items = { { label = "Failed to parse Maccy data", icon = "⚠" } },
            }
        end

        if data.error then
            if data.error == "not_found" then
                return {
                    title = "Clipboard History",
                    items = { {
                        label  = "Maccy not installed",
                        icon   = "⚠",
                        detail = "Install from maccy.app, then copy some text",
                    } },
                }
            end
            return {
                title = "Clipboard History",
                items = { { label = "Error: " .. tostring(data.error), icon = "⚠" } },
            }
        end

        if not data.items or #data.items == 0 then
            return {
                title = "Clipboard History",
                items = { { label = "No clipboard history in Maccy", icon = "📭" } },
            }
        end

        local items = {}
        for i, entry in ipairs(data.items) do
            -- Use title (Maccy's preview) for the label; fall back to first line of value.
            local label = entry.title ~= "" and entry.title or (entry.value:match("^([^\n]+)") or entry.value)
            if #label > 70 then label = label:sub(1, 67) .. "..." end

            -- Show app name (last bundle-ID component, capitalised).
            local detail = ""
            if entry.app ~= "" then
                local short = entry.app:match("%.([^.]+)$") or entry.app
                detail = short:sub(1, 1):upper() .. short:sub(2)
            end

            items[#items + 1] = {
                label     = label,
                detail    = detail,
                icon      = i == 1 and "●" or "◈",
                copy_text = entry.value,
                actions   = {
                    { label = "Restore to Clipboard", kind = "clipboard", args = { entry.value } },
                    { label = "Copy",                 kind = "clipboard", args = { entry.value } },
                },
            }
        end

        return { title = "Clipboard — " .. #items, items = items }
    end,
})
