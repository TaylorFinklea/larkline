-- Kubernetes: Services — all services across namespaces.
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
        local raw = lark.exec("kubectl", { "get", "svc", "-A", "-o", "json" })

        if not raw or raw == "" then
            return {
                title = "Kubernetes Services",
                items = { error_item({
                    label = "kubectl unavailable or cluster unreachable",
                    detail = "Install kubectl and configure a cluster",
                    help_url = "https://kubernetes.io/docs/tasks/tools/install-kubectl/",
                }) },
            }
        end

        local ok, data = pcall(lark.json.decode, raw)
        if not ok or not data or not data.items then
            return {
                title = "Kubernetes Services",
                items = { error_item({
                    label = "Failed to parse kubectl output",
                    help_url = "https://kubernetes.io/docs/reference/kubectl/",
                }) },
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
