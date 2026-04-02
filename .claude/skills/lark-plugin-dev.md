# Lark Plugin Development

## When to use this skill

Use when the user asks to create, modify, debug, or understand a Lark plugin. Triggers on:
- "create a plugin", "new plugin", "build a plugin for lark"
- "write a lark plugin that..."
- Editing files in `examples/plugins/` or `~/.config/larkline/plugins/`
- Debugging Lua plugin errors mentioning `lark.*` or `on_run`

## Plugin Structure

Every plugin is a directory with `manifest.toml` + one or more `.lua` (or `.sh`) entry files.

```
~/.config/larkline/plugins/my-plugin/
├── manifest.toml    # Required: metadata + commands
└── init.lua         # Entry point (or multiple .lua for multi-command)
```

### manifest.toml — Single command

```toml
[plugin]
name = "My Plugin"
description = "What it does"
version = "0.1.0"
author = "you"
icon = "🔧"
icon_nerd = "󰛓"
entry = "init.lua"
timeout_seconds = 10
prefetch = true          # Run on startup for cached results
category = "tools"       # dev, system, tools, home, info, demo
```

### manifest.toml — Multi-command

```toml
[plugin]
name = "My Plugin"
description = "A multi-command plugin"
version = "0.1.0"
author = "you"
icon = "🔧"
icon_nerd = "󰛓"
category = "dev"
secrets = ["MY_API_KEY"]  # Declared secrets (advisory)

[[settings]]
id = "base_url"
label = "API URL"
type = "text"
default = "https://api.example.com"

[[commands]]
name = "List"
description = "List all items"
entry = "list.lua"
quickkey = "ml"           # Quick-launch alias
timeout_seconds = 10
prefetch = true
widget = true             # Show as dashboard widget card
widget_refresh_secs = 60  # Auto-refresh interval

[[commands]]
name = "Create"
description = "Create a new item"
entry = "create.lua"
timeout_seconds = 10
```

## Lua Plugin API (lark.*)

### Core

```lua
lark.register({ on_run = function() ... end })  -- Entry point
lark.exec("cmd", {"arg1", "arg2"})               -- Run subprocess, returns string
lark.env("KEY")                                    -- .env, env var, Keychain
lark.log("message")                                -- Tracing log
```

### HTTP

```lua
local resp = lark.http.get(url, { headers = h, timeout = 5 })
-- resp = { status = 200, body = "..." }
-- ALWAYS use resp.body, check resp.status

local resp = lark.http.post(url, body_string, { headers = h, timeout = 5 })
```

### JSON

```lua
local json_str = lark.json.encode({ key = "value" })
local ok, data = pcall(lark.json.decode, resp.body)  -- ALWAYS use pcall
```

### Persistent Store

```lua
lark.store.get("key")        -- value or nil
lark.store.set("key", value) -- Persists to disk
lark.store.delete("key")
lark.store.keys()             -- table of strings
```

### Form Values

```lua
if lark.form_values then
    local input = lark.form_values.field_id
    -- Process form submission
else
    -- Return form for user input
    return { title = "X", form = { fields = {...}, submit_label = "Go" } }
end
```

## Return Format

### Item list (most common)

```lua
return {
    title = "My Plugin",
    items = {
        {
            label = "Item name",
            detail = "Secondary info",
            icon = "📦",
            copy_text = "text to copy on y",
            actions = {
                { label = "Open", kind = "shell", args = {"open", url} },
                { label = "Copy", kind = "clipboard", args = {"text"} },
            },
        },
    },
}
```

### Raw text (ANSI-colored output, scrollable)

```lua
return { title = "My Output", raw_text = "ANSI text here" }
```

### Form (collect user input, plugin re-runs with values)

```lua
return {
    title = "Input",
    form = {
        fields = {
            { id = "name", label = "Name", type = { kind = "text" }, required = true },
            { id = "type", label = "Type", type = { kind = "select", options = {"a", "b"} } },
            { id = "flag", label = "Enable", type = { kind = "toggle" } },
        },
        submit_label = "Submit",
    },
}
```

## Sandbox Constraints

- Available: string, table, math, utf8, tostring, tonumber, type, pairs, ipairs, pcall, error
- NOT available: os, io, require, loadfile, dofile
- Use lark.env("HOME") not os.getenv("HOME")
- lark.http returns { status, body } not a raw string
- Memory limit: 32 MB per plugin

## Common Patterns

### Graceful CLI check

```lua
local which = lark.exec("which", { "mytool" })
if not which or not which:match("mytool") then
    return { title = "X", items = { { label = "mytool not installed", icon = "!" } } }
end
```

### Filter loop with goto

```lua
for _, item in ipairs(items) do
    if type(item.id) ~= "string" then goto next end
    -- process item
    ::next::
end
```

### Settings from store with quote stripping

```lua
local raw = lark.store.get("setting_id") or "default"
if type(raw) == "string" and raw:sub(1,1) == '"' then
    raw = raw:sub(2, -2)
end
```

## Testing

- CLI: `lark invoke "Plugin Name"` to test output
- In-app: Developer > Test Plugin command
- Scaffolding: `lark init-plugin my-plugin` or Developer > New Plugin
