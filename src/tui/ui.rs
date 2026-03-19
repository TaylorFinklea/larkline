//! Layout and widget rendering.
//!
//! This module is a pure function of [`AppState`] — it takes state in, draws to a `Frame`,
//! and returns. No mutations, no side effects.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
};

use crate::app::{AppState, Mode};

const SPINNER_CHARS: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

/// Top-level render function. Draws the full UI for the current `AppState`.
pub fn render(frame: &mut Frame, state: &AppState) {
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

    render_search_bar(frame, state, chunks[0]);
    render_status_bar(frame, state, chunks[2]);

    if state.mode == Mode::ViewOutput {
        // Horizontal split: plugin list (left) | output pane (right)
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(chunks[1]);

        render_plugin_list(frame, state, content_chunks[0]);
        render_output_pane(frame, state, content_chunks[1]);
    } else {
        render_plugin_list(frame, state, chunks[1]);
    }
}

fn render_search_bar(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let is_searching = state.mode == Mode::Search;

    let border_style = if is_searching {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let prompt = if is_searching {
        Span::styled("/ ", Style::default().fg(Color::Cyan).bold())
    } else {
        Span::styled("  ", Style::default())
    };

    let query = Span::raw(&state.query);
    let cursor = if is_searching {
        Span::styled("█", Style::default().fg(Color::Cyan))
    } else {
        Span::raw("")
    };

    let content = Line::from(vec![prompt, query, cursor]);
    let block = Block::default()
        .title(Span::styled(
            " lark ",
            Style::default().fg(Color::Cyan).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let paragraph = Paragraph::new(content).block(block);
    frame.render_widget(paragraph, area);
}

fn render_plugin_list(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let items: Vec<ListItem> = state
        .filtered
        .iter()
        .enumerate()
        .map(|(list_pos, &idx)| {
            let plugin = &state.plugins[idx];
            let icon = format!("{} ", plugin.icon);

            // Match indices from nucleo point into plugin.name characters.
            let matched: std::collections::HashSet<usize> = state
                .match_indices
                .get(list_pos)
                .map(|v| v.iter().copied().collect())
                .unwrap_or_default();

            // Build icon spans (never highlighted).
            let mut spans: Vec<Span> = icon
                .chars()
                .map(|c| Span::styled(c.to_string(), Style::default().bold()))
                .collect();

            // Build name spans with per-character highlighting.
            for (char_idx, c) in plugin.name.chars().enumerate() {
                let style = if matched.contains(&char_idx) {
                    Style::default().fg(Color::Cyan).bold()
                } else {
                    Style::default().bold()
                };
                spans.push(Span::styled(c.to_string(), style));
            }

            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                plugin.description.as_str(),
                Style::default().fg(Color::DarkGray),
            ));

            ListItem::new(Line::from(spans))
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            format!(" {} plugins ", state.filtered.len()),
            Style::default().fg(Color::DarkGray),
        ));

    let highlight_style = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White)
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

fn render_output_pane(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
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
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            format!(" {title_text} "),
            Style::default().fg(Color::Cyan).bold(),
        ));

    // Loading state
    if state.is_loading {
        let spinner = SPINNER_CHARS[state.spinner_tick as usize % 8];
        let loading_text = format!("{spinner} Running {title_text}…");
        let paragraph = Paragraph::new(Line::from(Span::styled(
            loading_text,
            Style::default().fg(Color::Cyan),
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
                Style::default().fg(Color::Red).bold(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                error.as_str(),
                Style::default().fg(Color::Red),
            )),
        ];
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    // Output state
    if let Some(ref output) = state.plugin_output {
        if !output.items.is_empty() {
            render_output_items(frame, state, output, block, area);
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
        Style::default().fg(Color::DarkGray),
    ))
    .block(block);
    frame.render_widget(paragraph, area);
}

fn render_output_items(
    frame: &mut Frame,
    state: &AppState,
    output: &crate::plugin::PluginOutput,
    block: Block,
    area: ratatui::layout::Rect,
) {
    let items: Vec<ListItem> = output
        .items
        .iter()
        .map(|item| {
            let mut spans = Vec::new();

            if let Some(ref icon) = item.icon {
                spans.push(Span::styled(format!("{icon} "), Style::default().bold()));
            }

            spans.push(Span::styled(item.label.as_str(), Style::default().bold()));

            if let Some(ref detail) = item.detail {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    detail.as_str(),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let highlight_style = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let list = List::new(items)
        .block(block)
        .highlight_style(highlight_style)
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    list_state.select(Some(state.output_selected));

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_status_bar(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
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

    let bar = Paragraph::new(hint).style(Style::default().fg(Color::DarkGray).bg(Color::Black));
    frame.render_widget(bar, area);
}
