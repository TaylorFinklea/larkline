# AI Integration

Larkline's AI layer turns the same plugin catalog that powers the
keyboard palette into a tool registry an in-app agent can call.

> **Status (2026-05-20):** Phase 5 shipped the [`Provider`] trait and
> four backend implementations (Anthropic, OpenAI Responses,
> OpenRouter, Ollama). The agent module is reachable from Rust but
> not yet surfaced in the TUI — Phase 6 lands the single-shot AI
> plugin, Phase 7 builds the tool registry, Phase 8 wires the agent
> loop with dry-run plan approval. Until then this document is a
> reference for the underlying layer.

## Quick start

1. Decide which provider you want.
2. If it requires an API key, store the key in macOS Keychain:
   ```sh
   lark secret set ANTHROPIC_API_KEY
   ```
   (Or `OPENAI_API_KEY` / `OPENROUTER_API_KEY` — Ollama needs no
   key.)
3. Add an `[ai]` section to `~/.config/larkline/config.toml`:
   ```toml
   [ai]
   provider = "anthropic"     # anthropic | openai | openrouter | ollama
   model    = ""              # blank = provider default
   ```
4. The next Phase 6+ AI plugin uses `agent::build_provider` to pick
   up these settings automatically.

## Config reference

```toml
[ai]
# Active provider. Default: "anthropic".
provider = "anthropic"

# Provider-specific model identifier. Blank string = use the provider
# default (claude-opus-4-7 / gpt-4o / anthropic-claude-3.5-sonnet /
# llama3.2 depending on provider).
model = ""

# Hard cap on output tokens per request. 0 = provider default.
max_tokens = 0

# Override for the OpenRouter base URL. Blank = openrouter.ai/api/v1.
openrouter_base_url = ""

# Override for the Ollama base URL. Blank = http://localhost:11434/v1.
ollama_base_url = ""
```

API keys never live in this file. They flow through the existing
secrets pipeline:

1. `~/.config/larkline/.env` (file, comma-quoted KEY=VALUE)
2. macOS Keychain (`security find-generic-password -s <KEY>`)
3. Process environment (`ANTHROPIC_API_KEY` etc.)

`lark secret set <KEY>` is the recommended path on macOS — it writes
to Keychain so the key is encrypted at rest and survives shell
restarts.

## Provider matrix

| Provider | Endpoint | API key env | Default model | Tool-use | Prompt caching |
|---|---|---|---|---|---|
| `anthropic` | `api.anthropic.com/v1/messages` | `ANTHROPIC_API_KEY` | `claude-opus-4-7` | ✅ | ✅ (last tool def marker) |
| `openai` | `api.openai.com/v1/responses` | `OPENAI_API_KEY` | `gpt-4o` | ✅ | n/a |
| `openrouter` | `openrouter.ai/api/v1/chat/completions` | `OPENROUTER_API_KEY` | `anthropic/claude-3.5-sonnet` | ⚠ varies per model | n/a |
| `ollama` | `localhost:11434/v1/chat/completions` | _(none — local)_ | `llama3.2` | ⚠ tool-capable models only | n/a |

**OpenRouter tool-use:** OpenRouter is a router across many models;
not all support tool-use. Known good: `anthropic/claude-3.5-sonnet`,
`openai/gpt-4o`, `meta-llama/llama-3.3-70b-instruct`. A v1.x
capability filter via `/models` is planned.

**Ollama tool-use:** local OSS models vary. Confirmed tool-capable:
Llama 3.2 (8B/70B), Mistral 7B Instruct v0.3+, Qwen 2.5 (7B+).
Smaller or older models will ignore the tools array and respond in
prose.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Phase 6+ AI plugin (examples/plugins/ai/ask.lua)           │
│     ↓                                                       │
│  agent::build_provider(ai_config, secrets) → Box<dyn        │
│                                              Provider>      │
│     ↓                                                       │
│  Provider::ask(AskRequest, mpsc::UnboundedSender<           │
│                              ProviderEvent>)                │
│     ↓                                                       │
│  ┌──────────────┬──────────────┬───────────────┬─────────┐  │
│  │ Anthropic    │ OpenAI       │ OpenAiChat    │  ...    │  │
│  │ Messages API │ Responses    │ (OpenRouter + │         │  │
│  │              │ API          │  Ollama)      │         │  │
│  └──────────────┴──────────────┴───────────────┴─────────┘  │
│     ↓                                                       │
│  reqwest streaming body → SSE event parser → ProviderEvent  │
│  (TextDelta | ToolUse | Usage | Done)                       │
└─────────────────────────────────────────────────────────────┘
```

### Provider trait

```rust
#[async_trait]
pub trait Provider: Send + Sync + Debug {
    fn name(&self) -> &'static str;
    async fn ask(
        &self,
        request: AskRequest,
        events: tokio::sync::mpsc::UnboundedSender<ProviderEvent>,
    ) -> Result<(), ProviderError>;
}
```

Object-safe so `Box<dyn Provider>` works for runtime selection.

### AskRequest

```rust
pub struct AskRequest {
    pub system: Option<String>,        // top-level for Anthropic;
                                       // "instructions" for OpenAI
                                       // Responses; role:"system" for
                                       // Chat Completions
    pub messages: Vec<Message>,        // conversation history
    pub tools: Vec<ToolDefinition>,    // empty = no tool use
    pub model: String,                 // provider-specific model id
    pub max_tokens: Option<u32>,       // None = provider default
}
```

### Message + ContentBlock

```rust
pub struct Message {
    pub role: Role,              // System | User | Assistant
    pub content: Vec<ContentBlock>,
}

pub enum ContentBlock {
    Text(String),
    ToolUse { id: String, name: String, input: JsonValue },
    ToolResult { tool_use_id: String, content: String, is_error: bool },
}
```

The shape mirrors Anthropic's Messages API because it's the
canonical tool-use design — tool use and tool results are
first-class content blocks. OpenAI providers flatten the blocks back
to their native shapes internally (function_call items for
Responses; tool_calls + role:"tool" for Chat Completions).

### ToolDefinition

```rust
pub struct ToolDefinition {
    pub name: String,         // conventionally {plugin_id}__{command_id}
    pub description: String,  // shown to the model for selection
    pub input_schema: JsonValue,  // JSON Schema for the input object
}
```

Phase 7 builds these from plugin manifests; the `Provider` layer
takes them as a flat list.

### ProviderEvent

```rust
pub enum ProviderEvent {
    TextDelta(String),                          // streaming text fragment
    ToolUse { id, name, args: JsonValue },      // complete tool call
    Usage { input_tokens: u32, output_tokens: u32 },
    Done { stop_reason: StopReason },           // terminal event
}

pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
    Other(String),
}
```

Events stream over `mpsc::UnboundedSender`; the caller drives the
matching receiver. Mirrors the existing
`EngineEvent::PartialOutput` pattern so the TUI can interleave AI
streaming with plugin output rendering.

### ProviderError

```rust
pub enum ProviderError {
    Auth(String),         // missing/invalid API key
    RateLimited(u64),     // retry-after seconds
    Api(String),          // non-2xx response
    Network(String),      // DNS/TLS/timeout
    Malformed(String),    // unparseable response body
    Config(String),       // misconfiguration
}
```

`is_retryable()` returns true for `RateLimited` and `Network`.

## Wire-format quick reference

| Field | Anthropic | OpenAI Responses | OpenAI Chat (OR/Ollama) |
|---|---|---|---|
| System prompt | top-level `system` | top-level `instructions` | `role:"system"` message |
| Message array | `messages` | `input` | `messages` |
| Text content | `{type:"text", text}` | `{type:"input_text"\|"output_text", text}` | string (or null for tool-only) |
| Tool def schema field | `input_schema` | `parameters` (flat) | `function.parameters` (nested) |
| Tool call id field | `id` | `call_id` | `id` |
| Args wire format | JSON value | JSON string | JSON string |
| Stream end marker | `message_stop` event | `response.completed` event | `data: [DONE]` literal |
| Prompt caching | `cache_control` per tool | none | none |

See per-provider source files for full details:
[`src/agent/anthropic.rs`](../src/agent/anthropic.rs),
[`src/agent/openai.rs`](../src/agent/openai.rs),
[`src/agent/openai_chat.rs`](../src/agent/openai_chat.rs).

## Future: tool-use safety model (Phase 7 + 8)

When the in-app agent ships in Phase 8, three layers will gate which
plugins it can call:

1. **Per-plugin opt-in** — manifests will declare
   `agent_callable = true/false` (default `false`). Non-callable
   plugins are invisible to the model.
2. **Per-command destructive flag** — `[[commands]] destructive =
   true` marks a command as state-changing. Destructive tools render
   with a `[!]` marker in the dry-run plan.
3. **Dry-run plan approval** — when the model emits tool calls, the
   agent collects them into a plan and shows it to the user for
   approval before any tool runs. Single Enter approves the entire
   plan; `n` rejects.

Plus an audit log at `$XDG_STATE_HOME/larkline/agent-audit.log`
(timestamp, prompt, tool, args, result-status) for post-hoc review.

None of this exists yet — Phase 5 only ships the provider layer.
Phases 7 + 8 add the safety scaffolding before the agent UI lands.

## Verifying a real provider

The smoke runbook at
[`.docs/ai/phases/v1.0-phase-5-ai-provider-smoke-runbook.md`](../.docs/ai/phases/v1.0-phase-5-ai-provider-smoke-runbook.md)
walks through end-to-end validation: auth smoke, single-prompt
streaming, tool-use round-trip, per-provider quirks (OpenRouter
model capability, Ollama local-server prereqs), and pass criteria.

The unit tests pin the request/response shapes against published
event schemas without requiring API keys — useful when adapting to
wire-format drift. `cargo test --bin lark agent::` runs them.

## Adding a new provider

If you want to add a fifth backend (e.g. Mistral La Plateforme,
Vertex AI, Groq), the pattern is:

1. Add a new file `src/agent/<name>.rs`.
2. Implement `struct <Name>Provider` + `impl Provider for <Name>Provider`.
3. Split out pure functions for request serialization and SSE
   parsing so they're unit-testable without a network call.
4. Add the provider to the `AiProviderName` enum in
   [`src/config.rs`](../src/config.rs) and update
   `AI_SECRET_KEYS` if an API key is needed.
5. Wire it into `agent::build_provider` in
   [`src/agent/mod.rs`](../src/agent/mod.rs).
6. Add unit tests covering at least one of each event type.
7. Add a smoke section to the Phase 5 runbook.

The existing providers (~600 lines each) are good templates. The
trickiest part is usually the streaming event taxonomy — providers
disagree on event names, payload shapes, and whether tool-use args
arrive incrementally or in one piece.
