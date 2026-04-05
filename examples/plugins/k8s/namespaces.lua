-- Kubernetes: Namespaces — namespace picker with resource counts.

lark.register({
    on_run = function()
        local raw = lark.exec("kubectl", { "get", "namespaces", "-o", "json" })

        if not raw or raw == "" then
            return {
                title = "Namespaces",
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
                title = "Namespaces",
                items = { { label = "Failed to parse kubectl output", icon = "⚠" } },
            }
        end

        -- Get the current default namespace from config.
        local current_ns = lark.exec("kubectl", { "config", "view", "--minify", "-o", "jsonpath={.contexts[0].context.namespace}" })
        if not current_ns or current_ns == "" then current_ns = "default" end

        local items = {}
        for _, ns_obj in ipairs(data.items) do
            local name = (ns_obj.metadata and ns_obj.metadata.name) or "?"
            local phase = (ns_obj.status and ns_obj.status.phase) or "Active"
            local is_current = name == current_ns

            local icon = is_current and "★" or "○"
            if phase ~= "Active" then icon = "⚠" end

            local detail_parts = { phase }
            if is_current then
                detail_parts[#detail_parts + 1] = "current"
            end

            -- Get pod count for this namespace (quick check).
            local pod_raw = lark.exec("kubectl", { "get", "pods", "-n", name, "--no-headers", "--ignore-not-found" })
            local pod_count = 0
            if pod_raw and pod_raw ~= "" then
                for _ in pod_raw:gmatch("[^\n]+") do
                    pod_count = pod_count + 1
                end
            end
            if pod_count > 0 then
                detail_parts[#detail_parts + 1] = pod_count .. " pods"
            end

            items[#items + 1] = {
                label = name,
                detail = table.concat(detail_parts, " · "),
                icon = icon,
                copy_text = name,
                actions = {
                    {
                        label = "Set as default namespace",
                        kind = "shell",
                        args = { "kubectl", "config", "set-context", "--current", "--namespace=" .. name },
                    },
                    { label = "List pods", kind = "shell", args = { "kubectl", "get", "pods", "-n", name } },
                    { label = "List all resources", kind = "shell", args = { "kubectl", "get", "all", "-n", name } },
                    { label = "Copy Name", kind = "clipboard", args = { name } },
                },
            }
        end

        if #items == 0 then
            return { title = "Namespaces", items = { { label = "No namespaces found", icon = "📭" } } }
        end

        return { title = "Namespaces — " .. #items, items = items }
    end,
})
