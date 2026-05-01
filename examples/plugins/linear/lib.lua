-- Linear: shared helpers for GraphQL API calls and issue formatting.
-- SYNC INSTRUCTIONS: Copy helpers into each command file that uses them
-- (sandbox has no require). This file is the canonical source.

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

-- Build a friendly error item from a Linear HTTP response.
-- Common: 401/403 (auth), 429 (rate limit), other (generic).
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

-- Execute a GraphQL query. Returns (data, nil) or (nil, error_items).
local function gql(query, variables)
    local token = lark.env("LINEAR_API_KEY")
    if not token then
        return nil, {
            error_item({
                label = "LINEAR_API_KEY not set",
                detail = "Add it to ~/.config/larkline/.env",
                help_url = "https://linear.app/docs/api-and-webhooks#personal-api-keys",
            }),
        }
    end

    local url = "https://api.linear.app/graphql"
    local body = lark.json.encode({ query = query, variables = variables or {} })
    local resp = lark.http.post(url, body, {
        headers = {
            Authorization = token,
            ["Content-Type"] = "application/json",
        },
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
    if not ok or not data then
        return nil, { error_item({ label = "Failed to parse Linear response" }) }
    end

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

-- Map Linear issue state type to icon.
local function state_icon(state_type)
    if state_type == "completed" then return "✅" end
    if state_type == "canceled" then return "⊘" end
    if state_type == "started" then return "▶" end
    if state_type == "unstarted" then return "○" end
    if state_type == "backlog" then return "◌" end
    if state_type == "triage" then return "⚠" end
    return "●"
end

-- Map priority number to label.
local function priority_label(p)
    if p == 1 then return "urgent" end
    if p == 2 then return "high" end
    if p == 3 then return "medium" end
    if p == 4 then return "low" end
    return nil
end
