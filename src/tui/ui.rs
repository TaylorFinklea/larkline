//! Layout and widget rendering.
//!
//! This module is a pure function of [`AppState`] — it takes state in, draws to a `Frame`,
//! and returns. No mutations, no side effects.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table,
        TableState,
    },
};

use ansi_to_tui::IntoText;

use crate::app::{AppState, Mode, OutputMode, UnifiedRow, VimMode};
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

    let show_right_pane = state.mode == Mode::ViewOutput
        || (state.mode == Mode::Unified
            && state.preview_plugin_index.is_some()
            && chunks[1].width >= 80);

    if show_right_pane {
        // Horizontal split: unified list (left) | right pane (right)
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(chunks[1]);

        render_unified_list(frame, state, theme, content_chunks[0]);
        if state.mode == Mode::ViewOutput {
            render_output_pane(frame, state, theme, content_chunks[1]);
        } else {
            render_preview_pane(frame, state, theme, content_chunks[1]);
        }
    } else {
        render_unified_list(frame, state, theme, chunks[1]);
    }
}

fn render_search_bar(
    frame: &mut Frame,
    state: &AppState,
    theme: &Theme,
    area: ratatui::layout::Rect,
) {
    let is_searching = !state.query.is_empty() || state.vim_mode == VimMode::Insert;

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

#[allow(clippy::too_many_lines)]
fn render_unified_list(
    frame: &mut Frame,
    state: &AppState,
    theme: &Theme,
    area: ratatui::layout::Rect,
) {
    let items: Vec<ListItem> = state
        .unified_rows
        .iter()
        .map(|row| match row {
            UnifiedRow::GroupHeader { name, icon } => {
                // Non-selectable group separator: ─── icon Name ───
                let sep = "─".repeat(2);
                let line = Line::from(vec![
                    Span::styled(format!(" {sep} "), Style::default().fg(theme.text_dimmed)),
                    if state.show_icons {
                        Span::styled(format!("{icon} "), Style::default().bold())
                    } else {
                        Span::raw("")
                    },
                    Span::styled(name.as_str(), Style::default().fg(theme.text).bold()),
                    Span::styled(format!(" {sep}"), Style::default().fg(theme.text_dimmed)),
                ]);
                ListItem::new(line)
            }
            UnifiedRow::Command {
                name,
                description,
                icon,
                quickkey,
                group_name,
                match_positions,
                ..
            } => {
                let mut spans = Vec::new();
                if state.show_icons {
                    spans.push(Span::styled(format!("{icon} "), Style::default().bold()));
                }
                // Name with character-level match highlighting.
                if match_positions.is_empty() {
                    spans.push(Span::styled(
                        name.as_str(),
                        Style::default().fg(theme.text).bold(),
                    ));
                } else {
                    for (char_idx, ch) in name.chars().enumerate() {
                        let style = if match_positions.contains(&char_idx) {
                            Style::default().fg(theme.accent).bold()
                        } else {
                            Style::default().fg(theme.text).bold()
                        };
                        spans.push(Span::styled(ch.to_string(), style));
                    }
                }
                // Description.
                if !description.is_empty() {
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(
                        description.as_str(),
                        Style::default().fg(theme.text_dimmed),
                    ));
                }
                // Group badge shown during search.
                if let Some(group) = group_name {
                    spans.push(Span::styled(
                        format!("  — {group}"),
                        Style::default().fg(theme.text_dimmed),
                    ));
                }
                // Quickkey badge on the right: [gb]
                if let Some(qk) = quickkey {
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(
                        format!("[{qk}]"),
                        Style::default().fg(theme.accent),
                    ));
                }
                ListItem::new(Line::from(spans))
            }
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.text_dimmed));

    let highlight_style = Style::default()
        .bg(theme.highlight_bg)
        .fg(theme.highlight_fg)
        .add_modifier(Modifier::BOLD);

    let list = List::new(items)
        .block(block)
        .highlight_style(highlight_style)
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    if !state.unified_rows.is_empty() && state.unified_rows.iter().any(UnifiedRow::is_selectable) {
        list_state.select(Some(state.unified_selected));
    }

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_preview_pane(
    frame: &mut Frame,
    state: &AppState,
    theme: &Theme,
    area: ratatui::layout::Rect,
) {
    use crate::app::CachedResult;

    let Some(idx) = state.preview_plugin_index else {
        // No command selected — render an empty bordered block.
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.text_dimmed));
        frame.render_widget(block, area);
        return;
    };

    let meta = &state.plugins[idx];
    let icon_str = if state.show_icons {
        format!("{} ", meta.icon)
    } else {
        String::new()
    };

    let mut lines: Vec<Line> = Vec::new();

    // Header: icon + name.
    lines.push(Line::from(vec![
        Span::styled(&icon_str, Style::default().bold()),
        Span::styled(meta.name.as_str(), Style::default().fg(theme.text).bold()),
    ]));

    // Description.
    if !meta.description.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            meta.description.as_str(),
            Style::default().fg(theme.text_dimmed),
        )));
    }

    lines.push(Line::raw(""));

    // Cache status + item preview.
    match state.result_cache.get(&idx) {
        Some(CachedResult::Ready(output) | CachedResult::Revalidating(output)) => {
            let n = output.items.len();
            lines.push(Line::from(Span::styled(
                format!("{n} item{}", if n == 1 { "" } else { "s" }),
                Style::default().fg(theme.text_dimmed),
            )));
            lines.push(Line::raw(""));
            for item in output.items.iter().take(5) {
                let bullet_icon = item.icon.as_deref().unwrap_or("·");
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {bullet_icon} "),
                        Style::default().fg(theme.text_dimmed),
                    ),
                    Span::styled(item.label.as_str(), Style::default().fg(theme.text)),
                ]));
            }
            if n > 5 {
                lines.push(Line::from(Span::styled(
                    format!("  … and {} more", n - 5),
                    Style::default().fg(theme.text_dimmed),
                )));
            }
        }
        Some(CachedResult::Loading(_)) => {
            lines.push(Line::from(Span::styled(
                "Loading…",
                Style::default().fg(theme.text_dimmed),
            )));
        }
        Some(CachedResult::Error(e)) => {
            lines.push(Line::from(Span::styled(
                format!("Error: {e}"),
                Style::default().fg(theme.accent),
            )));
        }
        None => {
            lines.push(Line::from(Span::styled(
                "Press Enter to run",
                Style::default().fg(theme.text_dimmed),
            )));
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.text_dimmed));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

#[allow(clippy::too_many_lines)]
fn render_output_pane(
    frame: &mut Frame,
    state: &AppState,
    theme: &Theme,
    area: ratatui::layout::Rect,
) {
    // Determine output title from the selected plugin.
    let title_text = if let Some(ref output) = state.plugin_output {
        output.title.clone()
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

    // Confirmation dialog
    if let Some(ref pending) = state.pending_confirmation {
        let prompt = format!(
            " {}\n Run: {} {}\n\n [Y]es  [N]o ",
            pending.description,
            pending.command,
            pending.args.join(" ")
        );
        let paragraph = Paragraph::new(prompt)
            .block(block)
            .style(Style::default().fg(theme.accent));
        frame.render_widget(paragraph, area);
        return;
    }

    // Loading state
    if state.is_loading {
        let spinner = SPINNER_CHARS[state.spinner_tick as usize % 8];
        let elapsed = state
            .loading_started
            .map_or(0.0, |t| t.elapsed().as_secs_f32());
        let loading_text = format!("{spinner} Running {title_text}… ({elapsed:.1}s)");
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
        match state.output_mode {
            OutputMode::List => {
                if !output.items.is_empty() {
                    render_output_items(frame, state, theme, output, block, area);
                    return;
                }
                if let Some(ref raw) = output.raw_text {
                    let text = raw
                        .as_bytes()
                        .into_text()
                        .unwrap_or_else(|_| ratatui::text::Text::raw(raw.as_str()));
                    let paragraph = Paragraph::new(text).block(block);
                    frame.render_widget(paragraph, area);
                    return;
                }
            }
            OutputMode::RawText => {
                if let Some(ref raw) = output.raw_text {
                    let text = raw
                        .as_bytes()
                        .into_text()
                        .unwrap_or_else(|_| ratatui::text::Text::raw(raw.as_str()));
                    let paragraph = Paragraph::new(text).block(block);
                    frame.render_widget(paragraph, area);
                } else {
                    // Format items as plain text lines.
                    let text = output
                        .items
                        .iter()
                        .map(|i| i.label.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    let paragraph = Paragraph::new(text).block(block);
                    frame.render_widget(paragraph, area);
                }
                return;
            }
            OutputMode::Table => {
                if !output.columns.is_empty() {
                    render_output_table(frame, state, theme, output, block, area);
                    return;
                }
            }
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

fn render_output_table(
    frame: &mut Frame,
    state: &AppState,
    theme: &Theme,
    output: &crate::plugin::PluginOutput,
    block: Block,
    area: ratatui::layout::Rect,
) {
    // Build header row.
    let header_cells: Vec<Cell> = output
        .columns
        .iter()
        .map(|col| {
            Cell::from(col.header.clone()).style(Style::default().add_modifier(Modifier::BOLD))
        })
        .collect();
    let header = Row::new(header_cells).bottom_margin(1);

    // Build data rows.
    let rows: Vec<Row> = output
        .items
        .iter()
        .map(|item| {
            let cells: Vec<Cell> = output
                .columns
                .iter()
                .map(|col| {
                    let value = match col.key.as_str() {
                        "label" => item.label.clone(),
                        "detail" => item.detail.clone().unwrap_or_default(),
                        "icon" => item.icon.clone().unwrap_or_default(),
                        "url" => item.url.clone().unwrap_or_default(),
                        key => item.metadata.get(key).cloned().unwrap_or_default(),
                    };
                    Cell::from(value)
                })
                .collect();
            Row::new(cells)
        })
        .collect();

    // Column widths: distribute evenly.
    #[allow(clippy::cast_possible_truncation)]
    let col_count = output.columns.len().max(1) as u16; // Columns < 65535 in practice.
    let width_pct = 100 / col_count;
    let widths: Vec<Constraint> = output
        .columns
        .iter()
        .map(|_| Constraint::Percentage(width_pct))
        .collect();

    let highlight_style = Style::default()
        .bg(theme.highlight_bg)
        .fg(theme.highlight_fg)
        .add_modifier(Modifier::BOLD);

    let table = Table::new(rows, &widths)
        .header(header)
        .block(block)
        .row_highlight_style(highlight_style)
        .highlight_symbol("▶ ");

    let mut table_state = TableState::default();
    table_state.select(Some(state.output_selected));

    frame.render_stateful_widget(table, area, &mut table_state);
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

    // Flash message (expires after 2 seconds).
    if let Some((ref msg, ref started)) = state.status_message {
        if started.elapsed().as_secs_f32() < 2.0 {
            let bar = Paragraph::new(format!(" ✓ {msg} ")).style(
                Style::default()
                    .fg(theme.accent)
                    .bg(theme.status_bar_bg)
                    .add_modifier(Modifier::BOLD),
            );
            frame.render_widget(bar, area);
            return;
        }
    }

    let plugin_name_for_status = || -> String {
        if let Some(ref output) = state.plugin_output {
            output.title.clone()
        } else {
            "output".to_string()
        }
    };

    let hint: String = if state.pending_confirmation.is_some() {
        " Confirm action: [Y]es  [N]o ".to_string()
    } else {
        match state.vim_mode {
            VimMode::Command => {
                format!(" [C]  :{}\u{2588} ", state.command_input)
            }
            VimMode::Insert => " [I]  type to search or use quickkeys  Esc: normal ".to_string(),
            VimMode::Normal => match state.mode {
                Mode::Unified => {
                    " [N]  j/k: nav  Enter: run  i: insert  :: cmd  q: quit ".to_string()
                }
                Mode::ViewOutput => {
                    if state.is_loading {
                        let spinner = SPINNER_CHARS[state.spinner_tick as usize % 8];
                        let elapsed = state
                            .loading_started
                            .map_or(0.0, |t| t.elapsed().as_secs_f32());
                        let name = plugin_name_for_status();
                        format!(" [N]  {spinner} Loading {name}… ({elapsed:.1}s) ")
                    } else if state
                        .plugin_output
                        .as_ref()
                        .is_some_and(|o| !o.items.is_empty())
                    {
                        let name = plugin_name_for_status();
                        let n = state.plugin_output.as_ref().map_or(0, |o| o.items.len());
                        format!(" [N]  {name} — {n} items  j/k: nav  Enter: run action  Esc: back ")
                    } else {
                        let name = plugin_name_for_status();
                        format!(" [N]  {name}  Esc: back ")
                    }
                }
            },
        }
    };

    let bar = Paragraph::new(hint).style(
        Style::default()
            .fg(theme.text_dimmed)
            .bg(theme.status_bar_bg),
    );
    frame.render_widget(bar, area);
}
