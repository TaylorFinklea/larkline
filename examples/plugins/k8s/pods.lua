-- Kubernetes: Pods — all pods across namespaces with status and actions.

lark.register({
    on_run = function()
        local raw = lark.exec("kubectl", { "get", "pods", "-A", "-o", "json" })

        if not raw or raw == "" then
            return {
                title = "Kubernetes Pods",
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
                title = "Kubernetes Pods",
                items = { { label = "Failed to parse kubectl output", icon = "⚠" } },
            }
        end

        local items = {}
        for _, pod in ipairs(data.items) do
            local ns   = (pod.metadata and pod.metadata.namespace) or "?"
            local name = (pod.metadata and pod.metadata.name) or "?"
            local phase = (pod.status and pod.status.phase) or "Unknown"

            local restarts = 0
            if pod.status and pod.status.containerStatuses then
                for _, cs in ipairs(pod.status.containerStatuses) do
                    restarts = restarts + (cs.restartCount or 0)
                end
            end

            local icon = phase == "Running" and "●" or (phase == "Pending" and "◌" or "✗")
            local detail = ns .. " · " .. phase
            if restarts > 0 then
                detail = detail .. " · ↺" .. restarts
            end

            items[#items + 1] = {
                label = name,
                detail = detail,
                icon = icon,
                copy_text = name,
                actions = {
                    { label = "Logs",       kind = "shell", args = { "kubectl", "logs", "-n", ns, name, "--tail=100" } },
                    { label = "Delete Pod", kind = "shell", args = { "kubectl", "delete", "pod", "-n", ns, name }, confirm = true },
                    { label = "Copy Name",  kind = "clipboard", args = { name } },
                },
            }
        end

        if #items == 0 then
            return { title = "Kubernetes Pods", items = { { label = "No pods found", icon = "📭" } } }
        end

        return { title = "Pods — " .. #items, items = items }
    end,
})
