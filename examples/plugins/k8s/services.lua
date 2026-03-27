-- Kubernetes: Services — all services across namespaces.

lark.register({
    on_run = function()
        local raw = lark.exec("kubectl", { "get", "svc", "-A", "-o", "json" })

        if not raw or raw == "" then
            return {
                title = "Kubernetes Services",
                items = { {
                    label = "kubectl unavailable or cluster unreachable",
                    icon = "⚠",
                    detail = "Install kubectl and configure a cluster",
                } },
            }
        end

        local ok, data = pcall(lark.json.decode, raw)
        if not ok or not data or not data.items then
            return {
                title = "Kubernetes Services",
                items = { { label = "Failed to parse kubectl output", icon = "⚠" } },
            }
        end

        local items = {}
        for _, svc in ipairs(data.items) do
            local ns   = (svc.metadata and svc.metadata.namespace) or "?"
            local name = (svc.metadata and svc.metadata.name) or "?"
            local svc_type = (svc.spec and svc.spec.type) or "?"

            local port_parts = {}
            if svc.spec and svc.spec.ports then
                for _, p in ipairs(svc.spec.ports) do
                    local s = tostring(p.port or "?")
                    if p.targetPort then
                        s = s .. ":" .. tostring(p.targetPort)
                    end
                    if p.protocol then
                        s = s .. "/" .. p.protocol
                    end
                    port_parts[#port_parts + 1] = s
                end
            end

            local detail = ns .. " · " .. svc_type
            if #port_parts > 0 then
                detail = detail .. " · " .. table.concat(port_parts, ", ")
            end

            items[#items + 1] = {
                label = name,
                detail = detail,
                icon = "◈",
                copy_text = name,
                actions = {
                    { label = "Copy Name",      kind = "clipboard", args = { name } },
                    { label = "Copy Namespace", kind = "clipboard", args = { ns } },
                },
            }
        end

        if #items == 0 then
            return { title = "Kubernetes Services", items = { { label = "No services found", icon = "📭" } } }
        end

        return { title = "Services — " .. #items, items = items }
    end,
})
