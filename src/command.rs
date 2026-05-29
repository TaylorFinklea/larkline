//! Registry of `:` ex-commands — the single source of truth for command
//! names, aliases, descriptions, and argument hints.
//!
//! Used by:
//! - command-mode autocomplete (Tab) — [`complete`]
//! - the live suggestions line in the status bar — [`matching`]
//!
//! Dispatch behavior lives in `app.rs`'s `CommandSubmit` handler; this
//! module owns only the metadata. The `registry_covers_dispatch` test in
//! `app.rs` keeps the two in sync.

/// Metadata for one `:` command.
#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    /// Canonical command name (what completion expands to).
    pub name: &'static str,
    /// Short aliases (e.g. `q` for `quit`). Matched but not suggested as
    /// the primary completion.
    pub aliases: &'static [&'static str],
    /// One-line description shown in the suggestions line.
    pub description: &'static str,
    /// Argument hint shown after the name (e.g.
    /// `phone|narrow|medium|wide|auto`). `None` for no-argument commands.
    pub arg_hint: Option<&'static str>,
}

impl CommandSpec {
    /// `name + arg_hint` rendered for the suggestions line, e.g.
    /// `layout <phone|narrow|medium|wide|auto>`.
    #[must_use]
    pub fn usage(&self) -> String {
        match self.arg_hint {
            Some(hint) => format!("{} <{hint}>", self.name),
            None => self.name.to_string(),
        }
    }

    /// True when `name` or any alias starts with `prefix`
    /// (case-insensitive).
    fn matches_prefix(&self, prefix: &str) -> bool {
        let p = prefix.to_ascii_lowercase();
        self.name.starts_with(&p) || self.aliases.iter().any(|a| a.starts_with(&p))
    }
}

/// All known `:` commands. Order is the suggestion display order.
pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "quit",
        aliases: &["q"],
        description: "Quit larkline",
        arg_hint: None,
    },
    CommandSpec {
        name: "refresh",
        aliases: &["r"],
        description: "Re-run all plugins",
        arg_hint: None,
    },
    CommandSpec {
        name: "layout",
        aliases: &[],
        description: "Set the layout profile (or auto)",
        arg_hint: Some("phone|narrow|medium|wide|auto"),
    },
];

/// Commands whose name or an alias begins with `prefix`. Empty prefix
/// returns all commands. Used to populate the live suggestions line.
///
/// Only matches the verb (first token) — once the user types a space,
/// they're entering arguments, so suggestions collapse to the single
/// matched command (callers can show its `arg_hint`).
#[must_use]
pub fn matching(input: &str) -> Vec<&'static CommandSpec> {
    // If the input already has a space, the verb is complete — match it
    // exactly so the suggestions line can show just that command's args.
    if let Some((verb, _)) = input.split_once(' ') {
        return COMMANDS
            .iter()
            .filter(|c| c.name == verb || c.aliases.contains(&verb))
            .collect();
    }
    COMMANDS
        .iter()
        .filter(|c| c.matches_prefix(input))
        .collect()
}

/// Longest common prefix to complete `input` to, given the matching
/// command names. Returns `None` when there's nothing to add (no match,
/// or already past the verb). On a single match, returns the full name.
///
/// Examples:
/// - `"la"` → `Some("layout")` (single match → full name)
/// - `"r"` → `Some("refresh")` (matches `refresh` + alias `r`; the
///   canonical name wins)
/// - `""` → `None` (don't auto-expand on empty)
/// - `"xyz"` → `None` (no match)
#[must_use]
pub fn complete(input: &str) -> Option<String> {
    // Don't complete once the user is typing arguments.
    if input.contains(' ') || input.is_empty() {
        return None;
    }
    let names: Vec<&str> = matching(input).iter().map(|c| c.name).collect();
    match names.as_slice() {
        [] => None,
        [single] => Some((*single).to_string()),
        many => {
            let lcp = longest_common_prefix(many);
            // Only useful if it extends what the user typed.
            if lcp.len() > input.len() {
                Some(lcp)
            } else {
                None
            }
        }
    }
}

/// Longest common prefix across a set of strings (byte-wise; command
/// names are ASCII).
fn longest_common_prefix(strs: &[&str]) -> String {
    let Some(first) = strs.first() else {
        return String::new();
    };
    let mut end = first.len();
    for s in &strs[1..] {
        end = end.min(s.len());
        while !s.as_bytes()[..end].eq(&first.as_bytes()[..end]) {
            end -= 1;
        }
    }
    first[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_empty_returns_all() {
        assert_eq!(matching("").len(), COMMANDS.len());
    }

    #[test]
    fn matching_prefix_filters_by_name_and_alias() {
        let m = matching("q");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].name, "quit");

        let m = matching("la");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].name, "layout");
    }

    #[test]
    fn matching_after_space_locks_to_verb() {
        let m = matching("layout ph");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].name, "layout");
    }

    #[test]
    fn complete_single_match_expands_to_full_name() {
        assert_eq!(complete("la").as_deref(), Some("layout"));
        assert_eq!(complete("q").as_deref(), Some("quit"));
    }

    #[test]
    fn complete_empty_or_arg_phase_is_none() {
        assert_eq!(complete(""), None);
        assert_eq!(complete("layout phone"), None);
    }

    #[test]
    fn complete_no_match_is_none() {
        assert_eq!(complete("zzz"), None);
    }

    #[test]
    fn usage_includes_arg_hint() {
        let layout = COMMANDS.iter().find(|c| c.name == "layout").unwrap();
        assert_eq!(layout.usage(), "layout <phone|narrow|medium|wide|auto>");
        let quit = COMMANDS.iter().find(|c| c.name == "quit").unwrap();
        assert_eq!(quit.usage(), "quit");
    }

    #[test]
    fn longest_common_prefix_basic() {
        assert_eq!(longest_common_prefix(&["layout", "later"]), "la");
        assert_eq!(longest_common_prefix(&["quit", "quiet"]), "qui");
        assert_eq!(longest_common_prefix(&["a", "b"]), "");
    }
}
