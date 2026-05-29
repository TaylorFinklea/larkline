//! Safe-metadata audit log for agent runtime events.
//!
//! Separate from the session log: a single rolling file at
//! `$XDG_STATE_HOME/larkline/agent-audit.log` (one file across all
//! sessions, ops-review use case). Captures **only safe metadata** —
//! provider, model, tool name, status, tokens, duration, span IDs.
//! Excludes prompts, completions, tool args, tool results, API keys,
//! headers by default. Content capture is opt-in for v1.1.
//!
//! Decision: ADR-008, harness-deck `20260523-pi-mono-study`. Prior art:
//! `packages/agent/docs/observability.md` in earendil-works/pi.
//!
//! Schema example:
//!
//! ```text
//! {"ts_ms":...,"trace_id":"...","span_id":"...","name":"agent.turn","kind":"start"}
//! {"ts_ms":...,"trace_id":"...","span_id":"...","name":"ai.provider.request","kind":"end","metadata":{"provider":"anthropic","model":"...","input_tokens":2341,"output_tokens":189,"duration_ms":3420}}
//! ```

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::agent::provider::StopReason;

/// Kind of a record — start/end pairs form spans; one-shot events fire
/// once with `Event`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditKind {
    /// Start of a span (turn, provider request, tool call). Pairs with
    /// a later `End` carrying the same `span_id`.
    Start,
    /// End of a span. Should include duration_ms in metadata.
    End,
    /// One-shot event (e.g. retry attempt, queue update). No paired
    /// counterpart.
    Event,
}

/// One audit record. Serialized as a single JSONL line. Field names are
/// stable wire contract — external log readers depend on them.
#[derive(Debug, Clone, Serialize)]
pub struct AuditRecord {
    /// Wall-clock timestamp, unix epoch milliseconds.
    pub ts_ms: u64,
    /// Trace ID — one per top-level user prompt; shared across all
    /// child spans (turn, provider request, tool call).
    pub trace_id: Uuid,
    /// Span ID — unique per operation.
    pub span_id: Uuid,
    /// Parent span ID, if any. `None` for top-level spans.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<Uuid>,
    /// Operation name — stable string. Pi convention: `agent.turn`,
    /// `agent.tool_call`, `ai.provider.request`, etc.
    pub name: &'static str,
    /// Span phase or one-shot event marker.
    pub kind: AuditKind,
    /// Safe metadata bag. Excludes prompts, completions, args, results,
    /// keys, headers. Only structural facts: model name, token counts,
    /// stop reasons, status codes, durations, tool names.
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
}

/// Errors from audit log I/O.
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    /// File-system I/O failure (open, write, flush).
    #[error("audit I/O: {0}")]
    Io(#[from] std::io::Error),
    /// JSON serialization failure.
    #[error("audit JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Append-only audit log writer.
///
/// One instance shared across sessions in production. Each write reaches
/// the OS page cache immediately (survives a process crash; see `write`
/// for the fsync caveat). The harness holds an `Option<AuditLog>` so tests
/// can run without an audit file.
#[derive(Debug)]
pub struct AuditLog {
    path: PathBuf,
    file: std::fs::File,
}

impl AuditLog {
    /// Open or create the audit log at `path` for appending. Creates
    /// parent directories as needed.
    pub fn open(path: &Path) -> Result<Self, AuditError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }

    /// Absolute path to the audit file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one record. `write_all` hands the bytes to the OS so a
    /// committed entry survives a process crash; `File::flush` is a no-op,
    /// so this is page-cache durability, not fsync (a power loss can still
    /// lose an un-synced tail). Use `sync_data` if that's ever required.
    pub fn write(&mut self, record: &AuditRecord) -> Result<(), AuditError> {
        let json = serde_json::to_string(record)?;
        self.file.write_all(json.as_bytes())?;
        self.file.write_all(b"\n")?;
        self.file.flush()?;
        Ok(())
    }

    // ---- typed helpers for the common emit sites ---------------------

    /// `agent.turn` start. Returns the span id so the caller can pair
    /// the end event.
    pub fn turn_start(&mut self, trace_id: Uuid) -> Result<Uuid, AuditError> {
        let span_id = Uuid::now_v7();
        self.write(&AuditRecord {
            ts_ms: now_ms(),
            trace_id,
            span_id,
            parent_span_id: None,
            name: "agent.turn",
            kind: AuditKind::Start,
            metadata: serde_json::Value::Null,
        })?;
        Ok(span_id)
    }

    /// `agent.turn` end. Metadata captures stop reason + token usage.
    pub fn turn_end(
        &mut self,
        trace_id: Uuid,
        span_id: Uuid,
        duration_ms: u64,
        stop_reason: &StopReason,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<(), AuditError> {
        self.write(&AuditRecord {
            ts_ms: now_ms(),
            trace_id,
            span_id,
            parent_span_id: None,
            name: "agent.turn",
            kind: AuditKind::End,
            metadata: json!({
                "duration_ms": duration_ms,
                "stop_reason": stop_reason_str(stop_reason),
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
            }),
        })
    }

    /// `ai.provider.request` start. Returns the span id.
    pub fn provider_start(
        &mut self,
        trace_id: Uuid,
        parent_span_id: Uuid,
        provider_name: &'static str,
        model: &str,
    ) -> Result<Uuid, AuditError> {
        let span_id = Uuid::now_v7();
        self.write(&AuditRecord {
            ts_ms: now_ms(),
            trace_id,
            span_id,
            parent_span_id: Some(parent_span_id),
            name: "ai.provider.request",
            kind: AuditKind::Start,
            metadata: json!({
                "provider": provider_name,
                "model": model,
            }),
        })?;
        Ok(span_id)
    }

    /// `ai.provider.request` end. Captures duration + outcome. `error`
    /// is `Some(error_kind_name)` for failures, `None` for success.
    //
    // Eight args is borderline but each is conceptually distinct
    // (3 ids + duration + 2 token counts + error + self). Bundling into
    // a struct would obscure the call sites; the harness has only one
    // call site so the cost stays contained.
    #[allow(clippy::too_many_arguments)]
    pub fn provider_end(
        &mut self,
        trace_id: Uuid,
        parent_span_id: Uuid,
        span_id: Uuid,
        duration_ms: u64,
        input_tokens: u32,
        output_tokens: u32,
        error: Option<&str>,
    ) -> Result<(), AuditError> {
        let metadata = if let Some(err) = error {
            json!({
                "duration_ms": duration_ms,
                "status": "error",
                "error_kind": err,
            })
        } else {
            json!({
                "duration_ms": duration_ms,
                "status": "ok",
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
            })
        };
        self.write(&AuditRecord {
            ts_ms: now_ms(),
            trace_id,
            span_id,
            parent_span_id: Some(parent_span_id),
            name: "ai.provider.request",
            kind: AuditKind::End,
            metadata,
        })
    }
}

/// Wall-clock now in unix epoch milliseconds.
fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Stable string for a stop reason. Avoids the Debug-derived format
/// drifting into the wire contract (consumers parse the audit log).
fn stop_reason_str(sr: &StopReason) -> &'static str {
    match sr {
        StopReason::EndTurn => "end_turn",
        StopReason::ToolUse => "tool_use",
        StopReason::MaxTokens => "max_tokens",
        StopReason::StopSequence => "stop_sequence",
        StopReason::Other(_) => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn read_lines(p: &Path) -> Vec<serde_json::Value> {
        std::fs::read_to_string(p)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn open_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a/b/audit.log");
        let log = AuditLog::open(&nested).unwrap();
        assert!(log.path().exists());
    }

    #[test]
    fn turn_start_end_pair_share_span_id() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("audit.log");
        let mut log = AuditLog::open(&path).unwrap();
        let trace = Uuid::now_v7();
        let span = log.turn_start(trace).unwrap();
        log.turn_end(trace, span, 142, &StopReason::EndTurn, 12, 34)
            .unwrap();

        let entries = read_lines(&path);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["name"], "agent.turn");
        assert_eq!(entries[0]["kind"], "start");
        assert_eq!(entries[0]["span_id"], entries[1]["span_id"]);
        assert_eq!(entries[1]["kind"], "end");
        assert_eq!(entries[1]["metadata"]["duration_ms"], 142);
        assert_eq!(entries[1]["metadata"]["stop_reason"], "end_turn");
        assert_eq!(entries[1]["metadata"]["input_tokens"], 12);
        assert_eq!(entries[1]["metadata"]["output_tokens"], 34);
    }

    #[test]
    fn provider_span_records_parent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("audit.log");
        let mut log = AuditLog::open(&path).unwrap();
        let trace = Uuid::now_v7();
        let turn_span = log.turn_start(trace).unwrap();
        let prov_span = log
            .provider_start(trace, turn_span, "anthropic", "claude-opus-4-7")
            .unwrap();
        log.provider_end(trace, turn_span, prov_span, 3420, 2341, 189, None)
            .unwrap();

        let entries = read_lines(&path);
        // turn.start, provider.start, provider.end
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[1]["name"], "ai.provider.request");
        assert_eq!(entries[1]["parent_span_id"], entries[0]["span_id"]);
        assert_eq!(entries[1]["metadata"]["provider"], "anthropic");
        assert_eq!(entries[1]["metadata"]["model"], "claude-opus-4-7");
        assert_eq!(entries[2]["metadata"]["status"], "ok");
        assert_eq!(entries[2]["metadata"]["duration_ms"], 3420);
    }

    #[test]
    fn provider_end_with_error_omits_token_counts() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("audit.log");
        let mut log = AuditLog::open(&path).unwrap();
        let trace = Uuid::now_v7();
        let span = log.turn_start(trace).unwrap();
        let prov = log
            .provider_start(trace, span, "anthropic", "model")
            .unwrap();
        log.provider_end(trace, span, prov, 50, 0, 0, Some("RateLimited"))
            .unwrap();

        let entries = read_lines(&path);
        let last = &entries[entries.len() - 1];
        assert_eq!(last["metadata"]["status"], "error");
        assert_eq!(last["metadata"]["error_kind"], "RateLimited");
        // Confirm we don't emit zero token counts for errors — those
        // would be misleading.
        assert!(last["metadata"]["input_tokens"].is_null());
    }

    #[test]
    fn unsafe_content_is_not_serialized() {
        // The metadata field is opaque, but our typed helpers must
        // never plumb prompt/completion/args text through. This test
        // pins the schema: if someone adds a `prompt: String` to a
        // helper, the assertion below catches it.
        let dir = tempdir().unwrap();
        let path = dir.path().join("audit.log");
        let mut log = AuditLog::open(&path).unwrap();
        let trace = Uuid::now_v7();
        let span = log.turn_start(trace).unwrap();
        log.turn_end(trace, span, 0, &StopReason::EndTurn, 1, 1)
            .unwrap();
        let prov = log
            .provider_start(trace, span, "anthropic", "secret-model-name")
            .unwrap();
        log.provider_end(trace, span, prov, 0, 1, 1, None).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        // None of the forbidden field names appear in the wire format.
        for forbidden in [
            "\"prompt\":",
            "\"completion\":",
            "\"args\":",
            "\"result\":",
            "\"api_key\":",
            "\"headers\":",
            "\"messages\":",
        ] {
            assert!(
                !raw.contains(forbidden),
                "audit log contained forbidden field {forbidden}: {raw}"
            );
        }
    }

    #[test]
    fn appends_across_open_calls() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("audit.log");
        let trace = Uuid::now_v7();

        let mut log1 = AuditLog::open(&path).unwrap();
        log1.turn_start(trace).unwrap();
        drop(log1);

        let mut log2 = AuditLog::open(&path).unwrap();
        log2.turn_start(trace).unwrap();
        drop(log2);

        let entries = read_lines(&path);
        assert_eq!(entries.len(), 2, "second open should append, not truncate");
    }
}
