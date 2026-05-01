-- Larkline shared error helpers — canonical copy.
--
-- The Lark Lua sandbox does not expose require/dofile/loadfile, so plugins
-- cannot load this file directly. Instead, plugins copy the relevant helpers
-- inline (typically into their own `lib.lua`) and add a `-- SHARED:` marker so
-- divergence stays diff-able. Pattern matches `examples/plugins/github/lib.lua`
-- and `examples/plugins/docker/lib.lua`.
--
-- Two helpers are provided:
--
--   error_item(opts)       -> { label, detail, icon = "!", retry_action, help_url }
--     Builds a structured error item. `opts` accepts:
--       label         (required)  text shown in the row
--       detail        (optional)  dimmed sub-text
--       retry_action  (optional)  ItemAction fired on `r` / <C-r>
--       help_url      (optional)  URL opened on `?` / <C-?>
--       icon          (optional)  defaults to "!"
--
--   from_exit(stderr, hints) -> error_item | nil
--     Translates a shell process's stderr into a friendly error item by
--     matching known patterns. Returns nil when no pattern matched, so the
--     caller can fall through to raw stderr passthrough. (The exit code is
--     intentionally not part of the signature — no current translator needs
--     it. Callers can route on `code` themselves before delegating here.)
--     `hints` lets the caller plug in plugin-specific affordances:
--       cli            CLI name (for "X not found" labels)
--       install_url    URL placed in help_url + "Install: <url>" detail
--       service        Friendly service name (for "X auth failed" labels)
--       login_command  Suggested command in detail (e.g. `gh auth login`)
--       login_help_url URL placed in help_url for auth/rate-limit failures
--       retry_action   ItemAction wired through to the produced error item
--
-- Returning the module table makes this file `dofile`-able from Rust tests
-- (see tests/plugin_error_translator_test.rs) without affecting plugins, who
-- copy the function bodies inline.

local M = {}

-- Build a structured error item.
function M.error_item(opts)
    return {
        label = opts.label,
        detail = opts.detail,
        icon = opts.icon or "!",
        retry_action = opts.retry_action,
        help_url = opts.help_url,
    }
end

-- Translate a shell process's stderr into a friendly error item.
-- Returns nil if no pattern matched.
function M.from_exit(stderr, hints)
    hints = hints or {}
    stderr = stderr or ""
    local lower = stderr:lower()

    -- Missing CLI: "command not found", "No such file or directory" on the binary.
    if lower:find("command not found", 1, true)
        or lower:find("no such file or directory", 1, true) then
        local cli = hints.cli or "command"
        local detail
        if hints.install_url then
            detail = "Install: " .. hints.install_url
        else
            detail = "Check your $PATH"
        end
        return M.error_item({
            label = cli .. " not found",
            detail = detail,
            help_url = hints.install_url,
            retry_action = hints.retry_action,
        })
    end

    -- Auth failure: HTTP 401/403, "unauthorized", "forbidden", "not authenticated",
    -- "not logged in", "authentication required".
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
        return M.error_item({
            label = (hints.service or "Service") .. " auth failed",
            detail = detail,
            help_url = hints.login_help_url,
            retry_action = hints.retry_action,
        })
    end

    -- Rate limit: HTTP 429, "rate limit", "too many requests". Try to extract a
    -- Retry-After seconds value from the stderr.
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
        return M.error_item({
            label = (hints.service or "Service") .. " rate limited",
            detail = detail,
            help_url = hints.login_help_url,
            retry_action = hints.retry_action,
        })
    end

    -- Network down: DNS, connection refused, network unreachable.
    if lower:find("could not resolve host", 1, true)
        or lower:find("getaddrinfo", 1, true)
        or lower:find("name or service not known", 1, true)
        or lower:find("connection refused", 1, true)
        or lower:find("network is unreachable", 1, true)
        or lower:find("no route to host", 1, true) then
        return M.error_item({
            label = "Network unreachable",
            detail = "Check your connection",
            retry_action = hints.retry_action,
        })
    end

    return nil
end

return M
