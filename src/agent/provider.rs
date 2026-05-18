//! `Provider` trait + shared message/tool/event types.
//!
//! The trait is intentionally minimal: a provider takes an [`AskRequest`]
//! and streams [`ProviderEvent`]s back over an unbounded mpsc sender.
//! Provider implementations adapt the shared message/tool shape to each
//! API's wire format (Anthropic Messages, OpenAI Responses, OpenAI Chat
//! Completions for OpenRouter/Ollama).
//!
//! The shape mirrors Anthropic's Messages API because it's the canonical
//! tool-use design — `content` is a list of typed blocks, tool use and
//! tool results are first-class. OpenAI Chat/Responses providers flatten
//! the blocks back to their native shape internally.

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use crate::agent::error::ProviderError;

/// Who authored a message in the conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Conversation-level system prompt. Anthropic puts this in a
    /// separate top-level field; OpenAI uses a `system` role message.
    System,
    /// Human / agent loop input.
    User,
    /// Model output.
    Assistant,
}

/// A single content block within a [`Message`]. The Anthropic Messages
/// API uses these as a list; OpenAI providers flatten them.
#[derive(Debug, Clone, PartialEq)]
pub enum ContentBlock {
    /// Plain prose. The most common block.
    Text(String),
    /// Model wants to invoke a tool. Emitted in assistant messages.
    ToolUse {
        /// Provider-issued opaque id for matching tool_result back.
        id: String,
        /// Tool name (must match a `ToolDefinition.name`).
        name: String,
        /// JSON arguments to pass to the tool.
        input: JsonValue,
    },
    /// Result of running a tool the model requested. Sent in the next
    /// user message after the assistant emitted a `ToolUse`.
    ToolResult {
        /// Matches the `ToolUse.id` from the assistant message.
        tool_use_id: String,
        /// Rendered tool output (typically the plugin's JSON response
        /// serialized to a string, but providers accept arbitrary text).
        content: String,
        /// True when the tool failed; lets the model recover gracefully
        /// instead of treating an error blob as a successful result.
        is_error: bool,
    },
}

/// One turn in the conversation.
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

impl Message {
    /// Convenience constructor for a user message with a single text block.
    #[must_use]
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::Text(text.into())],
        }
    }

    /// Convenience constructor for an assistant message with a single
    /// text block. Used to extend conversation history.
    #[must_use]
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlock::Text(text.into())],
        }
    }
}

/// A tool the model is allowed to invoke. The schema follows JSON Schema
/// conventions — Anthropic's `input_schema` and OpenAI's `parameters`
/// both accept this shape. Phase 7 builds these from plugin manifests.
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    /// Tool identifier the model returns in `ToolUse.name`. Conventionally
    /// `{plugin_id}__{command_id}` so the dispatcher can route directly.
    pub name: String,
    /// Human-readable description shown to the model. Drives selection.
    pub description: String,
    /// JSON Schema for the input object. Use `{}` for no-argument tools.
    pub input_schema: JsonValue,
}

/// Bundle of everything a provider needs for one request. Pulled out of
/// the trait method signature so `Provider` stays object-safe.
#[derive(Debug, Clone)]
pub struct AskRequest {
    /// Conversation-level system prompt. Routed to whichever wire field
    /// the provider expects (Anthropic top-level vs OpenAI system role).
    pub system: Option<String>,
    /// Message history in chronological order. The newest message is the
    /// turn the model is responding to.
    pub messages: Vec<Message>,
    /// Tool registry the model may call. Empty means no tool use.
    pub tools: Vec<ToolDefinition>,
    /// Provider-specific model identifier (e.g. `claude-opus-4-7`,
    /// `gpt-4o`, `anthropic/claude-3.5-sonnet`, `llama3.2`).
    pub model: String,
    /// Hard cap on output tokens. None lets the provider use its default.
    pub max_tokens: Option<u32>,
}

/// Reason the model stopped generating, normalized across providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Normal completion of the response.
    EndTurn,
    /// Model emitted one or more tool-use blocks; the agent loop should
    /// dispatch them and call `ask` again with the results.
    ToolUse,
    /// Hit the configured `max_tokens` ceiling. Response is truncated.
    MaxTokens,
    /// Hit a configured stop sequence (rare for our usage).
    StopSequence,
    /// Provider-specific stop reason we don't have a normalized variant
    /// for. Carries the raw string for diagnostics.
    Other(String),
}

/// Events streamed from a provider while it's generating a response.
/// The provider sends these to the `mpsc::UnboundedSender` passed to
/// [`Provider::ask`]; the caller drives the matching receiver.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderEvent {
    /// A chunk of assistant text. Concatenate consecutive deltas to
    /// rebuild the full text block.
    TextDelta(String),
    /// A tool-use block has fully arrived. Carries the full input JSON
    /// (not streamed in chunks — providers buffer until complete).
    ToolUse {
        id: String,
        name: String,
        args: JsonValue,
    },
    /// Token-accounting snapshot. Providers emit this once at the end of
    /// the stream; some emit incremental updates too.
    Usage {
        input_tokens: u32,
        output_tokens: u32,
    },
    /// Stream terminated. No further events will arrive.
    Done { stop_reason: StopReason },
}

/// The provider abstraction. Each backend (Anthropic, OpenAI, OpenRouter,
/// Ollama) implements this. The trait is object-safe so `Box<dyn
/// Provider>` works for runtime selection.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Human-readable provider name for logging and status display.
    fn name(&self) -> &str;

    /// Stream a response to the given request. Events arrive on
    /// `events` in arrival order; the channel is closed when the
    /// response finishes (cleanly or with an error). Returns once the
    /// stream is drained.
    ///
    /// Callers typically run this on a Tokio task while pumping the
    /// receiver from the UI thread.
    async fn ask(
        &self,
        request: AskRequest,
        events: tokio::sync::mpsc::UnboundedSender<ProviderEvent>,
    ) -> Result<(), ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_constructors() {
        let m = Message::user("hi");
        assert_eq!(m.role, Role::User);
        assert_eq!(m.content.len(), 1);
        assert!(matches!(&m.content[0], ContentBlock::Text(t) if t == "hi"));

        let a = Message::assistant("yo");
        assert_eq!(a.role, Role::Assistant);
    }

    #[test]
    fn stop_reason_other_carries_raw() {
        let s = StopReason::Other("safety".to_string());
        assert_eq!(format!("{s:?}"), "Other(\"safety\")");
    }

    #[test]
    fn provider_event_text_delta_concatenation() {
        // Sanity: deltas are just strings, no envelope tricks.
        let e1 = ProviderEvent::TextDelta("hello ".to_string());
        let e2 = ProviderEvent::TextDelta("world".to_string());
        let mut buf = String::new();
        for e in [e1, e2] {
            if let ProviderEvent::TextDelta(s) = e {
                buf.push_str(&s);
            }
        }
        assert_eq!(buf, "hello world");
    }
}
