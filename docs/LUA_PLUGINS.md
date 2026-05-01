# Lua Plugin Guide

Larkline supports Lua plugins alongside shell script plugins. Lua plugins run inside an embedded Lua 5.4 VM with access to the `lark.*` host API — no subprocess overhead, direct access to async HTTP, and structured output without JSON serialization.

## Plugin Structure

```
~/.config/larkline/plugins/my-plugin/
  manifest.toml
  init.lua
```

The manifest is the same as a script plugin, except `entry` points to a `.lua` file:

```toml
[plugin]
name = "My Plugin"
description = "Does something useful"
version = "0.1.0"
author = "you"
icon = "M"
entry = "init.lua"
timeout_seconds = 10
category = "dev"
```

## Minimal Plugin

```lua
lark.register({
    on_run = function()
        return {
            title = "My Plugin",
            items = {
                { label = "Hello", detail = "from Lua", icon = "L" },
            },
        }
    end,
})
```

Every Lua plugin must call `lark.register()` with a table containing an `on_run` function. The function returns a table matching the `PluginOutput` schema: `title` (string), `items` (array of item tables).

### Item Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `label` | string | yes | Primary text |
| `detail` | string | no | Secondary text (dimmed) |
| `icon` | string | no | Emoji or single char (use `"!"` for error rows) |
| `url` | string | no | URL for open action |
| `actions` | array | no | Item actions (see below) |
| `preview` | string | no | Markdown body for the lark.nvim Telescope preview pane (TUI ignores) |
| `retry_action` | object | no | ItemAction fired by `r` — overrides the default whole-plugin rerun |
| `help_url` | string | no | URL opened by `o` — preferred over `url` for troubleshooting links on error rows |

### Item Actions

```lua
{
    label = "192.168.1.1",
    actions = {
        { label = "Copy", command = "clipboard", args = { "192.168.1.1" } },
        { label = "Open", command = "open", args = { "http://192.168.1.1" } },
    },
}
```

Action `command` values: `"clipboard"`, `"open"`, `"shell"`.

---

## `lark.*` API Reference

### `lark.env(name) -> string | nil`

Read an environment variable. Returns `nil` if not set.

```lua
local token = lark.env("GITHUB_TOKEN")
if not token then
    return { title = "Error", items = { { label = "GITHUB_TOKEN not set" } } }
end
```

### `lark.log(message)`

Log a message at info level. Appears in stderr (hidden when TUI is active). Useful for debugging.

```lua
lark.log("fetching data from API")
```

### `lark.run(command, args?) -> string`

Run a command and return its stdout as a string. Uses `tokio::process::Command` with explicit argument list — no shell interpolation, safe by design. Exposed to Lua as `lark.exec()`.

```lua
local hostname = lark.exec("hostname"):match("^(.-)%s*$")
local df = lark.exec("df", { "-h", "/" })
```

### `lark.json.encode(table) -> string`

Serialize a Lua table to a JSON string.

```lua
local json = lark.json.encode({ key = "value", list = { 1, 2, 3 } })
```

### `lark.json.decode(string) -> table`

Parse a JSON string into a Lua table.

```lua
local data = lark.json.decode('{"name": "lark", "version": 1}')
print(data.name)  -- "lark"
```

### `lark.http.get(url, opts?) -> {status, body}`

Make an HTTP GET request. Returns a table with `status` (number) and `body` (string).

```lua
local resp = lark.http.get("https://api.github.com/user", {
    headers = { Authorization = "token " .. lark.env("GITHUB_TOKEN") },
    timeout = 5,  -- seconds
})

if resp.status == 200 then
    local user = lark.json.decode(resp.body)
    -- use user.login, user.name, etc.
end
```

### `lark.http.post(url, body, opts?) -> {status, body}`

Make an HTTP POST request. Same opts as `get`.

```lua
local resp = lark.http.post("https://api.example.com/data",
    lark.json.encode({ action = "toggle" }),
    { headers = { ["Content-Type"] = "application/json" } }
)
```

---

## Sandboxing

Lua plugins run in a restricted environment:

**Available:** `string`, `table`, `math`, `utf8`, coroutines
**Blocked:** `io` (file I/O), `os.execute`, `os.remove`, `package` (require/modules), `debug`, `loadfile`, `dofile`

All I/O goes through the `lark.*` API. Memory is capped at 32 MB per run.

## Error Handling

### Engine-level failures

- **Syntax errors** in your Lua file show as "Lua syntax/load error" in the output pane
- **Runtime errors** (nil access, type errors) show as "on_run error: ..."
- **Missing `lark.register()`** shows "plugin did not call lark.register()"
- **Timeout** shows the standard timeout error

The app never crashes from a plugin error.

### Structured error rows (v0.15.0+)

When your plugin's expected operation fails (auth missing, API down, CLI not installed), return an error row instead of a hard error. Two optional fields make the failure actionable:

- `retry_action` — an `ItemAction` fired by `r`. Use this for chain-context failures where the default whole-plugin rerun would lose state. Most plugins leave it unset and rely on the standard `r` rerun.
- `help_url` — a URL opened by `o` (takes precedence over `url`). Point at troubleshooting docs: install instructions, auth setup, status pages.

Both surface in the status bar — `[r] retry` and `[o] help` hints appear automatically when the focused item has them.

The canonical Lua helpers live at [`examples/plugins/_shared/errors.lua`](../examples/plugins/_shared/errors.lua) and are inlined into each plugin (the sandbox has no `require`). Two helpers:

```lua
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

-- Usage on a missing-token error:
return {
    title = "My PRs",
    items = { error_item({
        label = "GITHUB_TOKEN not set",
        detail = "Add it to ~/.config/larkline/.env",
        help_url = "https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens",
    }) },
}
```

`from_exit(stderr, hints)` translates known stderr patterns (missing CLI, auth failure, network down, rate limit) into structured error items. It's wired into shell-based plugins for forward compatibility — the host's `lark.exec` returns stdout only today, so the translator activates fully when a stderr-aware exec API ships.

## When to Use Lua vs Shell

| Use case | Lua | Shell |
|----------|-----|-------|
| API calls (HTTP) | `lark.http.get()` — fast, async, in-process | `curl` — subprocess overhead |
| System commands | `lark.exec()` | Native, direct |
| JSON handling | `lark.json` — no escaping issues | Must use `jq` to avoid corruption |
| Complex logic | Natural — loops, tables, functions | Bash gets messy fast |
| Existing scripts | Rewrite needed | Drop in directly |

**Rule of thumb:** If your plugin mostly runs shell commands and formats the output, shell is fine. If it does HTTP calls, JSON manipulation, or complex logic, Lua is better.
