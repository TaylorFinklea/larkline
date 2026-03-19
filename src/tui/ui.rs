//! Layout and widget rendering.
//!
//! This module is a pure function of [`AppState`] — it takes state in, draws to a `Frame`,
//! and returns. No mutations, no side effects.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
};

use crate::app::{AppState, Mode};
use crate::config::Theme;

const SPINNER_CHARS: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

/// Top-level render function. Draws the full UI for the current `AppState`.
pub fn render(frame: &mut Frame, state: &AppState, theme: &Theme) {
    let area = frame.area();

    // Vertical split: search bar | content area | status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search bar
            Constraint::Min(0),    // Content area (expands to fill)
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    render_search_bar(frame, state, theme, chunks[0]);
    render_status_bar(frame, state, theme, chunks[2]);

    if state.mode == Mode::ViewOutput {
        // Horizontal split: plugin list (left) | output pane (right)
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(chunks[1]);

        render_plugin_list(frame, state, theme, content_chunks[0]);
        render_output_pane(frame, state, theme, content_chunks[1]);
    } else {
        render_plugin_list(frame, state, theme, chunks[1]);
    }
}

fn render_search_bar(
    frame: &mut Frame,
    state: &AppState,
    theme: &Theme,
    area: ratatui::layout::Rect,
) {
    let is_searching = state.mode == Mode::Search;

    let border_style = if is_searching {
        Style::default().fg(theme.accent)
    } else {
        Style::default().fg(theme.text_dimmed)
    };

    let prompt = if is_searching {
        Span::styled("/ ", Style::default().fg(theme.accent).bold())
    } else {
        Span::styled("  ", Style::default())
    };

    let query = Span::raw(&state.query);
    let cursor = if is_searching {
        Span::styled("█", Style::default().fg(theme.accent))
    } else {
        Span::raw("")
    };

    let content = Line::from(vec![prompt, query, cursor]);
    let block = Block::default()
        .title(Span::styled(
            " lark ",
            Style::default().fg(theme.accent).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let paragraph = Paragraph::new(content).block(block);
    frame.render_widget(paragraph, area);
}

fn render_plugin_list(
    frame: &mut Frame,
    state: &AppState,
    theme: &Theme,
    area: ratatui::layout::Rect,
) {
    let items: Vec<ListItem> = state
        .filtered
        .iter()
        .enumerate()
        .map(|(list_pos, &idx)| {
            let plugin = &state.plugins[idx];

            // Match indices from nucleo point into plugin.name characters.
            let matched: std::collections::HashSet<usize> = state
                .match_indices
                .get(list_pos)
                .map(|v| v.iter().copied().collect())
                .unwrap_or_default();

            let mut spans: Vec<Span> = Vec::new();

            // Favorite star indicator.
            if state.favorites.contains(&plugin.name) {
                spans.push(Span::styled("★ ", Style::default().fg(theme.accent).bold()));
            }

            // Icon (conditionally shown).
            if state.show_icons {
                let icon = format!("{} ", plugin.icon);
                for c in icon.chars() {
                    spans.push(Span::styled(c.to_string(), Style::default().bold()));
                }
            }

            // Build name spans with per-character highlighting.
            for (char_idx, c) in plugin.name.chars().enumerate() {
                let style = if matched.contains(&char_idx) {
                    Style::default().fg(theme.accent).bold()
                } else {
                    Style::default().fg(theme.text).bold()
                };
                spans.push(Span::styled(c.to_string(), style));
            }

            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                plugin.description.as_str(),
                Style::default().fg(theme.text_dimmed),
            ));

            ListItem::new(Line::from(spans))
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.text_dimmed))
        .title(Span::styled(
            format!(" {} plugins ", state.filtered.len()),
            Style::default().fg(theme.text_dimmed),
        ));

    let highlight_style = Style::default()
        .bg(theme.highlight_bg)
        .fg(theme.highlight_fg)
        .add_modifier(Modifier::BOLD);

    let list = List::new(items)
        .block(block)
        .highlight_style(highlight_style)
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    if !state.filtered.is_empty() {
        list_state.select(Some(state.selected));
    }

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_output_pane(
    frame: &mut Frame,
    state: &AppState,
    theme: &Theme,
    area: ratatui::layout::Rect,
) {
    // Determine output title from the selected plugin.
    let title_text = if let Some(ref output) = state.plugin_output {
        output.title.clone()
    } else if let Some(&idx) = state.filtered.get(state.selected) {
        state.plugins[idx].name.clone()
    } else {
        "output".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .title(Span::styled(
            format!(" {title_text} "),
            Style::default().fg(theme.accent).bold(),
        ));

    // Loading state
    if state.is_loading {
        let spinner = SPINNER_CHARS[state.spinner_tick as usize % 8];
        let loading_text = format!("{spinner} Running {title_text}…");
        let paragraph = Paragraph::new(Line::from(Span::styled(
            loading_text,
            Style::default().fg(theme.accent),
        )))
        .block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    // Error state
    if let Some(ref error) = state.plugin_error {
        let lines = vec![
            Line::from(Span::styled(
                "✖ Plugin failed",
                Style::default().fg(theme.error).bold(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                error.as_str(),
                Style::default().fg(theme.error),
            )),
        ];
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    // Output state
    if let Some(ref output) = state.plugin_output {
        if !output.items.is_empty() {
            render_output_items(frame, state, theme, output, block, area);
            return;
        }
        if let Some(ref raw) = output.raw_text {
            // Phase 4: ANSI rendering. For now, plain text.
            let paragraph = Paragraph::new(raw.as_str()).block(block);
            frame.render_widget(paragraph, area);
            return;
        }
    }

    // No output yet (ViewOutput entered but waiting or no items)
    let paragraph = Paragraph::new(Span::styled(
        "No output",
        Style::default().fg(theme.text_dimmed),
    ))
    .block(block);
    frame.render_widget(paragraph, area);
}

fn render_output_items(
    frame: &mut Frame,
    state: &AppState,
    theme: &Theme,
    output: &crate::plugin::PluginOutput,
    block: Block,
    area: ratatui::layout::Rect,
) {
    let items: Vec<ListItem> = output
        .items
        .iter()
        .map(|item| {
            let mut spans = Vec::new();

            if state.show_icons {
                if let Some(ref icon) = item.icon {
                    spans.push(Span::styled(format!("{icon} "), Style::default().bold()));
                }
            }

            spans.push(Span::styled(
                item.label.as_str(),
                Style::default().fg(theme.text).bold(),
            ));

            if let Some(ref detail) = item.detail {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    detail.as_str(),
                    Style::default().fg(theme.text_dimmed),
                ));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let highlight_style = Style::default()
        .bg(theme.highlight_bg)
        .fg(theme.highlight_fg)
        .add_modifier(Modifier::BOLD);

    let list = List::new(items)
        .block(block)
        .highlight_style(highlight_style)
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    list_state.select(Some(state.output_selected));

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_status_bar(
    frame: &mut Frame,
    state: &AppState,
    theme: &Theme,
    area: ratatui::layout::Rect,
) {
    // Config warnings take priority over the normal hint.
    if let Some(warning) = state.warnings.first() {
        let bar = Paragraph::new(format!(" ⚠ {warning} ")).style(
            Style::default()
                .fg(theme.accent)
                .bg(theme.status_bar_bg)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_widget(bar, area);
        return;
    }

    let hint = match state.mode {
        Mode::Browse => " j/k: navigate  Enter: select  /: search  q: quit ",
        Mode::Search => " Type to filter  Esc: clear  Enter: select  ↑↓: navigate ",
        Mode::ViewOutput => {
            if state.is_loading {
                " Loading… "
            } else if state
                .plugin_output
                .as_ref()
                .is_some_and(|o| !o.items.is_empty())
            {
                " j/k: navigate  Enter: run action  Esc: back "
            } else {
                " Esc: back "
            }
        }
    };

    let bar = Paragraph::new(hint).style(
        Style::default()
            .fg(theme.text_dimmed)
            .bg(theme.status_bar_bg),
    );
    frame.render_widget(bar, area);
}
