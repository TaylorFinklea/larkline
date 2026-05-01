-- Kubernetes: Contexts — list and switch kubectl contexts.
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
        local raw = lark.exec("kubectl", { "config", "get-contexts", "--no-headers" })

        if not raw or raw == "" then
            return {
                title = "Kubernetes Contexts",
                items = { error_item({
                    label = "No kubectl contexts found",
                    detail = "Run: kubectl config set-context",
                    help_url = "https://kubernetes.io/docs/concepts/configuration/organize-cluster-access-kubeconfig/",
                }) },
            }
        end

        local items = {}
        for line in raw:gmatch("[^\n]+") do
            if not line:match("%S") then goto continue end

            local is_current = line:sub(1, 1) == "*"
            -- Strip * marker and leading whitespace, then take first word as name
            local rest = line:gsub("^%*?%s+", "")
            local name = rest:match("^(%S+)")

            if name then
                items[#items + 1] = {
                    label = name,
                    detail = is_current and "current context" or "",
                    icon = is_current and "★" or "○",
                    copy_text = name,
                    actions = {
                        { label = "Use Context", kind = "shell", args = { "kubectl", "config", "use-context", name }, confirm = false },
                        { label = "Copy Name",   kind = "clipboard", args = { name } },
                    },
                }
            end

            ::continue::
        end

        if #items == 0 then
            return { title = "Kubernetes Contexts", items = { { label = "No contexts found", icon = "📭" } } }
        end

        return { title = "Contexts — " .. #items, items = items }
    end,
})
