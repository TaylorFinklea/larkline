-- Kubernetes: Logs — view recent logs from pods (uses stored namespace or default).
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
        -- Get pods from current namespace context.
        local ns_raw = lark.exec("kubectl", { "config", "view", "--minify", "-o", "jsonpath={.contexts[0].context.namespace}" })
        local ns = (ns_raw and ns_raw ~= "") and ns_raw or "default"

        local res = lark.exec_io("kubectl", { "get", "pods", "-n", ns, "-o", "json" })

        if res.exit_code ~= 0 or res.stdout == "" then
            return {
                title = "Logs (" .. ns .. ")",
                level = "warn",
                items = { from_exit(res.stderr, { cli = "kubectl", install_url = "https://kubernetes.io/docs/tasks/tools/" })
                    or error_item({
                        label = "kubectl unavailable or no pods in " .. ns,
                        detail = "Switch namespace or check cluster",
                        help_url = "https://kubernetes.io/docs/tasks/tools/install-kubectl/",
                    }) },
            }
        end
        local raw = res.stdout

        local ok, data = pcall(lark.json.decode, raw)
        if not ok or not data or not data.items then
            return {
                title = "Logs (" .. ns .. ")",
                items = { error_item({
                    label = "Failed to parse kubectl output",
                    help_url = "https://kubernetes.io/docs/reference/kubectl/",
                }) },
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
