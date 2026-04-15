-- Linear: shared helpers for GraphQL API calls and issue formatting.
-- SYNC INSTRUCTIONS: Copy helpers into each command file that uses them
-- (sandbox has no require). This file is the canonical source.

-- Execute a GraphQL query. Returns (data, nil) or (nil, error_items).
local function gql(query, variables)
    local token = lark.env("LINEAR_API_KEY")
    if not token then
        return nil, {
            { label = "LINEAR_API_KEY not set", detail = "Add it to ~/.config/larkline/.env or Keychain", icon = "!" },
        }
    end

    local body = lark.json.encode({ query = query, variables = variables or {} })
    local resp = lark.http.post("https://api.linear.app/graphql", body, {
        headers = {
            Authorization = token,
            ["Content-Type"] = "application/json",
        },
        timeout = 10,
    })

    if resp.status ~= 200 then
        return nil, { { label = "Linear API error", detail = "HTTP " .. resp.status, icon = "!" } }
    end

    local ok, data = pcall(lark.json.decode, resp.body)
    if not ok or not data then
        return nil, { { label = "Failed to parse response", icon = "!" } }
    end

    if data.errors then
        local msg = data.errors[1] and data.errors[1].message or "Unknown error"
        return nil, { { label = "GraphQL error", detail = msg, icon = "!" } }
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
