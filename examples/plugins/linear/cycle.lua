-- Linear: Current Cycle — active cycle issues and progress.
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
    if not ok or not data then return nil, { { label = "Failed to parse response", icon = "!" } } end
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
    return "●"
end

local QUERY = [[
query CurrentCycle {
  viewer {
    teams {
      nodes {
        key
        name
        activeCycle {
          id
          name
          number
          startsAt
          endsAt
          progress
          issues(first: 50) {
            nodes {
              id
              identifier
              title
              url
              priority
              state { type name }
              assignee { name }
            }
          }
        }
      }
    }
  }
}
]]

lark.register({
    on_run = function()
        local data, err = gql(QUERY, {})
        if not data then return { title = "Current Cycle", items = err } end

        local teams = data.viewer and data.viewer.teams and data.viewer.teams.nodes or {}
        local items = {}
        local total_issues = 0

        for _, team in ipairs(teams) do
            local cycle = team.activeCycle
            if cycle then
                local progress_pct = math.floor((cycle.progress or 0) * 100)
                items[#items + 1] = {
                    label = team.key .. " · Cycle " .. (cycle.number or "?") .. " · " .. progress_pct .. "%",
                    detail = (cycle.name or "") .. "  (" .. (cycle.startsAt or ""):sub(1, 10) .. " → " .. (cycle.endsAt or ""):sub(1, 10) .. ")",
                    icon = "📅",
                }

                local cycle_issues = cycle.issues and cycle.issues.nodes or {}
                for _, issue in ipairs(cycle_issues) do
                    local state = issue.state or {}
                    local assignee = issue.assignee and issue.assignee.name or "unassigned"
                    local url = issue.url or ""

                    items[#items + 1] = {
                        label = "  " .. (issue.title or "?"),
                        detail = (issue.identifier or "?") .. " · " .. (state.name or "") .. " · " .. assignee,
                        icon = state_icon(state.type),
                        url = url,
                        copy_text = url,
                        actions = {
                            { label = "Open in browser", kind = "open", args = { url } },
                            { label = "Copy URL", kind = "clipboard", args = { url } },
                        },
                    }
                    total_issues = total_issues + 1
                end
            end
        end

        if #items == 0 then
            return { title = "Current Cycle", items = { { label = "No active cycles", icon = "📭" } } }
        end

        return { title = "Current Cycle — " .. total_issues .. " issues", items = items }
    end,
})
