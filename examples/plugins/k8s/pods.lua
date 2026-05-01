-- Kubernetes: Pods — all pods across namespaces with status, logs, and describe actions.
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

local function phase_icon(phase, ready_count, total_count)
    if phase == "Running" then
        if ready_count == total_count then return "●" end
        return "◐"  -- partially ready
    end
    if phase == "Succeeded" then return "✅" end
    if phase == "Pending" then return "◌" end
    if phase == "Failed" then return "✗" end
    if phase == "Unknown" then return "?" end
    return "○"
end

local function container_summary(pod)
    if not pod.status or not pod.status.containerStatuses then return 0, 0, 0 end
    local ready, total, restarts = 0, 0, 0
    for _, cs in ipairs(pod.status.containerStatuses) do
        total = total + 1
        if cs.ready then ready = ready + 1 end
        restarts = restarts + (cs.restartCount or 0)
    end
    return ready, total, restarts
end

local function age_from_timestamp(ts)
    if not ts then return "?" end
    local result = lark.exec("date", { "-jf", "%Y-%m-%dT%H:%M:%SZ", ts, "+%s" })
    if not result or result == "" then return "?" end
    local now = tonumber(lark.exec("date", { "+%s" })) or 0
    local diff = now - (tonumber(result:gsub("%s+$", "")) or 0)
    if diff < 60 then return diff .. "s" end
    if diff < 3600 then return math.floor(diff / 60) .. "m" end
    if diff < 86400 then return math.floor(diff / 3600) .. "h" end
    return math.floor(diff / 86400) .. "d"
end

lark.register({
    on_run = function()
        local raw = lark.exec("kubectl", { "get", "pods", "-A", "-o", "json" })

        if not raw or raw == "" then
            return {
                title = "Kubernetes Pods",
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
                title = "Kubernetes Pods",
                items = { error_item({
                    label = "Failed to parse kubectl output",
                    help_url = "https://kubernetes.io/docs/reference/kubectl/",
                }) },
            }
        end

        local items = {}
        for _, pod in ipairs(data.items) do
            local ns   = (pod.metadata and pod.metadata.namespace) or "?"
            local name = (pod.metadata and pod.metadata.name) or "?"
            local phase = (pod.status and pod.status.phase) or "Unknown"
            local created = pod.metadata and pod.metadata.creationTimestamp

            local ready, total, restarts = container_summary(pod)
            local icon = phase_icon(phase, ready, total)

            local detail_parts = { ns, phase, ready .. "/" .. total .. " ready" }
            if restarts > 0 then
                detail_parts[#detail_parts + 1] = "↺" .. restarts
            end
            local age = age_from_timestamp(created)
            if age ~= "?" then
                detail_parts[#detail_parts + 1] = age
            end

            -- Node name for context.
            local node = (pod.spec and pod.spec.nodeName) or nil
            if node then
                detail_parts[#detail_parts + 1] = "on:" .. node
            end

            items[#items + 1] = {
                label = name,
                detail = table.concat(detail_parts, " · "),
                icon = icon,
                copy_text = name,
                actions = {
                    { label = "Logs (last 100)", kind = "shell", args = { "kubectl", "logs", "-n", ns, name, "--tail=100" } },
                    { label = "Logs (follow)", kind = "shell", args = { "kubectl", "logs", "-n", ns, name, "-f", "--tail=50" } },
                    { label = "Describe", kind = "shell", args = { "kubectl", "describe", "pod", "-n", ns, name } },
                    { label = "Exec shell", kind = "shell", args = { "kubectl", "exec", "-it", "-n", ns, name, "--", "/bin/sh" } },
                    { label = "Delete Pod", kind = "shell", args = { "kubectl", "delete", "pod", "-n", ns, name }, confirm = true },
                    { label = "Copy Name", kind = "clipboard", args = { name } },
                    { label = "Copy Namespace", kind = "clipboard", args = { ns } },
                },
            }
        end

        if #items == 0 then
            return { title = "Kubernetes Pods", items = { { label = "No pods found", icon = "📭" } } }
        end

        return { title = "Pods — " .. #items, items = items }
    end,
})
