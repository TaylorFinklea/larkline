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

use std::collections::HashMap;

/// Build the concrete [`Provider`] selected by the user's `[ai]`
/// configuration, drawing API keys from the resolved secrets map.
///
/// `secrets` is the same map populated by `crate::config::load_secrets`
/// + `crate::config::resolve_keychain_secrets` -- so once the user has
///   run `lark secret set ANTHROPIC_API_KEY` (or env-set it), this
///   factory finds it.
///
/// Returns a boxed trait object so the call site can store it in
/// `AppState` or hand it to the agent loop without knowing which
/// concrete provider was selected.
pub fn build_provider(
    ai: &crate::config::AiConfig,
    secrets: &HashMap<String, String>,
) -> Result<Box<dyn provider::Provider>, error::ProviderError> {
    use crate::config::AiProviderName;
    match ai.provider {
        AiProviderName::Anthropic => {
            let key = lookup_secret(secrets, "ANTHROPIC_API_KEY")?;
            Ok(Box::new(anthropic::AnthropicProvider::new(key)?))
        }
        AiProviderName::Openai => {
            let key = lookup_secret(secrets, "OPENAI_API_KEY")?;
            Ok(Box::new(openai::OpenAiResponsesProvider::new(key)?))
        }
        AiProviderName::Openrouter => {
            let key = lookup_secret(secrets, "OPENROUTER_API_KEY")?;
            Ok(Box::new(openai_chat::OpenAiChatProvider::openrouter(
                key,
                ai.resolved_openrouter_base_url().to_string(),
            )?))
        }
        AiProviderName::Ollama => Ok(Box::new(openai_chat::OpenAiChatProvider::ollama(
            ai.resolved_ollama_base_url().to_string(),
        )?)),
    }
}

fn lookup_secret(
    secrets: &HashMap<String, String>,
    key: &str,
) -> Result<String, error::ProviderError> {
    secrets
        .get(key)
        .cloned()
        .or_else(|| std::env::var(key).ok())
        .ok_or_else(|| {
            error::ProviderError::Auth(format!(
                "{key} is not set (run `lark secret set {key}` or export it)"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AiConfig, AiProviderName};

    #[test]
    fn build_provider_anthropic_picks_anthropic() {
        let cfg = AiConfig {
            provider: AiProviderName::Anthropic,
            ..AiConfig::default()
        };
        let mut secrets = HashMap::new();
        secrets.insert("ANTHROPIC_API_KEY".to_string(), "k".to_string());
        let p = build_provider(&cfg, &secrets).unwrap();
        assert_eq!(p.name(), "anthropic");
    }

    #[test]
    fn build_provider_ollama_works_without_api_key() {
        let cfg = AiConfig {
            provider: AiProviderName::Ollama,
            ..AiConfig::default()
        };
        let secrets = HashMap::new();
        let p = build_provider(&cfg, &secrets).unwrap();
        assert_eq!(p.name(), "ollama");
    }

    #[test]
    fn build_provider_openrouter_surfaces_missing_key_as_auth_error() {
        let cfg = AiConfig {
            provider: AiProviderName::Openrouter,
            ..AiConfig::default()
        };
        let secrets = HashMap::new();
        let err = build_provider(&cfg, &secrets).unwrap_err();
        match err {
            error::ProviderError::Auth(msg) => {
                assert!(msg.contains("OPENROUTER_API_KEY"));
            }
            other => panic!("expected Auth error, got {other:?}"),
        }
    }

    #[test]
    fn build_provider_openai_picks_responses() {
        let cfg = AiConfig {
            provider: AiProviderName::Openai,
            ..AiConfig::default()
        };
        let mut secrets = HashMap::new();
        secrets.insert("OPENAI_API_KEY".to_string(), "k".to_string());
        let p = build_provider(&cfg, &secrets).unwrap();
        assert_eq!(p.name(), "openai");
    }
}

pub use error::ProviderError;
pub use provider::{
    AskRequest, ContentBlock, Message, Provider, ProviderEvent, Role, StopReason, ToolDefinition,
};
