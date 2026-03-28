-- Automations — list, trigger, enable, or disable HA automations.

local helpers = require("helpers")

lark.register({
    on_run = function()
        local url, token, err = helpers.get_config()
        if err then return err end

        local states = helpers.api_get(url, token, "states")
        if not states then
            return { title = "Automations", items = { { label = "Failed to fetch states", icon = "!" } } }
        end

        local items = {}
        for _, entity in ipairs(states) do
            if entity.entity_id:match("^automation%.") then
                local name = helpers.friendly_name(entity)
                local state = entity.state or "unknown"
                local icon = helpers.icon_for(entity.entity_id, state)
                local body = lark.json.encode({ entity_id = entity.entity_id })

                local actions = {
                    {
                        label = "Trigger",
                        kind = "shell",
                        args = {
                            "curl", "-s", "-X", "POST",
                            url .. "/api/services/automation/trigger",
                            "-H", "Authorization: Bearer " .. token,
                            "-H", "Content-Type: application/json",
                            "-d", body,
                        },
                        confirm = true,
                    },
                }

                if state == "on" then
                    actions[#actions + 1] = {
                        label = "Disable",
                        kind = "shell",
                        args = {
                            "curl", "-s", "-X", "POST",
                            url .. "/api/services/automation/turn_off",
                            "-H", "Authorization: Bearer " .. token,
                            "-H", "Content-Type: application/json",
                            "-d", body,
                        },
                        confirm = true,
                    }
                else
                    actions[#actions + 1] = {
                        label = "Enable",
                        kind = "shell",
                        args = {
                            "curl", "-s", "-X", "POST",
                            url .. "/api/services/automation/turn_on",
                            "-H", "Authorization: Bearer " .. token,
                            "-H", "Content-Type: application/json",
                            "-d", body,
                        },
                        confirm = true,
                    }
                end

                actions[#actions + 1] = { label = "Copy entity ID", kind = "clipboard", args = { entity.entity_id } }

                items[#items + 1] = {
                    label = name,
                    detail = state .. "  " .. entity.entity_id,
                    icon = icon,
                    actions = actions,
                }
            end
        end

        table.sort(items, function(a, b) return a.label < b.label end)

        if #items == 0 then
            return { title = "Automations", items = { { label = "No automations found", icon = "📭" } } }
        end

        return { title = "Automations — " .. #items, items = items }
    end,
})
