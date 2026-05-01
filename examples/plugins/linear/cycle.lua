-- Linear: Current Cycle — active cycle issues and progress.
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
