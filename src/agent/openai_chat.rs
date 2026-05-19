//! OpenAI Chat Completions transport (shared by OpenRouter + Ollama).
//!
//! OpenRouter and Ollama both expose an OpenAI-compatible Chat
//! Completions endpoint at `<base_url>/chat/completions`. This module
//! implements a single [`OpenAiChatProvider`] parametrized by base URL
//! and optional API key so both backends share the wire format,
//! serializer, and SSE dispatcher.
//!
//! Differences from the Responses API (see `openai.rs`):
//!
//! * Endpoint is `/chat/completions`, not `/responses`.
//! * Request uses `messages` (not `input`) and a `system`-role entry
//!   for the system prompt.
//! * Tools wrap their function spec under a `function` key:
//!   `{type: "function", function: {name, description, parameters}}`.
//! * Tool calls in assistant messages live in a `tool_calls` array,
//!   each with `id` (matches a later `tool_call_id`) and a nested
//!   `function.arguments` JSON string.
//! * Tool results use `role: "tool"` messages with `tool_call_id`.
//! * Streaming events have no named-event line — every chunk is a
//!   `data: <json>` line and the stream ends with literal
//!   `data: [DONE]`. `finish_reason` in the last chunk signals the
//!   stop reason.
//! * Usage tokens require `stream_options.include_usage = true` to
//!   appear in the stream; we always request them.

use std::collections::HashMap;

use async_trait::async_trait;
use futures_util::StreamExt as _;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::{Value as JsonValue, json};
use tokio::sync::mpsc::UnboundedSender;

use crate::agent::error::ProviderError;
use crate::agent::provider::{
    AskRequest, ContentBlock, Message, Provider, ProviderEvent, Role, StopReason, ToolDefinition,
};

/// Default cap when the user hasn't configured one.
const DEFAULT_MAX_COMPLETION_TOKENS: u32 = 4096;

/// Shared Chat Completions provider. The OpenRouter and Ollama backends
/// are constructed via the [`OpenAiChatProvider::openrouter`] and
/// [`OpenAiChatProvider::ollama`] factories.
#[derive(Debug)]
pub struct OpenAiChatProvider {
    base_url: String,
    /// `None` for Ollama (local server, no auth); `Some` for OpenRouter.
    api_key: Option<String>,
    provider_name: &'static str,
    client: reqwest::Client,
}

impl OpenAiChatProvider {
    /// Build a provider for OpenRouter. `base_url` defaults to
    /// `https://openrouter.ai/api/v1` when blank.
    pub fn openrouter(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        let api_key = api_key.into();
        if api_key.is_empty() {
            return Err(ProviderError::Auth(
                "OPENROUTER_API_KEY is not set (run `lark secret set OPENROUTER_API_KEY`)"
                    .to_string(),
            ));
        }
        let base_url = base_url.into();
        let base_url = if base_url.is_empty() {
            "https://openrouter.ai/api/v1".to_string()
        } else {
            base_url
        };
        let client = build_client()?;
        Ok(Self {
            base_url,
            api_key: Some(api_key),
            provider_name: "openrouter",
            client,
        })
    }

    /// Build a provider for Ollama (local OpenAI-compatible server).
    /// `base_url` defaults to `http://localhost:11434/v1` when blank.
    /// No API key — the local server doesn't require auth.
    pub fn ollama(base_url: impl Into<String>) -> Result<Self, ProviderError> {
        let base_url = base_url.into();
        let base_url = if base_url.is_empty() {
            "http://localhost:11434/v1".to_string()
        } else {
            base_url
        };
        let client = build_client()?;
        Ok(Self {
            base_url,
            api_key: None,
            provider_name: "ollama",
            client,
        })
    }
}

fn build_client() -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| ProviderError::Network(e.to_string()))
}

#[async_trait]
impl Provider for OpenAiChatProvider {
    fn name(&self) -> &'static str {
        self.provider_name
    }

    async fn ask(
        &self,
        request: AskRequest,
        events: UnboundedSender<ProviderEvent>,
    ) -> Result<(), ProviderError> {
        let body = build_request_body(&request);

        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        if let Some(api_key) = self.api_key.as_ref() {
            headers.insert(
                "authorization",
                HeaderValue::from_str(&format!("Bearer {api_key}"))
                    .map_err(|_| ProviderError::Auth("invalid API key header".to_string()))?,
            );
            // OpenRouter encourages a few optional headers for usage
            // tracking but rejects nothing when they're absent.
            headers.insert(
                "http-referer",
                HeaderValue::from_static("https://github.com/TaylorFinklea/larkline"),
            );
            headers.insert("x-title", HeaderValue::from_static("Larkline"));
        }

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body_text));
        }

        let mut stream = response.bytes_stream();
        let mut buf: Vec<u8> = Vec::new();
        let mut dispatcher = SseDispatcher::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ProviderError::Network(e.to_string()))?;
            buf.extend_from_slice(&chunk);
            while let Some(end) = find_event_boundary(&buf) {
                let event_bytes = buf.drain(..end).collect::<Vec<_>>();
                buf.drain(..2.min(buf.len()));
                let event_text = String::from_utf8_lossy(&event_bytes);
                dispatch_event(&event_text, &mut dispatcher, &events)?;
            }
        }
        if !buf.is_empty() {
            let event_text = String::from_utf8_lossy(&buf);
            dispatch_event(&event_text, &mut dispatcher, &events)?;
        }

        Ok(())
    }
}

fn find_event_boundary(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n")
}

/// Chat Completions wraps every event as a single `data: <json>` line
/// (or `data: [DONE]` to terminate). No named event types.
fn dispatch_event(
    raw: &str,
    dispatcher: &mut SseDispatcher,
    events: &UnboundedSender<ProviderEvent>,
) -> Result<(), ProviderError> {
    for line in raw.lines() {
        let Some(rest) = line.strip_prefix("data:") else {
            continue;
        };
        let payload = rest.trim_start();
        if payload == "[DONE]" {
            // The protocol terminator; nothing to emit. finish_reason
            // from the previous chunk already produced Done/Usage.
            return Ok(());
        }
        for outgoing in dispatcher.handle_chunk(payload)? {
            if events.send(outgoing).is_err() {
                return Ok(());
            }
        }
    }
    Ok(())
}

/// Adapt an [`AskRequest`] to the JSON body Chat Completions expects.
#[must_use]
pub fn build_request_body(request: &AskRequest) -> JsonValue {
    let max_tokens = request.max_tokens.unwrap_or(DEFAULT_MAX_COMPLETION_TOKENS);

    let mut messages = serialize_messages(&request.messages);
    if let Some(system) = request.system.as_ref().filter(|s| !s.is_empty()) {
        // Prepend a system-role message so callers don't have to.
        messages.insert(
            0,
            json!({"role": "system", "content": system}),
        );
    }

    let mut body = json!({
        "model": request.model,
        "max_completion_tokens": max_tokens,
        "stream": true,
        // Without this flag the last chunk has empty usage; with it,
        // a dedicated final chunk carries input/output token counts.
        "stream_options": {"include_usage": true},
        "messages": JsonValue::Array(messages),
    });

    if !request.tools.is_empty() {
        body["tools"] = serialize_tools(&request.tools);
    }

    body
}

fn serialize_messages(messages: &[Message]) -> Vec<JsonValue> {
    messages
        .iter()
        .filter(|m| m.role != Role::System)
        .flat_map(serialize_message)
        .collect()
}

/// Adapt one internal message to one or more Chat Completions messages.
/// Most messages produce a single entry; tool results split into a
/// separate `role: "tool"` message per tool result block because Chat
/// Completions expects them as standalone messages, not embedded blocks.
fn serialize_message(msg: &Message) -> Vec<JsonValue> {
    let mut out: Vec<JsonValue> = Vec::new();

    // Tool result blocks become standalone role:"tool" messages.
    let mut text_buf = String::new();
    let mut tool_calls: Vec<JsonValue> = Vec::new();

    for block in &msg.content {
        match block {
            ContentBlock::Text(text) => {
                if !text_buf.is_empty() {
                    text_buf.push('\n');
                }
                text_buf.push_str(text);
            }
            ContentBlock::ToolUse { id, name, input } => {
                tool_calls.push(json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        // arguments is a JSON-encoded STRING.
                        "arguments": serde_json::to_string(input).unwrap_or_else(|_| "{}".to_string()),
                    },
                }));
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                // Chat Completions has no is_error flag — prefix the
                // content so the model can recover.
                let body = if *is_error {
                    format!("ERROR: {content}")
                } else {
                    content.clone()
                };
                out.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": body,
                }));
            }
        }
    }

    // Emit the main message (text + tool_calls) if it has any payload.
    let has_text = !text_buf.is_empty();
    let has_tool_calls = !tool_calls.is_empty();
    if has_text || has_tool_calls {
        let mut entry = json!({"role": role_name(msg.role)});
        // OpenAI requires `content` to be either a string or null.
        // Empty assistant turns that consist only of tool_calls send
        // content: null.
        entry["content"] = if has_text {
            JsonValue::String(text_buf)
        } else {
            JsonValue::Null
        };
        if has_tool_calls {
            entry["tool_calls"] = JsonValue::Array(tool_calls);
        }
        // Insert the main message before any standalone tool messages
        // already pushed -- ordering must be assistant-with-tool_calls
        // *before* the matching role:tool replies.
        out.insert(0, entry);
    }

    out
}

const fn role_name(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

/// Chat Completions tool format: `{type: "function", function:
/// {name, description, parameters}}` -- function is nested.
fn serialize_tools(tools: &[ToolDefinition]) -> JsonValue {
    let arr: Vec<JsonValue> = tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema,
                },
            })
        })
        .collect();
    JsonValue::Array(arr)
}

fn map_http_error(status: reqwest::StatusCode, body: &str) -> ProviderError {
    match status.as_u16() {
        401 | 403 => ProviderError::Auth(format!("HTTP {status}: {body}")),
        429 => {
            let retry = extract_retry_after(body).unwrap_or(60);
            ProviderError::RateLimited(retry)
        }
        _ => ProviderError::Api(format!("HTTP {status}: {body}")),
    }
}

fn extract_retry_after(body: &str) -> Option<u64> {
    let val: JsonValue = serde_json::from_str(body).ok()?;
    val.pointer("/error/retry_after").and_then(JsonValue::as_u64)
}

// ---------------------------------------------------------------------------
// SSE dispatcher
// ---------------------------------------------------------------------------

/// State for accumulating one in-flight tool call. Chat Completions
/// streams tool calls incrementally: the first chunk carries `id` and
/// `function.name`, subsequent chunks carry `function.arguments` as
/// partial JSON. We emit `ProviderEvent::ToolUse` only when
/// `finish_reason: "tool_calls"` arrives.
struct ToolCallBuilder {
    id: String,
    name: String,
    arguments: String,
}

struct SseDispatcher {
    /// Maps the tool_calls `index` to its in-flight accumulator.
    tool_calls: HashMap<u64, ToolCallBuilder>,
}

impl SseDispatcher {
    fn new() -> Self {
        Self {
            tool_calls: HashMap::new(),
        }
    }

    fn handle_chunk(&mut self, payload: &str) -> Result<Vec<ProviderEvent>, ProviderError> {
        let value: JsonValue = serde_json::from_str(payload)
            .map_err(|e| ProviderError::Malformed(format!("chunk JSON: {e}")))?;

        // Usage-only chunks have empty choices but populated usage.
        let mut out = Vec::new();
        if let Some(usage) = value.get("usage").filter(|v| !v.is_null()) {
            let input_tokens = usage
                .get("prompt_tokens")
                .and_then(JsonValue::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or_default();
            let output_tokens = usage
                .get("completion_tokens")
                .and_then(JsonValue::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or_default();
            // Only emit usage when we have real numbers; some
            // intermediate chunks include `usage: null`.
            if input_tokens > 0 || output_tokens > 0 {
                out.push(ProviderEvent::Usage {
                    input_tokens,
                    output_tokens,
                });
            }
        }

        let Some(choices) = value.get("choices").and_then(JsonValue::as_array) else {
            return Ok(out);
        };
        for choice in choices {
            // Text delta -- emit immediately.
            if let Some(text) = choice
                .pointer("/delta/content")
                .and_then(JsonValue::as_str)
                .filter(|s| !s.is_empty())
            {
                out.push(ProviderEvent::TextDelta(text.to_string()));
            }

            // Tool-call delta -- accumulate by index.
            if let Some(arr) = choice
                .pointer("/delta/tool_calls")
                .and_then(JsonValue::as_array)
            {
                for tc in arr {
                    let Some(idx) = tc.get("index").and_then(JsonValue::as_u64) else {
                        continue;
                    };
                    let entry = self.tool_calls.entry(idx).or_insert_with(|| ToolCallBuilder {
                        id: String::new(),
                        name: String::new(),
                        arguments: String::new(),
                    });
                    if let Some(id) = tc.get("id").and_then(JsonValue::as_str) {
                        if !id.is_empty() {
                            entry.id = id.to_string();
                        }
                    }
                    if let Some(name) = tc.pointer("/function/name").and_then(JsonValue::as_str) {
                        if !name.is_empty() {
                            entry.name = name.to_string();
                        }
                    }
                    if let Some(args_chunk) = tc
                        .pointer("/function/arguments")
                        .and_then(JsonValue::as_str)
                    {
                        entry.arguments.push_str(args_chunk);
                    }
                }
            }

            // finish_reason on the last chunk for this choice signals
            // both stop_reason and (for tool_calls) "now emit the
            // accumulated tool-use events".
            if let Some(reason) = choice
                .get("finish_reason")
                .and_then(JsonValue::as_str)
                .filter(|s| !s.is_empty())
            {
                if reason == "tool_calls" {
                    // Drain accumulated tool calls in index order so
                    // they reach the agent loop deterministically.
                    let mut entries: Vec<_> = self.tool_calls.drain().collect();
                    entries.sort_by_key(|(k, _)| *k);
                    for (_, builder) in entries {
                        let args_str = if builder.arguments.is_empty() {
                            "{}"
                        } else {
                            &builder.arguments
                        };
                        let parsed_args: JsonValue =
                            serde_json::from_str(args_str).map_err(|e| {
                                ProviderError::Malformed(format!(
                                    "tool_call arguments JSON: {e} in {args_str}"
                                ))
                            })?;
                        out.push(ProviderEvent::ToolUse {
                            id: builder.id,
                            name: builder.name,
                            args: parsed_args,
                        });
                    }
                }
                out.push(ProviderEvent::Done {
                    stop_reason: map_finish_reason(reason),
                });
            }
        }
        Ok(out)
    }
}

fn map_finish_reason(reason: &str) -> StopReason {
    match reason {
        "stop" => StopReason::EndTurn,
        "tool_calls" => StopReason::ToolUse,
        "length" => StopReason::MaxTokens,
        "stop_sequence" => StopReason::StopSequence,
        other => StopReason::Other(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn openrouter_factory_rejects_blank_key_and_picks_default_url() {
        let err = OpenAiChatProvider::openrouter("", "").unwrap_err();
        assert!(matches!(err, ProviderError::Auth(_)));
        let p = OpenAiChatProvider::openrouter("k", "").unwrap();
        assert_eq!(p.base_url, "https://openrouter.ai/api/v1");
        assert_eq!(p.name(), "openrouter");
    }

    #[test]
    fn ollama_factory_has_no_api_key_and_picks_default_url() {
        let p = OpenAiChatProvider::ollama("").unwrap();
        assert!(p.api_key.is_none());
        assert_eq!(p.base_url, "http://localhost:11434/v1");
        assert_eq!(p.name(), "ollama");
    }

    #[test]
    fn system_prompt_becomes_role_system_message_prepended() {
        let req = AskRequest {
            system: Some("be terse".to_string()),
            messages: vec![Message::user("hi")],
            tools: vec![],
            model: "gpt-4o".to_string(),
            max_tokens: None,
        };
        let body = build_request_body(&req);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], json!("system"));
        assert_eq!(msgs[0]["content"], json!("be terse"));
        assert_eq!(msgs[1]["role"], json!("user"));
    }

    #[test]
    fn tool_use_block_becomes_assistant_tool_calls() {
        let req = AskRequest {
            system: None,
            messages: vec![Message {
                role: Role::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "call_xyz".to_string(),
                    name: "get_weather".to_string(),
                    input: json!({"city": "SF"}),
                }],
            }],
            tools: vec![],
            model: "gpt-4o".to_string(),
            max_tokens: None,
        };
        let body = build_request_body(&req);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], json!("assistant"));
        // content is null when only tool_calls are present.
        assert_eq!(msgs[0]["content"], JsonValue::Null);
        let tcs = msgs[0]["tool_calls"].as_array().unwrap();
        assert_eq!(tcs[0]["id"], json!("call_xyz"));
        assert_eq!(tcs[0]["type"], json!("function"));
        assert_eq!(tcs[0]["function"]["name"], json!("get_weather"));
        // arguments is a JSON-encoded STRING.
        assert_eq!(tcs[0]["function"]["arguments"], json!(r#"{"city":"SF"}"#));
    }

    #[test]
    fn tool_result_block_becomes_role_tool_message() {
        let req = AskRequest {
            system: None,
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "call_xyz".to_string(),
                    content: "72F".to_string(),
                    is_error: false,
                }],
            }],
            tools: vec![],
            model: "gpt-4o".to_string(),
            max_tokens: None,
        };
        let body = build_request_body(&req);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], json!("tool"));
        assert_eq!(msgs[0]["tool_call_id"], json!("call_xyz"));
        assert_eq!(msgs[0]["content"], json!("72F"));
    }

    #[test]
    fn tools_wrap_function_in_nested_object() {
        let req = AskRequest {
            system: None,
            messages: vec![],
            tools: vec![ToolDefinition {
                name: "get_weather".to_string(),
                description: "Returns current weather.".to_string(),
                input_schema: json!({"type": "object"}),
            }],
            model: "gpt-4o".to_string(),
            max_tokens: None,
        };
        let body = build_request_body(&req);
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools[0]["type"], json!("function"));
        // Function spec is nested under "function", unlike Responses API.
        assert_eq!(tools[0]["function"]["name"], json!("get_weather"));
        assert_eq!(tools[0]["function"]["parameters"], json!({"type": "object"}));
    }

    #[test]
    fn stream_options_include_usage_is_set() {
        let req = AskRequest {
            system: None,
            messages: vec![],
            tools: vec![],
            model: "gpt-4o".to_string(),
            max_tokens: None,
        };
        let body = build_request_body(&req);
        assert_eq!(body["stream_options"]["include_usage"], json!(true));
    }

    #[test]
    fn text_delta_chunk_emits_provider_text_delta() {
        let mut d = SseDispatcher::new();
        let out = d
            .handle_chunk(r#"{"choices": [{"index": 0, "delta": {"content": "Hello"}}]}"#)
            .unwrap();
        assert_eq!(out, vec![ProviderEvent::TextDelta("Hello".to_string())]);
    }

    #[test]
    fn tool_calls_accumulate_across_chunks_and_emit_on_finish() {
        let mut d = SseDispatcher::new();
        // Chunk 1: tool_call header.
        d.handle_chunk(
            r#"{"choices": [{"delta": {"tool_calls": [{"index": 0, "id": "call_1", "type": "function", "function": {"name": "get_weather", "arguments": ""}}]}}]}"#,
        )
        .unwrap();
        // Chunk 2: partial args.
        d.handle_chunk(
            r#"{"choices": [{"delta": {"tool_calls": [{"index": 0, "function": {"arguments": "{\"city\":"}}]}}]}"#,
        )
        .unwrap();
        // Chunk 3: more args.
        d.handle_chunk(
            r#"{"choices": [{"delta": {"tool_calls": [{"index": 0, "function": {"arguments": " \"SF\"}"}}]}}]}"#,
        )
        .unwrap();
        // Chunk 4: finish_reason triggers emission.
        let out = d
            .handle_chunk(r#"{"choices": [{"delta": {}, "finish_reason": "tool_calls"}]}"#)
            .unwrap();
        assert_eq!(out.len(), 2);
        match &out[0] {
            ProviderEvent::ToolUse { id, name, args } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "get_weather");
                assert_eq!(args, &json!({"city": "SF"}));
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
        assert_eq!(
            out[1],
            ProviderEvent::Done {
                stop_reason: StopReason::ToolUse,
            }
        );
    }

    #[test]
    fn finish_reason_stop_maps_to_end_turn() {
        let mut d = SseDispatcher::new();
        let out = d
            .handle_chunk(r#"{"choices": [{"delta": {}, "finish_reason": "stop"}]}"#)
            .unwrap();
        assert_eq!(
            out,
            vec![ProviderEvent::Done {
                stop_reason: StopReason::EndTurn,
            }]
        );
    }

    #[test]
    fn finish_reason_length_maps_to_max_tokens() {
        assert_eq!(map_finish_reason("length"), StopReason::MaxTokens);
    }

    #[test]
    fn usage_chunk_emits_usage_event() {
        let mut d = SseDispatcher::new();
        let out = d
            .handle_chunk(r#"{"choices": [], "usage": {"prompt_tokens": 30, "completion_tokens": 12, "total_tokens": 42}}"#)
            .unwrap();
        assert_eq!(
            out,
            vec![ProviderEvent::Usage {
                input_tokens: 30,
                output_tokens: 12,
            }]
        );
    }

    #[test]
    fn null_usage_chunk_is_silent() {
        let mut d = SseDispatcher::new();
        let out = d
            .handle_chunk(r#"{"choices": [{"delta": {"content": "x"}}], "usage": null}"#)
            .unwrap();
        // Just the text delta, no usage event.
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], ProviderEvent::TextDelta(_)));
    }
}
