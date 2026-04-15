-- Notes: shared helpers for vault path resolution and file operations.
-- SYNC INSTRUCTIONS: Copy helpers into each command file that uses them
-- (sandbox has no require). This file is the canonical source.

-- Resolve the vault path from lark.store or common defaults.
-- Returns (path, nil) on success, (nil, error_items) on failure.
local function get_vault_path()
    local stored = lark.store.get("notes_vault_path")
    if stored and stored ~= "" then return stored, nil end

    -- Try common Obsidian vault locations.
    local home = lark.env("HOME") or "/tmp"
    local candidates = {
        home .. "/Documents/Obsidian",
        home .. "/Obsidian",
        home .. "/Documents/Notes",
        home .. "/Notes",
        home .. "/vaults",
    }
    for _, path in ipairs(candidates) do
        local check = lark.exec("test", { "-d", path })
        if check ~= nil then
            lark.store.set("notes_vault_path", path)
            return path, nil
        end
    end

    return nil, {
        { label = "No vault found", detail = "Use Settings to set your vault path", icon = "!" },
    }
end
