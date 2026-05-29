//! Append-only JSONL session log for agent conversations.
//!
//! Each session is a single file at
//! `$XDG_STATE_HOME/larkline/sessions/<session-id>.jsonl`. Entries are
//! one JSON object per line. The file is the source of truth: crash
//! recovery + resume + audit trail all derive from replaying the log.
//!
//! Entries form a tree via `id` + `parent_id`. v1.0 only ever appends
//! to one leaf (linear conversations); v1.1 will add tree traversal
//! for `/fork` + `/tree`. The `parent_id` field is stored from day one
//! so v1.1 is purely additive.
//!
//! Session IDs are UUID v7 — time-ordered so `ls sessions/` lists
//! newest-first without a separate sort step.
//!
//! Decision: ADR-008, harness-deck `20260523-pi-mono-study`. Prior art:
//! `packages/agent/docs/durable-harness.md` in earendil-works/pi.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::provider::{Message, StopReason};

/// Current session-file format. Bumping requires a migration; the reader
/// rejects anything other than this version with [`SessionError::Version`].
pub const SESSION_VERSION: u32 = 1;

/// One line in the session log, tagged by `type`.
///
/// Note: this internally-tagged enum has no `#[serde(other)]` fallback, so
/// a reader currently *errors* on an unknown `type` rather than skipping it.
/// True forward-compatible skipping of future variants is a v1.x concern
/// (it would require parsing to a `Value` and filtering by tag, since every
/// known variant is assumed to carry an `id`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEntry {
    /// First line of every session file. Identifies the format version and
    /// the session itself; no `parent_id` (it's the root).
    Session {
        /// Session ID (must equal the filename stem).
        id: Uuid,
        /// Format version. Compared against [`SESSION_VERSION`] on open.
        version: u32,
        /// Wall-clock creation time, unix epoch milliseconds.
        created_at_ms: u64,
    },
    /// User-authored message.
    User {
        id: Uuid,
        parent_id: Uuid,
        timestamp_ms: u64,
        message: Message,
    },
    /// Assistant message (model output) — text and/or tool calls.
    Assistant {
        id: Uuid,
        parent_id: Uuid,
        timestamp_ms: u64,
        message: Message,
    },
    /// Result of dispatching a tool the model requested. Phase 8.B will
    /// start emitting these; 8.A stores the variant so the format is
    /// stable from day one.
    ToolResult {
        id: Uuid,
        parent_id: Uuid,
        timestamp_ms: u64,
        call_id: String,
        content: String,
        is_error: bool,
    },
    /// End-of-turn marker. Records the stop reason + usage so replay can
    /// reconstruct conversation state without re-parsing every message.
    TurnEnd {
        id: Uuid,
        parent_id: Uuid,
        timestamp_ms: u64,
        stop_reason: StopReason,
        input_tokens: u32,
        output_tokens: u32,
    },
    /// Records the current leaf after every save point so reopening the
    /// file restores the cursor without scanning every branch. v1.0
    /// always points to the most recent committed entry.
    Leaf {
        id: Uuid,
        parent_id: Uuid,
        timestamp_ms: u64,
        entry_id: Uuid,
    },
}

impl SessionEntry {
    /// Returns the entry's own ID (every variant has one).
    #[must_use]
    pub fn id(&self) -> Uuid {
        match self {
            Self::Session { id, .. }
            | Self::User { id, .. }
            | Self::Assistant { id, .. }
            | Self::ToolResult { id, .. }
            | Self::TurnEnd { id, .. }
            | Self::Leaf { id, .. } => *id,
        }
    }
}

/// Errors from session log I/O. Distinct from `AgentError` because the
/// harness wraps these into `AgentError::Session(_)` — keeping the
/// granular set here lets tests assert on specific failure modes.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// File-system I/O failure (open, write, read).
    #[error("session I/O: {0}")]
    Io(#[from] std::io::Error),
    /// JSON serialization or parse failure.
    #[error("session JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// Session file has a version we don't know how to read.
    #[error("session file version {found} not supported (expected {expected})")]
    Version { expected: u32, found: u32 },
    /// Session file is missing the required header entry on the first line.
    #[error("session file missing header")]
    MissingHeader,
}

/// Append-only writer for one session file.
///
/// One instance per active session. Holds an open `File` handle; writes
/// are flushed after every entry so a crash never loses a committed
/// message. Not thread-safe — the agent harness owns the only instance
/// and serializes access via its phase state machine.
#[derive(Debug)]
pub struct SessionLog {
    session_id: Uuid,
    path: PathBuf,
    file: std::fs::File,
    /// Last committed entry's ID; the next append uses this as `parent_id`.
    last_id: Uuid,
}

impl SessionLog {
    /// Create a new session file at `dir/<uuid>.jsonl` and write the
    /// header entry. The session ID is UUID v7 (time-ordered).
    pub fn create_in(dir: &Path) -> Result<Self, SessionError> {
        std::fs::create_dir_all(dir)?;
        let session_id = Uuid::now_v7();
        let path = dir.join(format!("{session_id}.jsonl"));
        let file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        let mut log = Self {
            session_id,
            path,
            file,
            last_id: session_id,
        };
        let header = SessionEntry::Session {
            id: session_id,
            version: SESSION_VERSION,
            created_at_ms: now_ms(),
        };
        log.write_line(&header)?;
        Ok(log)
    }

    /// Reopen an existing session file for appending. Validates the
    /// header, replays the log to find the current leaf, and positions
    /// the writer at the end of the file.
    ///
    /// Returns the full entry sequence so the caller can reconstruct
    /// conversation state without re-reading the file.
    pub fn reopen(path: &Path) -> Result<(Self, Vec<SessionEntry>), SessionError> {
        let raw = std::fs::read_to_string(path)?;
        let mut entries: Vec<SessionEntry> = Vec::new();
        // Collect non-empty lines so we can distinguish a torn FINAL line
        // (a crash / power-loss mid-append) from genuine mid-file
        // corruption. The log is the source of truth for crash recovery, so
        // tail damage must drop only the bad last line, not the whole
        // recoverable prefix; a parse failure anywhere earlier is real
        // corruption and is surfaced rather than silently swallowed.
        let lines: Vec<&str> = raw
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let last_idx = lines.len().saturating_sub(1);
        for (i, line) in lines.iter().enumerate() {
            match serde_json::from_str::<SessionEntry>(line) {
                Ok(entry) => entries.push(entry),
                Err(e) if i == last_idx => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "dropping malformed trailing line during session reopen (likely a torn write)"
                    );
                    break;
                }
                Err(e) => return Err(e.into()),
            }
        }

        let header = entries.first().ok_or(SessionError::MissingHeader)?;
        let SessionEntry::Session {
            id: session_id,
            version,
            ..
        } = header
        else {
            return Err(SessionError::MissingHeader);
        };
        if *version != SESSION_VERSION {
            return Err(SessionError::Version {
                expected: SESSION_VERSION,
                found: *version,
            });
        }

        // Last non-Leaf entry is the resume point. Leaf entries are
        // breadcrumbs the harness writes after each save point; the next
        // real entry replaces them. v1.0 keeps it simple — we read the
        // last entry of any kind for parent_id continuity.
        let last_id = entries.last().map_or(*session_id, SessionEntry::id);

        let file = std::fs::OpenOptions::new().append(true).open(path)?;

        Ok((
            Self {
                session_id: *session_id,
                path: path.to_path_buf(),
                file,
                last_id,
            },
            entries,
        ))
    }

    /// Session ID (matches the filename stem).
    #[must_use]
    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    /// Absolute path to the session file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append a user message; returns the new entry's ID.
    pub fn append_user(&mut self, message: Message) -> Result<Uuid, SessionError> {
        let id = Uuid::now_v7();
        let entry = SessionEntry::User {
            id,
            parent_id: self.last_id,
            timestamp_ms: now_ms(),
            message,
        };
        self.append(&entry)?;
        Ok(id)
    }

    /// Append an assistant message; returns the new entry's ID.
    pub fn append_assistant(&mut self, message: Message) -> Result<Uuid, SessionError> {
        let id = Uuid::now_v7();
        let entry = SessionEntry::Assistant {
            id,
            parent_id: self.last_id,
            timestamp_ms: now_ms(),
            message,
        };
        self.append(&entry)?;
        Ok(id)
    }

    /// Append a turn-end marker; returns the new entry's ID.
    pub fn append_turn_end(
        &mut self,
        stop_reason: StopReason,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<Uuid, SessionError> {
        let id = Uuid::now_v7();
        let entry = SessionEntry::TurnEnd {
            id,
            parent_id: self.last_id,
            timestamp_ms: now_ms(),
            stop_reason,
            input_tokens,
            output_tokens,
        };
        self.append(&entry)?;
        Ok(id)
    }

    /// Append a tool-result entry, chaining `parent_id` from the
    /// current leaf (like the other typed helpers). `call_id` matches
    /// the `ToolUse.id` from the assistant message that requested it.
    pub fn append_tool_result(
        &mut self,
        call_id: String,
        content: String,
        is_error: bool,
    ) -> Result<Uuid, SessionError> {
        let id = Uuid::now_v7();
        let entry = SessionEntry::ToolResult {
            id,
            parent_id: self.last_id,
            timestamp_ms: now_ms(),
            call_id,
            content,
            is_error,
        };
        self.append(&entry)?;
        Ok(id)
    }

    /// Append a generic entry; updates the cursor + writes to disk.
    /// Public for tests + custom entry types.
    pub fn append(&mut self, entry: &SessionEntry) -> Result<(), SessionError> {
        self.write_line(entry)?;
        self.last_id = entry.id();
        Ok(())
    }

    fn write_line(&mut self, entry: &SessionEntry) -> Result<(), SessionError> {
        let json = serde_json::to_string(entry)?;
        self.file.write_all(json.as_bytes())?;
        self.file.write_all(b"\n")?;
        // `write_all` already hands the bytes to the OS, so a committed
        // entry survives a process crash / kill (it's in the page cache).
        // `File::flush` is a no-op (there's no userspace buffer); this is
        // page-cache durability, NOT fsync — a power loss or kernel panic
        // can still lose an un-synced tail (which `reopen` tolerates as a
        // torn trailing line). Call `sync_data` here if power-loss
        // durability is ever required.
        self.file.flush()?;
        Ok(())
    }
}

/// Wall-clock now in unix epoch milliseconds. Matches the rest of the
/// codebase's timestamp idiom (see `src/update.rs`, `src/history.rs`).
fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::provider::{ContentBlock, Role};
    use tempfile::tempdir;

    #[test]
    fn create_writes_header_and_returns_uuid_v7() {
        let dir = tempdir().unwrap();
        let log = SessionLog::create_in(dir.path()).unwrap();
        // UUID v7 has version bits 0111 in the 7th nibble.
        assert_eq!(log.session_id().get_version_num(), 7);
        let body = std::fs::read_to_string(log.path()).unwrap();
        assert!(body.contains("\"type\":\"session\""));
        assert!(body.contains(&format!("\"version\":{SESSION_VERSION}")));
    }

    #[test]
    fn append_user_then_assistant_round_trips() {
        let dir = tempdir().unwrap();
        let mut log = SessionLog::create_in(dir.path()).unwrap();
        let path = log.path().to_path_buf();
        let user_id = log.append_user(Message::user("hi")).unwrap();
        let asst_id = log.append_assistant(Message::assistant("hello")).unwrap();
        assert_ne!(user_id, asst_id);
        drop(log);

        let (_log, entries) = SessionLog::reopen(&path).unwrap();
        assert_eq!(entries.len(), 3); // header + user + assistant

        match &entries[1] {
            SessionEntry::User { message, .. } => {
                assert!(matches!(&message.content[0], ContentBlock::Text(t) if t == "hi"));
                assert_eq!(message.role, Role::User);
            }
            other => panic!("expected User, got {other:?}"),
        }
    }

    #[test]
    fn parent_id_chains_through_appends() {
        let dir = tempdir().unwrap();
        let mut log = SessionLog::create_in(dir.path()).unwrap();
        let path = log.path().to_path_buf();
        let user_id = log.append_user(Message::user("a")).unwrap();
        let asst_id = log.append_assistant(Message::assistant("b")).unwrap();
        drop(log);

        let (_log, entries) = SessionLog::reopen(&path).unwrap();
        let SessionEntry::Session { id: session_id, .. } = entries[0] else {
            panic!("missing header");
        };
        let SessionEntry::User { parent_id: p1, .. } = entries[1] else {
            panic!()
        };
        let SessionEntry::Assistant { parent_id: p2, .. } = entries[2] else {
            panic!()
        };
        assert_eq!(p1, session_id, "user's parent is session header");
        assert_eq!(p2, user_id, "assistant's parent is user message");
        // Sanity: not stale references.
        let _ = asst_id;
    }

    #[test]
    fn reopen_rejects_wrong_version() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bogus.jsonl");
        std::fs::write(
            &path,
            r#"{"type":"session","id":"00000000-0000-7000-8000-000000000000","version":9999,"created_at_ms":0}
"#,
        )
        .unwrap();
        let err = SessionLog::reopen(&path).unwrap_err();
        assert!(matches!(
            err,
            SessionError::Version {
                expected: SESSION_VERSION,
                found: 9999
            }
        ));
    }

    #[test]
    fn reopen_errors_on_missing_header() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.jsonl");
        std::fs::write(&path, "").unwrap();
        let err = SessionLog::reopen(&path).unwrap_err();
        assert!(matches!(err, SessionError::MissingHeader));
    }

    #[test]
    fn reopen_tolerates_torn_trailing_line() {
        // Simulate a crash mid-append: valid entries then a truncated last
        // line. The recoverable prefix must survive, not be discarded.
        let dir = tempdir().unwrap();
        let mut log = SessionLog::create_in(dir.path()).unwrap();
        let path = log.path().to_path_buf();
        log.append_user(Message::user("hello")).unwrap();
        log.append_assistant(Message::assistant("hi")).unwrap();
        drop(log);

        // Append a torn (partial-JSON) final line.
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        f.write_all(b"{\"type\":\"user\",\"id\":\"00000000-0000-7000-8000-0000")
            .unwrap();
        drop(f);

        let (_log, entries) = SessionLog::reopen(&path).unwrap();
        // session header + user + assistant survive; the torn line is dropped.
        assert_eq!(entries.len(), 3);
        assert!(matches!(entries[0], SessionEntry::Session { .. }));
        assert!(matches!(entries[2], SessionEntry::Assistant { .. }));
    }

    #[test]
    fn reopen_surfaces_midfile_corruption() {
        // A bad line that is NOT the last is genuine corruption — must error,
        // not be silently swallowed.
        let dir = tempdir().unwrap();
        let path = dir.path().join("corrupt.jsonl");
        std::fs::write(
            &path,
            "{\"type\":\"session\",\"id\":\"00000000-0000-7000-8000-000000000000\",\"version\":1,\"created_at_ms\":0}\nNOT JSON\n{\"type\":\"session\",\"id\":\"00000000-0000-7000-8000-000000000000\",\"version\":1,\"created_at_ms\":0}\n",
        )
        .unwrap();
        assert!(SessionLog::reopen(&path).is_err());
    }

    #[test]
    fn turn_end_carries_stop_reason_and_usage() {
        let dir = tempdir().unwrap();
        let mut log = SessionLog::create_in(dir.path()).unwrap();
        let path = log.path().to_path_buf();
        log.append_user(Message::user("q")).unwrap();
        log.append_assistant(Message::assistant("a")).unwrap();
        log.append_turn_end(StopReason::EndTurn, 12, 34).unwrap();
        drop(log);

        let (_log, entries) = SessionLog::reopen(&path).unwrap();
        let last = entries.last().unwrap();
        match last {
            SessionEntry::TurnEnd {
                stop_reason,
                input_tokens,
                output_tokens,
                ..
            } => {
                assert_eq!(*stop_reason, StopReason::EndTurn);
                assert_eq!(*input_tokens, 12);
                assert_eq!(*output_tokens, 34);
            }
            other => panic!("expected TurnEnd, got {other:?}"),
        }
    }
}
