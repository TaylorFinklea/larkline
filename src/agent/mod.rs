// Phase 5.A scaffolds the types + trait; Phase 5.C plumbs the first
// concrete provider (Anthropic) so the trait actually gets used.
// Until then, suppress the natural "never used" warnings for the
// public surface that's been defined ahead of consumers.
#![allow(dead_code)]
#![allow(unused_imports)]
// Provider names (OpenAI, OpenRouter, Anthropic, Ollama) appear all over
// the prose. Backticking each instance is noisy; allow the brand names.
#![allow(clippy::doc_markdown)]

//! AI provider integration layer.
//!
//! The `agent` module defines the [`Provider`] trait that abstracts over
//! Anthropic Messages, OpenAI Responses, OpenRouter, and Ollama APIs.
//! Each provider streams its response back as [`ProviderEvent`]s over an
//! `mpsc::UnboundedSender`, mirroring the existing engine event pattern
//! (see [`crate::plugin::engine::EngineEvent::PartialOutput`]).
//!
//! The AI plugins built on top (Phase 6 single-shot, Phase 8 tool-use)
//! create a channel, hand the sender to `Provider::ask`, and pump the
//! receiver into TUI streaming output. The trait is intentionally
//! object-safe so a `Box<dyn Provider>` can be selected at runtime from
//! the user's `[ai]` config.
//!
//! Tool-use is built into the protocol from day one: every provider
//! accepts a `Vec<ToolDefinition>` and yields `ProviderEvent::ToolUse`
//! when the model wants to call one. Phase 7 builds the tool registry
//! from plugin manifests; Phase 8 runs the agent loop.

pub mod anthropic;
pub mod error;
pub mod openai;
pub mod openai_chat;
pub mod provider;

pub use error::ProviderError;
pub use provider::{
    AskRequest, ContentBlock, Message, Provider, ProviderEvent, Role, StopReason, ToolDefinition,
};
