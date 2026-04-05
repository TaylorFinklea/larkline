-- Kubernetes: Logs — view recent logs from pods (uses stored namespace or default).

lark.register({
    on_run = function()
        -- Get pods from current namespace context.
        local ns_raw = lark.exec("kubectl", { "config", "view", "--minify", "-o", "jsonpath={.contexts[0].context.namespace}" })
        local ns = (ns_raw and ns_raw ~= "") and ns_raw or "default"

        local raw = lark.exec("kubectl", { "get", "pods", "-n", ns, "-o", "json" })

        if not raw or raw == "" then
            return {
                title = "Logs (" .. ns .. ")",
                items = { {
                    label = "kubectl unavailable or no pods in " .. ns,
                    icon = "⚠",
                    detail = "Switch namespace or check cluster",
                } },
            }
        end

        local ok, data = pcall(lark.json.decode, raw)
        if not ok or not data or not data.items then
            return {
                title = "Logs (" .. ns .. ")",
                items = { { label = "Failed to parse kubectl output", icon = "⚠" } },
            }
        end

        if #data.items == 0 then
            return {
                title = "Logs (" .. ns .. ")",
                items = { { label = "No pods in namespace " .. ns, icon = "📭" } },
            }
        end

        local items = {}
        for _, pod in ipairs(data.items) do
            local name = (pod.metadata and pod.metadata.name) or "?"
            local phase = (pod.status and pod.status.phase) or "Unknown"

            -- List containers for multi-container pods.
            local containers = {}
            if pod.spec and pod.spec.containers then
                for _, c in ipairs(pod.spec.containers) do
                    containers[#containers + 1] = c.name
                end
            end

            local icon = phase == "Running" and "●" or "○"
            local detail = phase
            if #containers > 1 then
                detail = detail .. " · " .. #containers .. " containers"
            end

            local actions = {
                { label = "Logs (last 100)", kind = "shell", args = { "kubectl", "logs", "-n", ns, name, "--tail=100" } },
                { label = "Logs (follow)", kind = "shell", args = { "kubectl", "logs", "-n", ns, name, "-f", "--tail=50" } },
                { label = "Logs (previous)", kind = "shell", args = { "kubectl", "logs", "-n", ns, name, "--previous", "--tail=100" } },
            }

            -- Add per-container log actions for multi-container pods.
            if #containers > 1 then
                for _, cname in ipairs(containers) do
                    actions[#actions + 1] = {
                        label = "Logs: " .. cname,
                        kind = "shell",
                        args = { "kubectl", "logs", "-n", ns, name, "-c", cname, "--tail=100" },
                    }
                end
            end

            actions[#actions + 1] = { label = "Copy Name", kind = "clipboard", args = { name } }

            items[#items + 1] = {
                label = name,
                detail = detail,
                icon = icon,
                copy_text = name,
                actions = actions,
            }
        end

        return { title = "Logs (" .. ns .. ") — " .. #items .. " pods", items = items }
    end,
})
