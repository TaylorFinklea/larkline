//! OpenAI Responses API provider with SSE streaming + function calling.
//!
//! Wire reference: OpenAI Responses API (the successor to Chat
//! Completions; see <https://platform.openai.com/docs/api-reference/responses>).
//! Wire-format names cross-referenced against the official Python SDK at
//! github.com/openai/openai-python in `src/openai/types/responses/`.
//!
//! Key differences from Anthropic:
//!
//! * System prompt lives in a top-level `instructions` field, not in
//!   the message array.
//! * The message array is named `input` and items wrap content in
//!   `input_text` / `output_text` parts depending on role.
//! * Tools use `type: "function"` at the top level with
//!   `name` / `description` / `parameters` (not `input_schema`).
//! * Function calls in the response carry a `call_id` separate from
//!   the streaming-protocol `item_id`. The `call_id` is what later
//!   `function_call_output` items must reference.
//! * Function call arguments stream as a JSON *string*, not a JSON
//!   value. The `response.function_call_arguments.done` event carries
//!   the complete arguments string AND the function name, so we don't
//!   need to buffer deltas like the Anthropic provider does.

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

const API_URL: &str = "https://api.openai.com/v1/responses";
/// Default cap when the user hasn't configured one. OpenAI's Responses
/// API doesn't require this field (unlike Anthropic) but supplying a
/// reasonable cap protects against runaway billing on misconfigured
/// agent loops.
const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 4096;

/// OpenAI Responses API provider.
#[derive(Debug)]
pub struct OpenAiResponsesProvider {
    api_key: String,
    client: reqwest::Client,
}

impl OpenAiResponsesProvider {
    /// Build a provider with the given API key. Returns
    /// [`ProviderError::Auth`] if the key is blank.
    pub fn new(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let api_key = api_key.into();
        if api_key.is_empty() {
            return Err(ProviderError::Auth(
                "OPENAI_API_KEY is not set (run `lark secret set OPENAI_API_KEY`)".to_string(),
            ));
        }
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        Ok(Self { api_key, client })
    }
}

#[async_trait]
impl Provider for OpenAiResponsesProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    async fn ask(
        &self,
        request: AskRequest,
        events: UnboundedSender<ProviderEvent>,
    ) -> Result<(), ProviderError> {
        let body = build_request_body(&request);

        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                .map_err(|_| ProviderError::Auth("invalid API key header".to_string()))?,
        );
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

fn dispatch_event(
    raw: &str,
    dispatcher: &mut SseDispatcher,
    events: &UnboundedSender<ProviderEvent>,
) -> Result<(), ProviderError> {
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
        if events.send(outgoing).is_err() {
            break;
        }
    }
    Ok(())
}

/// Adapt an [`AskRequest`] to the JSON body the Responses API expects.
#[must_use]
pub fn build_request_body(request: &AskRequest) -> JsonValue {
    let max_output_tokens = request.max_tokens.unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS);

    let mut body = json!({
        "model": request.model,
        "max_output_tokens": max_output_tokens,
        "stream": true,
        "input": serialize_input(&request.messages),
    });

    if let Some(instructions) = request.system.as_ref().filter(|s| !s.is_empty()) {
        body["instructions"] = JsonValue::String(instructions.clone());
    }

    if !request.tools.is_empty() {
        body["tools"] = serialize_tools(&request.tools);
    }

    body
}

/// Convert our internal Message list to the Responses API `input` array.
/// Each message becomes one or more input items: a "message" item wraps
/// text content, and tool use / tool result blocks become standalone
/// `function_call` / `function_call_output` items respectively.
fn serialize_input(messages: &[Message]) -> JsonValue {
    let mut items: Vec<JsonValue> = Vec::new();
    for msg in messages.iter().filter(|m| m.role != Role::System) {
        // Collect text parts into a single message item; emit standalone
        // function_call / function_call_output items for the non-text
        // blocks. Order is preserved.
        let mut text_parts: Vec<JsonValue> = Vec::new();
        for block in &msg.content {
            match block {
                ContentBlock::Text(text) => {
                    let part_type = if msg.role == Role::Assistant {
                        "output_text"
                    } else {
                        "input_text"
                    };
                    text_parts.push(json!({ "type": part_type, "text": text }));
                }
                ContentBlock::ToolUse { id, name, input } => {
                    // Assistant emits this. The `id` we got from the
                    // provider is the call_id; the Responses API uses
                    // it directly here.
                    items.push(json!({
                        "type": "function_call",
                        "call_id": id,
                        "name": name,
                        // arguments is a JSON-encoded string, not a value.
                        "arguments": serde_json::to_string(input).unwrap_or_else(|_| "{}".to_string()),
                    }));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error: _,
                } => {
                    // is_error has no native equivalent in the Responses
                    // API; callers convey error state via the `output`
                    // string itself.
                    items.push(json!({
                        "type": "function_call_output",
                        "call_id": tool_use_id,
                        "output": content,
                    }));
                }
            }
        }
        if !text_parts.is_empty() {
            items.push(json!({
                "type": "message",
                "role": role_name(msg.role),
                "content": text_parts,
            }));
        }
    }
    JsonValue::Array(items)
}

const fn role_name(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

/// Responses API tool format: `{type: "function", name, description,
/// parameters}` -- parameters takes the place of Anthropic's
/// input_schema, and the `function` type is required at the top level.
fn serialize_tools(tools: &[ToolDefinition]) -> JsonValue {
    let arr: Vec<JsonValue> = tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "name": t.name,
                "description": t.description,
                "parameters": t.input_schema,
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
    val.pointer("/error/retry_after")
        .and_then(JsonValue::as_u64)
}

// ---------------------------------------------------------------------------
// SSE dispatcher
// ---------------------------------------------------------------------------

/// Tracks the per-stream state needed to translate Responses API SSE
/// events into [`ProviderEvent`]s.
///
/// Output items in the response arrive in order; each one gets an
/// `output_index`. For `function_call` items we capture the `call_id`
/// when the item first appears (`response.output_item.added`) and join
/// it with the final `arguments` string on
/// `response.function_call_arguments.done`. Text deltas are
/// independent — each `response.output_text.delta` carries its own
/// content directly.
struct SseDispatcher {
    /// output_index -> call_id for in-flight function_call items.
    function_call_ids: HashMap<u64, String>,
    /// Set when at least one function_call item has appeared in this
    /// response. Drives the stop_reason emitted on completion.
    saw_function_call: bool,
    /// Captured at `response.completed` time; emitted as a single
    /// Usage event so downstream cost tracking sees one snapshot.
    completed_usage: Option<(u32, u32)>,
}

impl SseDispatcher {
    fn new() -> Self {
        Self {
            function_call_ids: HashMap::new(),
            saw_function_call: false,
            completed_usage: None,
        }
    }

    // The match below has one arm per SSE event type; splitting it
    // across helpers would just move the noise around without making
    // the dispatch logic any clearer.
    #[allow(clippy::too_many_lines)]
    fn handle(
        &mut self,
        event_type: &str,
        data: &str,
    ) -> Result<Vec<ProviderEvent>, ProviderError> {
        // Lots of low-value Responses API events fire (in_progress,
        // sequence_number heartbeats, audio.*, refusal.*, etc). Match
        // only the ones we care about; ignore everything else silently.
        match event_type {
            "response.output_item.added" => {
                let value: JsonValue = parse_data(data)?;
                let item_type = value.pointer("/item/type").and_then(JsonValue::as_str);
                if item_type == Some("function_call") {
                    self.saw_function_call = true;
                    if let (Some(idx), Some(call_id)) = (
                        value.get("output_index").and_then(JsonValue::as_u64),
                        value
                            .pointer("/item/call_id")
                            .and_then(JsonValue::as_str)
                            .map(str::to_string),
                    ) {
                        self.function_call_ids.insert(idx, call_id);
                    }
                }
                Ok(Vec::new())
            }
            "response.output_text.delta" => {
                let value: JsonValue = parse_data(data)?;
                let delta = value
                    .get("delta")
                    .and_then(JsonValue::as_str)
                    .unwrap_or_default()
                    .to_string();
                if delta.is_empty() {
                    Ok(Vec::new())
                } else {
                    Ok(vec![ProviderEvent::TextDelta(delta)])
                }
            }
            "response.function_call_arguments.done" => {
                let value: JsonValue = parse_data(data)?;
                let name = value
                    .get("name")
                    .and_then(JsonValue::as_str)
                    .unwrap_or_default()
                    .to_string();
                let arguments = value
                    .get("arguments")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("{}");
                let parsed_args: JsonValue = serde_json::from_str(arguments).map_err(|e| {
                    ProviderError::Malformed(format!(
                        "function_call_arguments.done JSON: {e} in {arguments}"
                    ))
                })?;
                let output_index = value
                    .get("output_index")
                    .and_then(JsonValue::as_u64)
                    .unwrap_or_default();
                // call_id falls back to item_id if we somehow missed the
                // output_item.added event (e.g. provider bug). The
                // distinction matters when later turns need to send
                // function_call_output entries back.
                let call_id = self
                    .function_call_ids
                    .remove(&output_index)
                    .unwrap_or_else(|| {
                        value
                            .get("item_id")
                            .and_then(JsonValue::as_str)
                            .unwrap_or_default()
                            .to_string()
                    });
                Ok(vec![ProviderEvent::ToolUse {
                    id: call_id,
                    name,
                    args: parsed_args,
                }])
            }
            "response.completed" => {
                let value: JsonValue = parse_data(data)?;
                // Usage lives at /response/usage; status at /response/status.
                let input_tokens = value
                    .pointer("/response/usage/input_tokens")
                    .and_then(JsonValue::as_u64)
                    .and_then(|n| u32::try_from(n).ok())
                    .unwrap_or_default();
                let output_tokens = value
                    .pointer("/response/usage/output_tokens")
                    .and_then(JsonValue::as_u64)
                    .and_then(|n| u32::try_from(n).ok())
                    .unwrap_or_default();
                self.completed_usage = Some((input_tokens, output_tokens));

                let status = value
                    .pointer("/response/status")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("completed");
                let incomplete_reason = value
                    .pointer("/response/incomplete_details/reason")
                    .and_then(JsonValue::as_str);

                // Map status + incomplete_details.reason to our normalized
                // StopReason. The Responses API uses `incomplete` with a
                // reason instead of dedicated stop_reason variants.
                let stop_reason = match (status, incomplete_reason) {
                    ("incomplete", Some("max_output_tokens")) => StopReason::MaxTokens,
                    ("incomplete", Some(other)) => StopReason::Other(other.to_string()),
                    _ if self.saw_function_call => StopReason::ToolUse,
                    ("completed", _) => StopReason::EndTurn,
                    (other, _) => StopReason::Other(other.to_string()),
                };

                let mut out = vec![ProviderEvent::Done { stop_reason }];
                if let Some((input_tokens, output_tokens)) = self.completed_usage.take() {
                    out.push(ProviderEvent::Usage {
                        input_tokens,
                        output_tokens,
                    });
                }
                Ok(out)
            }
            "response.error" => {
                let value: JsonValue = parse_data(data)?;
                let msg = value
                    .pointer("/error/message")
                    .and_then(JsonValue::as_str)
                    .unwrap_or(data)
                    .to_string();
                Err(ProviderError::Api(format!("OpenAI SSE error: {msg}")))
            }
            _ => Ok(Vec::new()),
        }
    }
}

fn parse_data(data: &str) -> Result<JsonValue, ProviderError> {
    serde_json::from_str(data).map_err(|e| ProviderError::Malformed(format!("SSE data: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_body_uses_instructions_not_system_role() {
        let req = AskRequest {
            system: Some("be terse".to_string()),
            messages: vec![Message::user("hi")],
            tools: vec![],
            model: "gpt-4o".to_string(),
            max_tokens: None,
        };
        let body = build_request_body(&req);
        assert_eq!(body["model"], json!("gpt-4o"));
        assert_eq!(body["instructions"], json!("be terse"));
        assert_eq!(body["max_output_tokens"], json!(DEFAULT_MAX_OUTPUT_TOKENS));
        assert_eq!(body["stream"], json!(true));
        // Input is an array; system is NOT a message entry.
        let input = body["input"].as_array().unwrap();
        assert_eq!(input.len(), 1);
        assert_eq!(input[0]["type"], json!("message"));
        assert_eq!(input[0]["role"], json!("user"));
        assert_eq!(input[0]["content"][0]["type"], json!("input_text"));
    }

    #[test]
    fn assistant_text_uses_output_text_part_type() {
        let req = AskRequest {
            system: None,
            messages: vec![Message::assistant("hello back")],
            tools: vec![],
            model: "gpt-4o".to_string(),
            max_tokens: None,
        };
        let body = build_request_body(&req);
        let input = body["input"].as_array().unwrap();
        assert_eq!(input[0]["content"][0]["type"], json!("output_text"));
    }

    #[test]
    fn tool_use_block_emits_function_call_item() {
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
        let input = body["input"].as_array().unwrap();
        assert_eq!(input[0]["type"], json!("function_call"));
        assert_eq!(input[0]["call_id"], json!("call_xyz"));
        assert_eq!(input[0]["name"], json!("get_weather"));
        // arguments is a JSON-encoded STRING in the Responses API.
        assert_eq!(input[0]["arguments"], json!(r#"{"city":"SF"}"#));
    }

    #[test]
    fn tool_result_block_emits_function_call_output_item() {
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
        let input = body["input"].as_array().unwrap();
        assert_eq!(input[0]["type"], json!("function_call_output"));
        assert_eq!(input[0]["call_id"], json!("call_xyz"));
        assert_eq!(input[0]["output"], json!("72F"));
    }

    #[test]
    fn tools_use_top_level_function_type_and_parameters() {
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
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["type"], json!("function"));
        assert_eq!(tools[0]["name"], json!("get_weather"));
        // Note: `parameters`, not `input_schema` like Anthropic.
        assert_eq!(tools[0]["parameters"], json!({"type": "object"}));
    }

    #[test]
    fn text_delta_event_emits_provider_text_delta() {
        let mut d = SseDispatcher::new();
        let out = d
            .handle(
                "response.output_text.delta",
                r#"{"item_id": "x", "output_index": 0, "content_index": 0, "delta": "Hello"}"#,
            )
            .unwrap();
        assert_eq!(out, vec![ProviderEvent::TextDelta("Hello".to_string())]);
    }

    #[test]
    fn function_call_done_emits_tool_use_with_call_id_from_added_event() {
        let mut d = SseDispatcher::new();
        // First the output_item.added supplies the call_id.
        d.handle(
            "response.output_item.added",
            r#"{"output_index": 1, "item": {"type": "function_call", "call_id": "call_99", "name": "get_weather", "arguments": ""}}"#,
        )
        .unwrap();
        // Then function_call_arguments.done supplies the final args.
        let out = d
            .handle(
                "response.function_call_arguments.done",
                r#"{"item_id": "fc_42", "output_index": 1, "name": "get_weather", "arguments": "{\"city\":\"SF\"}"}"#,
            )
            .unwrap();
        assert_eq!(out.len(), 1);
        match &out[0] {
            ProviderEvent::ToolUse { id, name, args } => {
                assert_eq!(id, "call_99");
                assert_eq!(name, "get_weather");
                assert_eq!(args, &json!({"city": "SF"}));
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn completed_event_emits_done_and_usage_with_tool_use_when_seen() {
        let mut d = SseDispatcher::new();
        d.handle(
            "response.output_item.added",
            r#"{"output_index": 0, "item": {"type": "function_call", "call_id": "c1", "name": "x", "arguments": ""}}"#,
        )
        .unwrap();
        let out = d
            .handle(
                "response.completed",
                r#"{"response": {"status": "completed", "usage": {"input_tokens": 10, "output_tokens": 5}}}"#,
            )
            .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(
            out[0],
            ProviderEvent::Done {
                stop_reason: StopReason::ToolUse,
            }
        );
        assert_eq!(
            out[1],
            ProviderEvent::Usage {
                input_tokens: 10,
                output_tokens: 5,
            }
        );
    }

    #[test]
    fn completed_event_emits_max_tokens_when_incomplete() {
        let mut d = SseDispatcher::new();
        let out = d
            .handle(
                "response.completed",
                r#"{"response": {"status": "incomplete", "incomplete_details": {"reason": "max_output_tokens"}, "usage": {"input_tokens": 0, "output_tokens": 0}}}"#,
            )
            .unwrap();
        assert_eq!(
            out[0],
            ProviderEvent::Done {
                stop_reason: StopReason::MaxTokens,
            }
        );
    }

    #[test]
    fn error_event_maps_to_provider_error() {
        let mut d = SseDispatcher::new();
        let err = d
            .handle("response.error", r#"{"error": {"message": "rate limit"}}"#)
            .unwrap_err();
        assert!(matches!(err, ProviderError::Api(_)));
    }

    #[test]
    fn unknown_event_types_are_silently_ignored() {
        let mut d = SseDispatcher::new();
        assert!(d.handle("response.in_progress", "{}").unwrap().is_empty());
        assert!(
            d.handle("response.audio.delta", r#"{"delta": "x"}"#)
                .unwrap()
                .is_empty()
        );
        assert!(d.handle("response.created", "{}").unwrap().is_empty());
    }

    #[test]
    fn provider_constructor_rejects_blank_key() {
        let err = OpenAiResponsesProvider::new("").unwrap_err();
        assert!(matches!(err, ProviderError::Auth(_)));
    }
}
