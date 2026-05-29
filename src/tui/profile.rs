//! Width-based layout profile.
//!
//! Larkline's TUI runs across a wide range of terminal widths -- from a
//! 50-column phone tmux session to a 200-column desktop. A single fixed
//! layout looks great in one and unusable in the other. [`LayoutProfile`]
//! captures the discrete width regimes we care about and gates which
//! decorations the rest of the render path shows.
//!
//! The profile is derived per-frame from `frame.area().width`, so resize
//! is automatic (ratatui re-renders on SIGWINCH). The user can lock a
//! profile via `:layout <name>` when the auto-detected one is wrong --
//! for example when a terminal misreports its width over SSH/mosh.
//!
//! The render path treats the profile as a hint, never a hard constraint:
//! state flags like `widgets_visible` and `sidebar_hidden` still win when
//! the user has opted in/out explicitly. The profile only adds gates that
//! prevent the UI from rendering itself into an unreadable jumble.

/// Width-based layout regime. Derived from terminal width or set
/// explicitly via `:layout <profile>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutProfile {
    /// < 60 cols: single column only, no widgets, no preview, compact rows,
    /// minimum status hints. Targets phone tmux sessions.
    Phone,
    /// 60-99 cols: single column with widgets hidden, no preview pane.
    /// Status hints trimmed but not minimized.
    Narrow,
    /// 100-159 cols: list + widgets + preview pane, modest card count.
    Medium,
    /// >= 160 cols: today's full desktop layout.
    Wide,
}

impl LayoutProfile {
    /// Derive a profile from a terminal width in columns.
    #[must_use]
    pub fn from_width(width: u16) -> Self {
        match width {
            0..=59 => Self::Phone,
            60..=99 => Self::Narrow,
            100..=159 => Self::Medium,
            _ => Self::Wide,
        }
    }

    /// Parse a profile name from a `:layout` argument. Returns `None` for
    /// the special `auto` value (caller should clear any override) and
    /// for unknown names.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "phone" => Some(Self::Phone),
            "narrow" => Some(Self::Narrow),
            "medium" => Some(Self::Medium),
            "wide" => Some(Self::Wide),
            _ => None,
        }
    }

    /// Whether the widget dashboard row is allowed to render. The user's
    /// `widgets_visible` flag still controls explicit hide; this only
    /// adds a width-based veto so narrow terminals don't try to pack 6
    /// cards into 80 columns.
    #[must_use]
    pub fn allows_widget_row(self) -> bool {
        matches!(self, Self::Medium | Self::Wide)
    }

    /// Whether the compact glance strip (1-line status chips) may render.
    /// It's only one line, so it shows on Narrow and up; Phone hides it to
    /// keep every row for content.
    #[must_use]
    pub fn allows_glance_strip(self) -> bool {
        matches!(self, Self::Narrow | Self::Medium | Self::Wide)
    }

    /// Whether the secondary right-hand pane (preview in `Unified`, output
    /// detail in `ViewOutput`) can render side-by-side with the list.
    /// Below this threshold the list takes the full width and the right
    /// pane is only reachable via Enter/Esc navigation.
    #[must_use]
    pub fn allows_right_pane(self) -> bool {
        matches!(self, Self::Medium | Self::Wide)
    }

    /// Maximum number of widget cards to render. Widgets beyond this
    /// index are dropped from the row (user's pin order chooses which).
    #[must_use]
    pub fn max_widget_cards(self) -> usize {
        match self {
            Self::Phone | Self::Narrow => 0,
            Self::Medium => 4,
            Self::Wide => 6,
        }
    }

    /// Cap on key hints in the status bar. Hints beyond this count are
    /// dropped to leave room for the mode badge and active context.
    /// `usize::MAX` means "no trimming".
    #[must_use]
    pub fn max_status_hints(self) -> usize {
        match self {
            Self::Phone => 3,
            Self::Narrow => 5,
            _ => usize::MAX,
        }
    }

    /// Human-readable label used in the `:layout` override message and
    /// for any future status-bar indicator.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Narrow => "narrow",
            Self::Medium => "medium",
            Self::Wide => "wide",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_width_hits_each_band() {
        assert_eq!(LayoutProfile::from_width(0), LayoutProfile::Phone);
        assert_eq!(LayoutProfile::from_width(59), LayoutProfile::Phone);
        assert_eq!(LayoutProfile::from_width(60), LayoutProfile::Narrow);
        assert_eq!(LayoutProfile::from_width(99), LayoutProfile::Narrow);
        assert_eq!(LayoutProfile::from_width(100), LayoutProfile::Medium);
        assert_eq!(LayoutProfile::from_width(159), LayoutProfile::Medium);
        assert_eq!(LayoutProfile::from_width(160), LayoutProfile::Wide);
        assert_eq!(LayoutProfile::from_width(400), LayoutProfile::Wide);
    }

    #[test]
    fn parse_round_trips_labels() {
        for p in [
            LayoutProfile::Phone,
            LayoutProfile::Narrow,
            LayoutProfile::Medium,
            LayoutProfile::Wide,
        ] {
            assert_eq!(LayoutProfile::parse(p.label()), Some(p));
        }
        assert_eq!(LayoutProfile::parse("  WIDE  "), Some(LayoutProfile::Wide));
        assert_eq!(LayoutProfile::parse("auto"), None);
        assert_eq!(LayoutProfile::parse("nope"), None);
    }

    #[test]
    fn narrow_and_phone_hide_decorations() {
        assert!(!LayoutProfile::Phone.allows_widget_row());
        assert!(!LayoutProfile::Phone.allows_right_pane());
        assert_eq!(LayoutProfile::Phone.max_widget_cards(), 0);
        assert!(!LayoutProfile::Narrow.allows_widget_row());
        assert!(!LayoutProfile::Narrow.allows_right_pane());
        assert!(LayoutProfile::Medium.allows_widget_row());
        assert!(LayoutProfile::Wide.allows_widget_row());
    }
}
