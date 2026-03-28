-- Devices — list all entities with current state.

local helpers = require("helpers")

lark.register({
    on_run = function()
        local url, token, err = helpers.get_config()
        if err then return err end

        local states = helpers.api_get(url, token, "states")
        if not states then
            return { title = "Home Assistant", items = { { label = "Failed to fetch states", icon = "!" } } }
        end

        -- Sort by domain then friendly name.
        table.sort(states, function(a, b)
            local da = a.entity_id:match("^([^%.]+)%.") or ""
            local db = b.entity_id:match("^([^%.]+)%.") or ""
            if da ~= db then return da < db end
            return helpers.friendly_name(a) < helpers.friendly_name(b)
        end)

        local items = {}
        -- Filter to actionable domains by default.
        local show = { light = true, switch = true, binary_sensor = true, sensor = true,
                       climate = true, media_player = true, cover = true, lock = true, fan = true, camera = true }

        for _, entity in ipairs(states) do
            local domain = entity.entity_id:match("^([^%.]+)%.")
            if show[domain] then
                local name = helpers.friendly_name(entity)
                local state = entity.state or "unknown"
                local icon = helpers.icon_for(entity.entity_id, state)

                local actions = {
                    { label = "Copy entity ID", kind = "clipboard", args = { entity.entity_id } },
                }
                -- Toggleable domains get a toggle action.
                if domain == "light" or domain == "switch" or domain == "fan" or domain == "cover" or domain == "lock" then
                    table.insert(actions, 1, {
                        label = "Toggle",
                        kind = "shell",
                        args = { "lark", "invoke", "Home Assistant:toggle:" .. entity.entity_id },
                        confirm = true,
                    })
                end

                items[#items + 1] = {
                    label = name,
                    detail = state .. "  " .. entity.entity_id,
                    icon = icon,
                    copy_text = entity.entity_id,
                    actions = actions,
                }
            end
        end

        if #items == 0 then
            return { title = "Home Assistant", items = { { label = "No entities found", icon = "📭" } } }
        end

        return { title = "Home Assistant — " .. #items .. " devices", items = items }
    end,
})
