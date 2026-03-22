-- Markdown Demo — demonstrates rich output rendering.
-- Returns markdown content with headers, code blocks, lists, and links.

lark.register({
    on_run = function()
        return {
            title = "Markdown Demo",
            output_format = "markdown",
            raw_text = [[
# Larkline Markdown Demo

Welcome to the **rich output** rendering demo. This plugin shows how markdown
content is rendered with *styling* and ~~strikethrough~~ support.

## Code Blocks

Fenced code blocks with language tags get syntax highlighting:

```rust
fn main() {
    let greeting = "Hello from Larkline!";
    println!("{greeting}");
}
```

```python
def greet(name: str) -> str:
    return f"Hello, {name}!"

print(greet("Larkline"))
```

## Lists

- First item with **bold** text
- Second item with `inline code`
- Third item with *italic* emphasis

## Links

Check out the [Larkline repo](https://github.com/tfinklea/larkline) for more.

---

> Blockquotes are rendered with a vertical bar prefix.
> They support **formatting** too.

## That's It

Use `j`/`k` to scroll, `Ctrl+D`/`Ctrl+U` for half-page jumps.
Press `t` to toggle between output modes.
]],
        }
    end,
})
