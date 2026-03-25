-- Nebular News: Trigger Pull — start a manual feed pull.

lark.register({
    on_run = function()
        local url = lark.env("NEBULARNEWS_URL")
        if not url then
            return {
                title = "Trigger Pull",
                items = { { label = "NEBULARNEWS_URL not set", detail = "Add it to ~/.config/larkline/.env", icon = "!" } },
            }
        end

        local token = lark.env("NEBULARNEWS_TOKEN")
        local headers = { ["Content-Type"] = "application/json" }
        if token then
            headers["Authorization"] = "Bearer " .. token
        end

        local resp = lark.http.post(url .. "/api/pull", "", { headers = headers, timeout = 10 })

        if resp.status >= 200 and resp.status < 300 then
            local detail = "HTTP " .. resp.status
            local ok, data = pcall(lark.json.decode, resp.body)
            if ok and data.run_id then
                detail = "Run ID: " .. data.run_id
            end

            return {
                title = "Trigger Pull",
                items = {
                    { label = "Pull triggered successfully", detail = detail, icon = "✅" },
                },
            }
        else
            return {
                title = "Trigger Pull",
                items = {
                    { label = "Failed to trigger pull", detail = "HTTP " .. resp.status, icon = "❌" },
                },
            }
        end
    end,
})
