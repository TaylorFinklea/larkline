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
| `icon` | string | no | Emoji or single char |
| `url` | string | no | URL for open action |
| `actions` | array | no | Item actions (see below) |

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

- **Syntax errors** in your Lua file show as "Lua syntax/load error" in the output pane
- **Runtime errors** (nil access, type errors) show as "on_run error: ..."
- **Missing `lark.register()`** shows "plugin did not call lark.register()"
- **Timeout** shows the standard timeout error

The app never crashes from a plugin error.

## When to Use Lua vs Shell

| Use case | Lua | Shell |
|----------|-----|-------|
| API calls (HTTP) | `lark.http.get()` — fast, async, in-process | `curl` — subprocess overhead |
| System commands | `lark.exec()` | Native, direct |
| JSON handling | `lark.json` — no escaping issues | Must use `jq` to avoid corruption |
| Complex logic | Natural — loops, tables, functions | Bash gets messy fast |
| Existing scripts | Rewrite needed | Drop in directly |

**Rule of thumb:** If your plugin mostly runs shell commands and formats the output, shell is fine. If it does HTTP calls, JSON manipulation, or complex logic, Lua is better.
