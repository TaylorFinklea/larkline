-- Kubernetes: Namespaces — namespace picker with resource counts.
-- Shared helpers copied inline (the Lark sandbox has no require).

-- SHARED: error_item — canonical copy in examples/plugins/_shared/errors.lua.
local function error_item(opts)
    return {
        label = opts.label,
        detail = opts.detail,
        icon = opts.icon or "!",
        retry_action = opts.retry_action,
        help_url = opts.help_url,
    }
end

-- SHARED: from_exit — canonical copy in examples/plugins/_shared/errors.lua.
-- Translate a shell process's stderr into a friendly error item. Returns nil
-- if no pattern matched.
local function from_exit(stderr, hints)
    hints = hints or {}
    stderr = stderr or ""
    local lower = stderr:lower()

    if lower:find("command not found", 1, true)
        or lower:find("no such file or directory", 1, true) then
        local cli = hints.cli or "command"
        local detail
        if hints.install_url then
            detail = "Install: " .. hints.install_url
        else
            detail = "Check your $PATH"
        end
        return error_item({
            label = cli .. " not found",
            detail = detail,
            help_url = hints.install_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("401", 1, true)
        or lower:find("403", 1, true)
        or lower:find("unauthorized", 1, true)
        or lower:find("forbidden", 1, true)
        or lower:find("not authenticated", 1, true)
        or lower:find("not logged in", 1, true)
        or lower:find("authentication required", 1, true) then
        local detail
        if hints.login_command then
            detail = "Run `" .. hints.login_command .. "`"
        else
            detail = "Check credentials"
        end
        return error_item({
            label = (hints.service or "Service") .. " auth failed",
            detail = detail,
            help_url = hints.login_help_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("429", 1, true)
        or lower:find("rate limit", 1, true)
        or lower:find("too many requests", 1, true) then
        local retry_after = stderr:match("[Rr]etry%-[Aa]fter:?%s*(%d+)")
        local detail
        if retry_after then
            detail = "Rate limited — retry in " .. retry_after .. "s"
        else
            detail = "Rate limited — try again later"
        end
        return error_item({
            label = (hints.service or "Service") .. " rate limited",
            detail = detail,
            help_url = hints.login_help_url,
            retry_action = hints.retry_action,
        })
    end

    if lower:find("could not resolve host", 1, true)
        or lower:find("getaddrinfo", 1, true)
        or lower:find("name or service not known", 1, true)
        or lower:find("connection refused", 1, true)
        or lower:find("network is unreachable", 1, true)
        or lower:find("no route to host", 1, true) then
        return error_item({
            label = "Network unreachable",
            detail = "Check your connection",
            retry_action = hints.retry_action,
        })
    end

    return nil
end

lark.register({
    on_run = function()
        local res = lark.exec_io("kubectl", { "get", "namespaces", "-o", "json" })

        if res.exit_code ~= 0 or res.stdout == "" then
            return {
                title = "Namespaces",
                level = "warn",
                items = { from_exit(res.stderr, { cli = "kubectl", install_url = "https://kubernetes.io/docs/tasks/tools/" })
                    or error_item({
                        label = "kubectl unavailable or cluster unreachable",
                        detail = "Install kubectl and configure a cluster",
                        help_url = "https://kubernetes.io/docs/tasks/tools/install-kubectl/",
                    }) },
            }
        end
        local raw = res.stdout

        local ok, data = pcall(lark.json.decode, raw)
        if not ok or not data or not data.items then
            return {
                title = "Namespaces",
                items = { error_item({
                    label = "Failed to parse kubectl output",
                    help_url = "https://kubernetes.io/docs/reference/kubectl/",
                }) },
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
