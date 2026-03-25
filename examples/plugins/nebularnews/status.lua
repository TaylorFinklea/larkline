-- Nebular News: Status — health check and last pull status.

lark.register({
    on_run = function()
        local url = lark.env("NEBULARNEWS_URL")
        if not url then
            return {
                title = "Nebular News",
                items = { { label = "NEBULARNEWS_URL not set", detail = "Add it to ~/.config/larkline/.env", icon = "!" } },
            }
        end

        local token = lark.env("NEBULARNEWS_TOKEN")
        local headers = {}
        if token then
            headers["Authorization"] = "Bearer " .. token
        end

        -- Health check.
        local health = lark.http.get(url .. "/api/health", { headers = headers, timeout = 5 })
        local health_ok = health.status == 200

        -- Pull status.
        local pull_resp = lark.http.get(url .. "/api/pull/status", { headers = headers, timeout = 5 })
        local pull_info = nil
        if pull_resp.status == 200 then
            local ok, data = pcall(lark.json.decode, pull_resp.body)
            if ok then pull_info = data end
        end

        local items = {}
        items[#items + 1] = {
            label = health_ok and "Healthy" or "Unhealthy",
            detail = "Service at " .. url,
            icon = health_ok and "✅" or "❌",
        }

        if pull_info then
            local status_text = pull_info.status or pull_info.state or "unknown"
            local last_run = pull_info.last_run or pull_info.updated_at or "?"
            items[#items + 1] = {
                label = "Last pull: " .. status_text,
                detail = last_run,
                icon = "📥",
            }
        end

        items[#items + 1] = {
            label = "Open Dashboard",
            icon = "🌐",
            url = url,
            actions = {
                { label = "Open in browser", kind = "open", args = { url } },
            },
        }

        return { title = "Nebular News", items = items }
    end,
})
