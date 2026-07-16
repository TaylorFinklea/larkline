//! Content-addressed cache for ANSI-parsed raw text.
//!
//! Raw plugin output may contain ANSI escape sequences; parsing them into a
//! styled [`Text`] with `ansi_to_tui` is O(content) and used to happen inside
//! every frame render at the 16ms tick. This cache parses each distinct raw
//! buffer once and serves the parsed [`Text`] on subsequent frames.
//!
//! Entries are keyed by the raw buffer content itself, so a changed buffer can
//! never serve a stale parse — there are no invalidation call sites to
//! maintain (unlike `markdown_cache`). The theme is deliberately not part of
//! the key: ANSI colors are embedded in the escape codes, so the parse output
//! is theme-independent (the markdown path, which *is* theme-dependent, has
//! its own cache).
//!
//! The app syncs the cache before each draw with the set of buffers the frame
//! will render ([`live_raw_buffers`]); [`AnsiTextCache::sync`] evicts
//! everything else, so the cache never outgrows the number of visible
//! raw-text panes.

use std::collections::HashMap;

use ratatui::text::Text;

use crate::app::{AppState, Mode, OutputMode};

/// Parse a raw buffer's ANSI escape sequences into a styled [`Text`],
/// falling back to unstyled text if parsing fails.
pub fn parse_ansi(raw: &str) -> Text<'static> {
    use ansi_to_tui::IntoText as _;
    raw.as_bytes()
        .into_text()
        .unwrap_or_else(|_| Text::raw(raw.to_string()))
}

/// Parsed [`Text`] per raw buffer, keyed by the buffer content.
#[derive(Debug, Default)]
pub struct AnsiTextCache {
    entries: HashMap<String, Text<'static>>,
}

impl AnsiTextCache {
    /// The parsed text for this exact raw buffer, if cached.
    pub fn get(&self, raw: &str) -> Option<&Text<'static>> {
        self.entries.get(raw)
    }

    /// Ensure every buffer in `live` has a parsed entry; evict the rest.
    /// Buffers already cached are *not* reparsed.
    pub fn sync(&mut self, live: &[&str]) {
        self.sync_with(live, parse_ansi);
    }

    fn sync_with(&mut self, live: &[&str], parse: impl Fn(&str) -> Text<'static>) {
        self.entries.retain(|key, _| live.contains(&key.as_str()));
        for raw in live {
            if !self.entries.contains_key(*raw) {
                self.entries.insert((*raw).to_string(), parse(raw));
            }
        }
    }
}

/// Collect the raw buffers the next frame will render through the ANSI path.
///
/// Mirrors the render precedence in `tui::ui`: `ViewOutput` renders raw text
/// in `RawText` mode (always) and `List` mode (only when there are no items);
/// mini-app panes render raw text when there is no form and no items.
/// Markdown-format content is excluded — that path is owned by
/// `markdown_cache`.
pub fn live_raw_buffers(state: &AppState) -> Vec<&str> {
    let mut live = Vec::new();
    if state.mode == Mode::ViewOutput {
        if let Some(ref output) = state.plugin_output {
            if let Some(ref raw) = output.raw_text {
                let renders_raw = match state.output_mode {
                    OutputMode::RawText => true,
                    OutputMode::List => output.items.is_empty(),
                    OutputMode::Table | OutputMode::Markdown => false,
                };
                if renders_raw {
                    live.push(raw.as_str());
                }
            }
        }
    }
    if let Some(ref mini) = state.mini_app {
        for pane in mini.panes.values() {
            let content = &pane.content;
            if content.form.is_none()
                && content.items.is_empty()
                && content.output_format.as_deref() != Some("markdown")
            {
                if let Some(ref raw) = content.raw_text {
                    live.push(raw.as_str());
                }
            }
        }
    }
    live
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::app::{MiniAppState, PaneState};
    use crate::plugin::traits::{FormSpec, MiniAppLayout, OutputItem, PaneContent, PluginOutput};

    // -- parse_ansi ---------------------------------------------------------

    #[test]
    fn parse_ansi_styles_colored_spans() {
        let text = parse_ansi("\x1b[31mred\x1b[0m");
        let span = &text.lines[0].spans[0];
        assert_eq!(span.content.as_ref(), "red");
        assert_eq!(span.style.fg, Some(ratatui::style::Color::Red));
    }

    #[test]
    fn parse_ansi_preserves_plain_text() {
        let text = parse_ansi("plain line");
        assert_eq!(text.lines[0].spans[0].content.as_ref(), "plain line");
    }

    // -- AnsiTextCache ------------------------------------------------------

    #[test]
    fn get_returns_none_for_unsynced_content() {
        assert!(AnsiTextCache::default().get("anything").is_none());
    }

    #[test]
    fn sync_caches_parsed_text_for_live_buffers() {
        let raw = "\x1b[32mok\x1b[0m";
        let mut cache = AnsiTextCache::default();
        cache.sync(&[raw]);
        let text = cache.get(raw).expect("live buffer must be cached");
        assert_eq!(text.lines[0].spans[0].content.as_ref(), "ok");
    }

    #[test]
    fn sync_parses_each_buffer_once_across_frames() {
        let raw = "\x1b[31mred\x1b[0m";
        let mut cache = AnsiTextCache::default();
        let calls = Cell::new(0u32);
        let counting_parse = |r: &str| {
            calls.set(calls.get() + 1);
            parse_ansi(r)
        };
        cache.sync_with(&[raw], counting_parse);
        cache.sync_with(&[raw], counting_parse);
        assert_eq!(
            calls.get(),
            1,
            "unchanged content must be parsed exactly once, not per frame"
        );
    }

    #[test]
    fn sync_evicts_buffers_no_longer_live() {
        let mut cache = AnsiTextCache::default();
        cache.sync(&["old output"]);
        cache.sync(&["new output"]);
        assert!(
            cache.get("old output").is_none(),
            "stale buffer must be evicted"
        );
        assert!(cache.get("new output").is_some());
    }

    // -- live_raw_buffers ---------------------------------------------------

    fn view_output_state(output_mode: OutputMode, items: Vec<OutputItem>) -> AppState {
        AppState {
            mode: Mode::ViewOutput,
            output_mode,
            plugin_output: Some(PluginOutput {
                raw_text: Some("raw ansi".to_string()),
                items,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn pane(content: PaneContent) -> PaneState {
        PaneState {
            content,
            ..Default::default()
        }
    }

    fn mini_app_state(panes: Vec<(&str, PaneState)>) -> MiniAppState {
        MiniAppState {
            plugin_index: 0,
            layout: MiniAppLayout::Pane {
                id: "root".to_string(),
                content: PaneContent::default(),
            },
            panes: panes
                .into_iter()
                .map(|(id, p)| (id.to_string(), p))
                .collect(),
            focused_pane: "root".to_string(),
            pane_order: vec![],
        }
    }

    #[test]
    fn raw_text_mode_output_is_live() {
        let state = view_output_state(OutputMode::RawText, vec![]);
        assert_eq!(live_raw_buffers(&state), vec!["raw ansi"]);
    }

    #[test]
    fn list_mode_output_is_live_only_without_items() {
        let empty = view_output_state(OutputMode::List, vec![]);
        assert_eq!(live_raw_buffers(&empty), vec!["raw ansi"]);

        let with_items = view_output_state(
            OutputMode::List,
            vec![OutputItem {
                label: "item".to_string(),
                ..Default::default()
            }],
        );
        assert!(live_raw_buffers(&with_items).is_empty());
    }

    #[test]
    fn markdown_and_table_modes_are_not_live() {
        for mode in [OutputMode::Markdown, OutputMode::Table] {
            let state = view_output_state(mode, vec![]);
            assert!(live_raw_buffers(&state).is_empty());
        }
    }

    #[test]
    fn non_view_output_mode_is_not_live() {
        let mut state = view_output_state(OutputMode::RawText, vec![]);
        state.mode = Mode::Unified;
        assert!(live_raw_buffers(&state).is_empty());
    }

    #[test]
    fn mini_app_raw_panes_are_live_but_markdown_items_and_form_panes_are_not() {
        let raw_pane = pane(PaneContent {
            raw_text: Some("pane ansi".to_string()),
            ..Default::default()
        });
        let markdown_pane = pane(PaneContent {
            raw_text: Some("# doc".to_string()),
            output_format: Some("markdown".to_string()),
            ..Default::default()
        });
        let items_pane = pane(PaneContent {
            raw_text: Some("shadowed by items".to_string()),
            items: vec![OutputItem {
                label: "item".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        });
        let form_pane = pane(PaneContent {
            raw_text: Some("shadowed by form".to_string()),
            form: Some(FormSpec {
                fields: vec![],
                submit_label: None,
            }),
            ..Default::default()
        });

        let state = AppState {
            mode: Mode::MiniApp,
            mini_app: Some(mini_app_state(vec![
                ("raw", raw_pane),
                ("md", markdown_pane),
                ("items", items_pane),
                ("form", form_pane),
            ])),
            ..Default::default()
        };

        assert_eq!(live_raw_buffers(&state), vec!["pane ansi"]);
    }
}
