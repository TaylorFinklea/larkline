//! Anthropic Messages API provider with SSE streaming + tool-use.
//!
//! Wire reference: <https://docs.anthropic.com/en/api/messages> and
//! <https://docs.anthropic.com/en/api/messages-streaming>.
//!
//! Architecture: the HTTP I/O lives in [`AnthropicProvider::ask`]; the
//! pure transformation logic is split out into:
//!
//! * [`build_request_body`] — adapts an [`AskRequest`] to the Anthropic
//!   request JSON, with prompt caching on the final tool definition.
//! * [`parse_sse_line`] / [`SseEvent`] — line-by-line SSE parser.
//! * [`SseDispatcher`] — accumulates content blocks across delta events
//!   and converts each completed block into a [`ProviderEvent`].
//!
//! These are unit-testable against canonical event traces without
//! requiring a real API key.

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

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
/// Default max_tokens when the user hasn't configured one. Anthropic
/// requires this field; pick a high-enough value that single-shot
/// responses aren't truncated.
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Anthropic Messages provider. One instance per active model.
#[derive(Debug)]
pub struct AnthropicProvider {
    api_key: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    /// Build a provider with the given API key. Returns
    /// [`ProviderError::Auth`] if the key is blank.
    pub fn new(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let api_key = api_key.into();
        if api_key.is_empty() {
            return Err(ProviderError::Auth(
                "ANTHROPIC_API_KEY is not set (run `lark secret set ANTHROPIC_API_KEY`)"
                    .to_string(),
            ));
        }
        let client = reqwest::Client::builder()
            // Generous timeout for the request handshake; the streaming
            // body is read incrementally and not subject to this limit.
            .connect_timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        Ok(Self { api_key, client })
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    async fn ask(
        &self,
        request: AskRequest,
        events: UnboundedSender<ProviderEvent>,
    ) -> Result<(), ProviderError> {
        let body = build_request_body(&request);

        let mut headers = HeaderMap::new();
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&self.api_key)
                .map_err(|_| ProviderError::Auth("invalid API key header".to_string()))?,
        );
        headers.insert("anthropic-version", HeaderValue::from_static(API_VERSION));
        headers.insert("content-type", HeaderValue::from_static("application/json"));

        let response = self
            .client
            .post(API_URL)
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

        // Read the SSE body chunk-by-chunk. reqwest's chunk stream isn't
        // line-aligned, so we buffer until we see a "\n\n" (event
        // terminator) and dispatch each event one at a time.
        let mut stream = response.bytes_stream();
        let mut buf: Vec<u8> = Vec::new();
        let mut dispatcher = SseDispatcher::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ProviderError::Network(e.to_string()))?;
            buf.extend_from_slice(&chunk);
            while let Some(end) = find_event_boundary(&buf) {
                let event_bytes = buf.drain(..end).collect::<Vec<_>>();
                // Drop the trailing "\n\n" terminator.
                buf.drain(..2.min(buf.len()));
                let event_text = String::from_utf8_lossy(&event_bytes);
                dispatch_event(&event_text, &mut dispatcher, &events)?;
            }
        }
        // Flush any trailing event without a terminator (rare).
        if !buf.is_empty() {
            let event_text = String::from_utf8_lossy(&buf);
            dispatch_event(&event_text, &mut dispatcher, &events)?;
        }

        Ok(())
    }
}

/// Locate the end of one SSE event in `buf` — the index of the first
/// "\n\n" byte pair. Returns `None` when no complete event is buffered.
fn find_event_boundary(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n")
}

fn dispatch_event(
    raw: &str,
    dispatcher: &mut SseDispatcher,
    events: &UnboundedSender<ProviderEvent>,
) -> Result<(), ProviderError> {
    // Parse the multi-line event into (event_type, data) — both default
    // to empty if the corresponding line is absent. Anthropic always
    // sends both; the parser tolerates either order.
    let mut event_type = "";
    let mut data = String::new();
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event_type = rest.trim();
        } else if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.trim_start());
        }
    }
    if event_type.is_empty() {
        return Ok(());
    }
    for outgoing in dispatcher.handle(event_type, &data)? {
        // The receiver may have been dropped if the caller cancelled;
        // treat that as a clean shutdown rather than an error.
        if events.send(outgoing).is_err() {
            break;
        }
    }
    Ok(())
}

/// Adapt an [`AskRequest`] to the JSON body Anthropic expects.
///
/// This is split out so unit tests can pin the wire format without
/// touching the network.
#[must_use]
pub fn build_request_body(request: &AskRequest) -> JsonValue {
    let max_tokens = request.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);

    let mut body = json!({
        "model": request.model,
        "max_tokens": max_tokens,
        "stream": true,
        "messages": serialize_messages(&request.messages),
    });

    if let Some(system) = request.system.as_ref().filter(|s| !s.is_empty()) {
        body["system"] = JsonValue::String(system.clone());
    }

    if !request.tools.is_empty() {
        body["tools"] = serialize_tools(&request.tools);
    }

    body
}

/// Anthropic's role-and-blocks message representation. Each content
/// block becomes one of: `{type: "text", text}`, `{type: "tool_use",
/// id, name, input}`, or `{type: "tool_result", tool_use_id, content,
/// is_error}`.
fn serialize_messages(messages: &[Message]) -> JsonValue {
    let arr: Vec<JsonValue> = messages
        .iter()
        .filter(|m| m.role != Role::System) // system handled at top level
        .map(|m| {
            json!({
                "role": role_name(m.role),
                "content": m.content.iter().map(serialize_block).collect::<Vec<_>>(),
            })
        })
        .collect();
    JsonValue::Array(arr)
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

fn serialize_block(block: &ContentBlock) -> JsonValue {
    match block {
        ContentBlock::Text(text) => json!({ "type": "text", "text": text }),
        ContentBlock::ToolUse { id, name, input } => json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": input,
        }),
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content,
            "is_error": is_error,
        }),
    }
}

/// Tool definitions with prompt caching enabled on the final entry.
/// Anthropic's cache_control applies to the named block AND everything
/// preceding it, so marking the last tool covers the whole list — saves
/// ~40-70% on multi-turn agent loops that re-send the same tools.
fn serialize_tools(tools: &[ToolDefinition]) -> JsonValue {
    let last = tools.len().saturating_sub(1);
    let arr: Vec<JsonValue> = tools
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut entry = json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema,
            });
            if i == last {
                entry["cache_control"] = json!({"type": "ephemeral"});
            }
            entry
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
    // Anthropic returns `{"error": {"type": "...", "message": "..."}}`
    // without a structured retry-after; the value lives in the
    // `retry-after` HTTP header which we don't currently capture.
    // Parse a body-level numeric hint as a best effort.
    let val: JsonValue = serde_json::from_str(body).ok()?;
    val.get("retry_after")
        .and_then(JsonValue::as_u64)
        .or_else(|| val.pointer("/error/retry_after").and_then(JsonValue::as_u64))
}

// ---------------------------------------------------------------------------
// SSE event dispatch
// ---------------------------------------------------------------------------

/// Internal SSE event taxonomy we care about. Other event types
/// (`message_start`, `ping`) are silently ignored.
#[derive(Debug, PartialEq, Eq)]
enum SseEvent {
    ContentBlockStart { index: usize, block: BlockKind },
    ContentBlockDelta { index: usize, delta: DeltaKind },
    ContentBlockStop { index: usize },
    MessageDelta { stop_reason: Option<String>, output_tokens: Option<u32> },
    MessageStop,
    Error(String),
}

#[derive(Debug, PartialEq, Eq)]
enum BlockKind {
    Text,
    ToolUse { id: String, name: String },
}

#[derive(Debug, PartialEq, Eq)]
enum DeltaKind {
    Text(String),
    InputJson(String),
}

/// Pull the `index` field out of an event body as a `usize`. Centralizes
/// the missing-field error message and the u64 → usize conversion so
/// every event handler doesn't have to repeat the dance.
fn extract_index(value: &JsonValue) -> Result<usize, ProviderError> {
    let n = value
        .get("index")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| ProviderError::Malformed("missing index".to_string()))?;
    usize::try_from(n).map_err(|_| ProviderError::Malformed(format!("index {n} too large")))
}

/// Parse one SSE event into our internal taxonomy. Returns `None` for
/// event types we don't care about (`ping`, `message_start`, etc).
fn parse_sse_event(event_type: &str, data: &str) -> Result<Option<SseEvent>, ProviderError> {
    if matches!(event_type, "ping" | "message_start") {
        return Ok(None);
    }
    if event_type == "error" {
        return Ok(Some(SseEvent::Error(data.to_string())));
    }
    let value: JsonValue = serde_json::from_str(data)
        .map_err(|e| ProviderError::Malformed(format!("SSE data: {e} in {data}")))?;

    match event_type {
        "content_block_start" => {
            let index = extract_index(&value)?;
            let block = value
                .get("content_block")
                .ok_or_else(|| ProviderError::Malformed("missing content_block".to_string()))?;
            let kind = match block.get("type").and_then(JsonValue::as_str) {
                Some("text") => BlockKind::Text,
                Some("tool_use") => BlockKind::ToolUse {
                    id: block
                        .get("id")
                        .and_then(JsonValue::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    name: block
                        .get("name")
                        .and_then(JsonValue::as_str)
                        .unwrap_or_default()
                        .to_string(),
                },
                other => {
                    return Err(ProviderError::Malformed(format!(
                        "unknown content_block type: {other:?}"
                    )));
                }
            };
            Ok(Some(SseEvent::ContentBlockStart { index, block: kind }))
        }
        "content_block_delta" => {
            let index = extract_index(&value)?;
            let delta = value
                .get("delta")
                .ok_or_else(|| ProviderError::Malformed("missing delta".to_string()))?;
            let kind = match delta.get("type").and_then(JsonValue::as_str) {
                Some("text_delta") => DeltaKind::Text(
                    delta
                        .get("text")
                        .and_then(JsonValue::as_str)
                        .unwrap_or_default()
                        .to_string(),
                ),
                Some("input_json_delta") => DeltaKind::InputJson(
                    delta
                        .get("partial_json")
                        .and_then(JsonValue::as_str)
                        .unwrap_or_default()
                        .to_string(),
                ),
                other => {
                    return Err(ProviderError::Malformed(format!(
                        "unknown delta type: {other:?}"
                    )));
                }
            };
            Ok(Some(SseEvent::ContentBlockDelta { index, delta: kind }))
        }
        "content_block_stop" => {
            let index = extract_index(&value)?;
            Ok(Some(SseEvent::ContentBlockStop { index }))
        }
        "message_delta" => {
            let stop_reason = value
                .pointer("/delta/stop_reason")
                .and_then(JsonValue::as_str)
                .map(str::to_string);
            let output_tokens = value
                .pointer("/usage/output_tokens")
                .and_then(JsonValue::as_u64)
                .and_then(|n| u32::try_from(n).ok());
            Ok(Some(SseEvent::MessageDelta {
                stop_reason,
                output_tokens,
            }))
        }
        "message_stop" => Ok(Some(SseEvent::MessageStop)),
        _ => Ok(None),
    }
}

/// Stateful dispatcher that turns a sequence of [`SseEvent`]s into
/// [`ProviderEvent`]s. Buffers tool-use args across `input_json_delta`
/// events and emits a single `ProviderEvent::ToolUse` on
/// `content_block_stop` so consumers see complete inputs.
struct SseDispatcher {
    /// Per-block state: maps content_block index to its accumulated args.
    /// Only populated for tool_use blocks; text blocks stream directly.
    tool_blocks: HashMap<usize, ToolBlock>,
    output_tokens: Option<u32>,
}

struct ToolBlock {
    id: String,
    name: String,
    partial_json: String,
}

impl SseDispatcher {
    fn new() -> Self {
        Self {
            tool_blocks: HashMap::new(),
            output_tokens: None,
        }
    }

    /// Handle one raw SSE event, returning the [`ProviderEvent`]s to
    /// forward to the caller. May return zero, one, or several events.
    fn handle(
        &mut self,
        event_type: &str,
        data: &str,
    ) -> Result<Vec<ProviderEvent>, ProviderError> {
        let Some(event) = parse_sse_event(event_type, data)? else {
            return Ok(Vec::new());
        };

        let mut out = Vec::new();
        match event {
            SseEvent::ContentBlockStart { index, block } => {
                if let BlockKind::ToolUse { id, name } = block {
                    self.tool_blocks.insert(
                        index,
                        ToolBlock {
                            id,
                            name,
                            partial_json: String::new(),
                        },
                    );
                }
            }
            SseEvent::ContentBlockDelta { index, delta } => match delta {
                DeltaKind::Text(text) => {
                    out.push(ProviderEvent::TextDelta(text));
                }
                DeltaKind::InputJson(partial) => {
                    if let Some(block) = self.tool_blocks.get_mut(&index) {
                        block.partial_json.push_str(&partial);
                    }
                }
            },
            SseEvent::ContentBlockStop { index } => {
                if let Some(block) = self.tool_blocks.remove(&index) {
                    // Empty input is valid (no-argument tool); parse "{}"
                    // explicitly so we still emit a usable Value.
                    let raw = if block.partial_json.is_empty() {
                        "{}"
                    } else {
                        &block.partial_json
                    };
                    let args: JsonValue = serde_json::from_str(raw).map_err(|e| {
                        ProviderError::Malformed(format!(
                            "tool_use input JSON: {e} in {raw}"
                        ))
                    })?;
                    out.push(ProviderEvent::ToolUse {
                        id: block.id,
                        name: block.name,
                        args,
                    });
                }
            }
            SseEvent::MessageDelta {
                stop_reason,
                output_tokens,
            } => {
                if let Some(n) = output_tokens {
                    self.output_tokens = Some(n);
                }
                if let Some(reason) = stop_reason {
                    let normalized = match reason.as_str() {
                        "end_turn" => StopReason::EndTurn,
                        "tool_use" => StopReason::ToolUse,
                        "max_tokens" => StopReason::MaxTokens,
                        "stop_sequence" => StopReason::StopSequence,
                        other => StopReason::Other(other.to_string()),
                    };
                    out.push(ProviderEvent::Done {
                        stop_reason: normalized,
                    });
                }
            }
            SseEvent::MessageStop => {
                if let Some(n) = self.output_tokens.take() {
                    out.push(ProviderEvent::Usage {
                        input_tokens: 0,
                        output_tokens: n,
                    });
                }
            }
            SseEvent::Error(msg) => {
                return Err(ProviderError::Api(format!("Anthropic SSE error: {msg}")));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_body_includes_system_and_max_tokens_default() {
        let req = AskRequest {
            system: Some("be terse".to_string()),
            messages: vec![Message::user("hi")],
            tools: vec![],
            model: "claude-opus-4-7".to_string(),
            max_tokens: None,
        };
        let body = build_request_body(&req);
        assert_eq!(body["model"], json!("claude-opus-4-7"));
        assert_eq!(body["max_tokens"], json!(DEFAULT_MAX_TOKENS));
        assert_eq!(body["stream"], json!(true));
        assert_eq!(body["system"], json!("be terse"));
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn request_body_omits_system_when_blank() {
        let req = AskRequest {
            system: None,
            messages: vec![Message::user("hi")],
            tools: vec![],
            model: "claude-opus-4-7".to_string(),
            max_tokens: Some(512),
        };
        let body = build_request_body(&req);
        assert_eq!(body["max_tokens"], json!(512));
        assert!(body.get("system").is_none());
    }

    #[test]
    fn tools_get_cache_control_on_last_entry() {
        let req = AskRequest {
            system: None,
            messages: vec![],
            tools: vec![
                ToolDefinition {
                    name: "a".to_string(),
                    description: "first".to_string(),
                    input_schema: json!({"type": "object"}),
                },
                ToolDefinition {
                    name: "b".to_string(),
                    description: "second".to_string(),
                    input_schema: json!({"type": "object"}),
                },
            ],
            model: "claude-opus-4-7".to_string(),
            max_tokens: None,
        };
        let body = build_request_body(&req);
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert!(tools[0].get("cache_control").is_none());
        assert_eq!(tools[1]["cache_control"], json!({"type": "ephemeral"}));
    }

    #[test]
    fn parses_text_delta_event() {
        let mut d = SseDispatcher::new();
        let out = d
            .handle(
                "content_block_start",
                r#"{"index": 0, "content_block": {"type": "text", "text": ""}}"#,
            )
            .unwrap();
        assert!(out.is_empty());

        let out = d
            .handle(
                "content_block_delta",
                r#"{"index": 0, "delta": {"type": "text_delta", "text": "Hello"}}"#,
            )
            .unwrap();
        assert_eq!(out, vec![ProviderEvent::TextDelta("Hello".to_string())]);
    }

    #[test]
    fn buffers_tool_use_input_json_across_deltas() {
        let mut d = SseDispatcher::new();
        d.handle(
            "content_block_start",
            r#"{"index": 0, "content_block": {"type": "tool_use", "id": "toolu_1", "name": "get_weather", "input": {}}}"#,
        )
        .unwrap();
        d.handle(
            "content_block_delta",
            r#"{"index": 0, "delta": {"type": "input_json_delta", "partial_json": "{\"city\":"}}"#,
        )
        .unwrap();
        d.handle(
            "content_block_delta",
            r#"{"index": 0, "delta": {"type": "input_json_delta", "partial_json": " \"SF\"}"}}"#,
        )
        .unwrap();
        let out = d
            .handle("content_block_stop", r#"{"index": 0}"#)
            .unwrap();
        assert_eq!(out.len(), 1);
        match &out[0] {
            ProviderEvent::ToolUse { id, name, args } => {
                assert_eq!(id, "toolu_1");
                assert_eq!(name, "get_weather");
                assert_eq!(args, &json!({"city": "SF"}));
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn message_delta_emits_done_with_normalized_stop_reason() {
        let mut d = SseDispatcher::new();
        let out = d
            .handle(
                "message_delta",
                r#"{"delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 42}}"#,
            )
            .unwrap();
        assert_eq!(
            out,
            vec![ProviderEvent::Done {
                stop_reason: StopReason::ToolUse,
            }]
        );
        // Usage emitted on message_stop.
        let out = d.handle("message_stop", "{}").unwrap();
        assert_eq!(
            out,
            vec![ProviderEvent::Usage {
                input_tokens: 0,
                output_tokens: 42,
            }]
        );
    }

    #[test]
    fn unknown_event_type_is_ignored() {
        let mut d = SseDispatcher::new();
        let out = d.handle("ping", "{}").unwrap();
        assert!(out.is_empty());
        let out = d.handle("message_start", r#"{"message": {}}"#).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn provider_constructor_rejects_blank_key() {
        let err = AnthropicProvider::new("").unwrap_err();
        assert!(matches!(err, ProviderError::Auth(_)));
    }

    #[test]
    fn map_http_error_classifies_auth_vs_rate_limit_vs_other() {
        let e = map_http_error(reqwest::StatusCode::UNAUTHORIZED, "{}");
        assert!(matches!(e, ProviderError::Auth(_)));
        let e = map_http_error(reqwest::StatusCode::TOO_MANY_REQUESTS, "{}");
        assert!(matches!(e, ProviderError::RateLimited(_)));
        let e = map_http_error(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "boom");
        assert!(matches!(e, ProviderError::Api(_)));
    }
}
