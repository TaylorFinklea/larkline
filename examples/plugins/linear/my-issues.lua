-- Linear: My Issues — issues assigned to the current user.
-- SHARED: gql(), state_icon(), priority_label() from lib.lua

local function gql(query, variables)
    local token = lark.env("LINEAR_API_KEY")
    if not token then
        return nil, { { label = "LINEAR_API_KEY not set", detail = "Add to env or Keychain", icon = "!" } }
    end
    local body = lark.json.encode({ query = query, variables = variables or {} })
    local resp = lark.http.post("https://api.linear.app/graphql", body, {
        headers = { Authorization = token, ["Content-Type"] = "application/json" },
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
        return nil, { { label = "GraphQL error", detail = data.errors[1].message, icon = "!" } }
    end
    return data.data, nil
end

local function state_icon(t)
    if t == "completed" then return "✅" end
    if t == "canceled" then return "⊘" end
    if t == "started" then return "▶" end
    if t == "unstarted" then return "○" end
    if t == "backlog" then return "◌" end
    if t == "triage" then return "⚠" end
    return "●"
end

local function priority_label(p)
    if p == 1 then return "urgent" end
    if p == 2 then return "high" end
    if p == 3 then return "medium" end
    if p == 4 then return "low" end
    return nil
end

local QUERY = [[
query MyIssues {
  viewer {
    assignedIssues(filter: { state: { type: { nin: ["completed", "canceled"] } } }, first: 30) {
      nodes {
        id
        identifier
        title
        url
        priority
        state { type name }
        team { key name }
        project { name }
      }
    }
  }
}
]]

lark.register({
    on_run = function()
        local data, err = gql(QUERY, {})
        if not data then return { title = "My Linear Issues", items = err } end

        local issues = data.viewer and data.viewer.assignedIssues and data.viewer.assignedIssues.nodes or {}
        if #issues == 0 then
            return { title = "My Linear Issues", items = { { label = "No open issues", icon = "✅" } } }
        end

        local items = {}
        for _, issue in ipairs(issues) do
            local state = issue.state or {}
            local team = issue.team and issue.team.key or "?"
            local project = issue.project and issue.project.name or nil

            local detail_parts = { team .. "  " .. (issue.identifier or "?") }
            detail_parts[#detail_parts + 1] = state.name or "unknown"
            local prio = priority_label(issue.priority)
            if prio then detail_parts[#detail_parts + 1] = prio end
            if project then detail_parts[#detail_parts + 1] = project end

            local url = issue.url or ""
            items[#items + 1] = {
                label = issue.title or "?",
                detail = table.concat(detail_parts, " · "),
                icon = state_icon(state.type),
                url = url,
                copy_text = url,
                actions = {
                    { label = "Open in browser", kind = "open", args = { url } },
                    { label = "Copy URL", kind = "clipboard", args = { url } },
                    { label = "Copy identifier", kind = "clipboard", args = { issue.identifier or "" } },
                },
            }
        end

        return { title = "My Linear Issues — " .. #items, items = items }
    end,
})
