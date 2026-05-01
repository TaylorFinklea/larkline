-- Linear: Triage — issues in your team's triage queue.
-- SHARED: error_item(), gql(), state_icon(), priority_label() from lib.lua

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

local function linear_http_error(status, extra)
    if status == 401 or status == 403 then
        return error_item({
            label = "Linear auth failed",
            detail = "API key invalid or expired" .. (extra and (" · " .. extra) or ""),
            help_url = "https://linear.app/settings/api",
        })
    end
    if status == 429 then
        return error_item({
            label = "Linear rate limited",
            detail = "Try again in a minute" .. (extra and (" · " .. extra) or ""),
            help_url = "https://linear.app/docs/api-and-webhooks#rate-limiting",
        })
    end
    return error_item({
        label = "Linear API error",
        detail = "HTTP " .. tostring(status) .. (extra and (" · " .. extra) or ""),
        help_url = "https://linear.app/docs/api-and-webhooks",
    })
end

local function gql(query, variables)
    local token = lark.env("LINEAR_API_KEY")
    if not token then
        return nil, { error_item({
            label = "LINEAR_API_KEY not set",
            detail = "Add it to ~/.config/larkline/.env",
            help_url = "https://linear.app/docs/api-and-webhooks#personal-api-keys",
        }) }
    end
    local url = "https://api.linear.app/graphql"
    local body = lark.json.encode({ query = query, variables = variables or {} })
    local resp = lark.http.post(url, body, {
        headers = { Authorization = token, ["Content-Type"] = "application/json" },
        timeout = 10,
    })
    if not resp or resp.status == nil then
        return nil, { error_item({
            label = "No response from Linear",
            detail = url,
            help_url = "https://linear.app/docs/api-and-webhooks",
        }) }
    end
    if resp.status ~= 200 then
        return nil, { linear_http_error(resp.status) }
    end
    local ok, data = pcall(lark.json.decode, resp.body)
    if not ok or not data then return nil, { error_item({ label = "Failed to parse Linear response" }) } end
    if data.errors then
        local msg = data.errors[1] and data.errors[1].message or "Unknown error"
        return nil, { error_item({
            label = "Linear API error",
            detail = msg,
            help_url = "https://linear.app/docs/api-and-webhooks",
        }) }
    end
    return data.data, nil
end

local function priority_label(p)
    if p == 1 then return "urgent" end
    if p == 2 then return "high" end
    if p == 3 then return "medium" end
    if p == 4 then return "low" end
    return nil
end

local QUERY = [[
query Triage {
  issues(filter: { state: { type: { eq: "triage" } } }, first: 30) {
    nodes {
      id
      identifier
      title
      url
      priority
      createdAt
      team { key name }
    }
  }
}
]]

lark.register({
    on_run = function()
        local data, err = gql(QUERY, {})
        if not data then return { title = "Triage", items = err } end

        local issues = data.issues and data.issues.nodes or {}
        if #issues == 0 then
            return { title = "Triage", items = { { label = "Triage empty — nice work!", icon = "✅" } } }
        end

        local items = {}
        for _, issue in ipairs(issues) do
            local team = issue.team and issue.team.key or "?"
            local url = issue.url or ""
            local created = (issue.createdAt or ""):sub(1, 10)

            local detail_parts = { team .. "  " .. (issue.identifier or "?"), "created " .. created }
            local prio = priority_label(issue.priority)
            if prio then detail_parts[#detail_parts + 1] = prio end

            items[#items + 1] = {
                label = issue.title or "?",
                detail = table.concat(detail_parts, " · "),
                icon = "⚠",
                url = url,
                copy_text = url,
                actions = {
                    { label = "Open in browser", kind = "open", args = { url } },
                    { label = "Copy URL", kind = "clipboard", args = { url } },
                    { label = "Copy identifier", kind = "clipboard", args = { issue.identifier or "" } },
                },
            }
        end

        return { title = "Triage — " .. #items, items = items }
    end,
})
