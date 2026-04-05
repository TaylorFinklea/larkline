-- Kubernetes: Deployments — deployment status and scaling.

lark.register({
    on_run = function()
        local raw = lark.exec("kubectl", { "get", "deployments", "-A", "-o", "json" })

        if not raw or raw == "" then
            return {
                title = "Deployments",
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
                title = "Deployments",
                items = { { label = "Failed to parse kubectl output", icon = "⚠" } },
            }
        end

        local items = {}
        for _, dep in ipairs(data.items) do
            local ns   = (dep.metadata and dep.metadata.namespace) or "?"
            local name = (dep.metadata and dep.metadata.name) or "?"

            local desired = (dep.spec and dep.spec.replicas) or 0
            local ready = 0
            local updated = 0
            local available = 0
            if dep.status then
                ready = dep.status.readyReplicas or 0
                updated = dep.status.updatedReplicas or 0
                available = dep.status.availableReplicas or 0
            end

            local icon = "●"
            if ready == 0 and desired > 0 then icon = "✗"
            elseif ready < desired then icon = "◐"
            elseif ready == desired then icon = "●"
            end

            local detail = ns .. " · " .. ready .. "/" .. desired .. " ready"
            if updated ~= desired then
                detail = detail .. " · " .. updated .. " updated"
            end

            items[#items + 1] = {
                label = name,
                detail = detail,
                icon = icon,
                copy_text = name,
                actions = {
                    { label = "Scale to 0", kind = "shell", args = { "kubectl", "scale", "deployment", "-n", ns, name, "--replicas=0" }, confirm = true },
                    { label = "Scale to 1", kind = "shell", args = { "kubectl", "scale", "deployment", "-n", ns, name, "--replicas=1" } },
                    { label = "Scale to 3", kind = "shell", args = { "kubectl", "scale", "deployment", "-n", ns, name, "--replicas=3" } },
                    { label = "Restart (rollout)", kind = "shell", args = { "kubectl", "rollout", "restart", "deployment", "-n", ns, name }, confirm = true },
                    { label = "Describe", kind = "shell", args = { "kubectl", "describe", "deployment", "-n", ns, name } },
                    { label = "Rollout History", kind = "shell", args = { "kubectl", "rollout", "history", "deployment", "-n", ns, name } },
                    { label = "Copy Name", kind = "clipboard", args = { name } },
                },
            }
        end

        if #items == 0 then
            return { title = "Deployments", items = { { label = "No deployments found", icon = "📭" } } }
        end

        return { title = "Deployments — " .. #items, items = items }
    end,
})
