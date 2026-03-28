-- Toggle — toggle a light, switch, fan, cover, or lock.

local helpers = require("helpers")

lark.register({
    on_run = function()
        local url, token, err = helpers.get_config()
        if err then return err end

        -- If invoked with a specific entity_id (via action), use it directly.
        -- Otherwise, show toggleable entities for the user to pick.
        local states = helpers.api_get(url, token, "states")
        if not states then
            return { title = "Toggle", items = { { label = "Failed to fetch states", icon = "!" } } }
        end

        local toggleable = { light = true, switch = true, fan = true, cover = true, lock = true }
        local items = {}

        for _, entity in ipairs(states) do
            local domain = entity.entity_id:match("^([^%.]+)%.")
            if toggleable[domain] then
                local name = helpers.friendly_name(entity)
                local state = entity.state or "unknown"
                local icon = helpers.icon_for(entity.entity_id, state)
                local service = domain .. "/toggle"
                local body = lark.json.encode({ entity_id = entity.entity_id })

                items[#items + 1] = {
                    label = name,
                    detail = state .. "  " .. entity.entity_id,
                    icon = icon,
                    actions = {
                        {
                            label = "Toggle " .. name,
                            kind = "shell",
                            args = {
                                "curl", "-s", "-X", "POST",
                                url .. "/api/services/" .. service,
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
            return { title = "Toggle", items = { { label = "No toggleable devices found", icon = "📭" } } }
        end

        return { title = "Toggle — " .. #items .. " devices", items = items }
    end,
})
