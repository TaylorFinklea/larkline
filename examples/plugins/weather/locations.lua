-- Weather: Locations — manage saved weather locations via lark.store.

lark.register({
    on_run = function()
        local current = lark.store.get("weather_location") or ""
        local saved = lark.store.get("weather_locations") or {}

        -- If a form was submitted, handle it.
        if lark.form_values and lark.form_values.new_location then
            local new_loc = lark.form_values.new_location
            if new_loc ~= "" then
                -- Add to saved locations and set as active.
                local found = false
                for _, loc in ipairs(saved) do
                    if loc == new_loc then found = true break end
                end
                if not found then
                    saved[#saved + 1] = new_loc
                    lark.store.set("weather_locations", saved)
                end
                lark.store.set("weather_location", new_loc)
                current = new_loc
            end
        end

        local items = {}

        -- Auto-detect option.
        items[#items + 1] = {
            label = "Auto-detect (IP-based)",
            detail = current == "" and "active" or "",
            icon = current == "" and "★" or "○",
            actions = {
                {
                    label = "Use auto-detect",
                    kind = "shell",
                    args = { "echo", "Switching to auto-detect" },
                },
            },
        }

        -- Saved locations.
        for _, loc in ipairs(saved) do
            local is_active = loc == current
            items[#items + 1] = {
                label = loc,
                detail = is_active and "active" or "",
                icon = is_active and "★" or "○",
                actions = {
                    {
                        label = "Set as active",
                        kind = "shell",
                        args = { "echo", "Switched to " .. loc },
                    },
                    {
                        label = "Remove location",
                        kind = "shell",
                        args = { "echo", "Removed " .. loc },
                        confirm = true,
                    },
                },
            }
        end

        return {
            title = "Weather Locations — " .. (#saved + 1),
            items = items,
            form = {
                fields = {
                    { id = "new_location", label = "Add location (city, zip, or coords)", type = "text" },
                },
                submit_label = "Add & Activate",
            },
        }
    end,
})
