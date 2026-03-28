-- Scenes — list and activate Home Assistant scenes.

local helpers = require("helpers")

lark.register({
    on_run = function()
        local url, token, err = helpers.get_config()
        if err then return err end

        local states = helpers.api_get(url, token, "states")
        if not states then
            return { title = "Scenes", items = { { label = "Failed to fetch states", icon = "!" } } }
        end

        local items = {}
        for _, entity in ipairs(states) do
            if entity.entity_id:match("^scene%.") then
                local name = helpers.friendly_name(entity)
                local body = lark.json.encode({ entity_id = entity.entity_id })

                items[#items + 1] = {
                    label = name,
                    detail = entity.entity_id,
                    icon = "🎬",
                    actions = {
                        {
                            label = "Activate " .. name,
                            kind = "shell",
                            args = {
                                "curl", "-s", "-X", "POST",
                                url .. "/api/services/scene/turn_on",
                                "-H", "Authorization: Bearer " .. token,
                                "-H", "Content-Type: application/json",
                                "-d", body,
                            },
                            confirm = true,
                        },
                        { label = "Copy entity ID", kind = "clipboard", args = { entity.entity_id } },
                    },
                }
            end
        end

        table.sort(items, function(a, b) return a.label < b.label end)

        if #items == 0 then
            return { title = "Scenes", items = { { label = "No scenes found", icon = "📭" } } }
        end

        return { title = "Scenes — " .. #items .. " scenes", items = items }
    end,
})
