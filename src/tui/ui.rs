//! Layout and widget rendering.
//!
//! This module is a pure function of [`AppState`] — it takes state in, draws to a `Frame`,
//! and returns. No mutations, no side effects.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table,
        TableState,
    },
};

use ansi_to_tui::IntoText;

use crate::app::{
    AppState, Mode, OutputMode, PowerMenuState, ThemePickerState, UnifiedRow, VimMode,
};
use crate::config::Theme;

const SPINNER_CHARS: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

/// Top-level render function. Draws the full UI for the current `AppState`.
pub fn render(frame: &mut Frame, state: &AppState, theme: &Theme) {
    let area = frame.area();

    // Vertical split: search bar | [widgets] | content area | status bar
    let has_widgets = state.widgets_visible && !state.widget_indices.is_empty();
    let widget_height = if has_widgets { 6 } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if has_widgets {
            vec![
                Constraint::Length(3),             // Search bar
                Constraint::Length(widget_height), // Widget dashboard row
                Constraint::Min(0),                // Content area
                Constraint::Length(1),             // Status bar
            ]
        } else {
            vec![
                Constraint::Length(3), // Search bar
                Constraint::Length(0), // No widgets
                Constraint::Min(0),    // Content area
                Constraint::Length(1), // Status bar
            ]
        })
        .split(area);

    render_search_bar(frame, state, theme, chunks[0]);
    render_status_bar(frame, state, theme, chunks[3]);

    // Render widget dashboard row.
    if has_widgets {
        render_widget_row(frame, state, theme, chunks[1]);
    }

    // Content area is now chunks[2] instead of chunks[1].
    let content_area = chunks[2];

    let show_right_pane = state.mode == Mode::ViewOutput
        || (state.mode == Mode::Unified
            && state.preview_plugin_index.is_some()
            && content_area.width >= 80);

    // Plugin Manager takes full content area.
    if state.mode == Mode::PluginManager {
        if let Some(ref pm) = state.plugin_manager {
            render_plugin_manager(frame, pm, theme, content_area);
        }
    // Sidebar hidden: ViewOutput gets full width, Unified hides preview.
    } else if state.sidebar_hidden && state.mode == Mode::ViewOutput {
        render_output_pane(frame, state, theme, content_area);
    } else if state.sidebar_hidden && state.mode == Mode::Unified {
        render_unified_list(frame, state, theme, content_area);
    } else if show_right_pane {
        // Narrow sidebar when drilled in; configurable ratio for browse-with-preview.
        let left_pct = if state.mode == Mode::ViewOutput {
            28
        } else {
            state.sidebar_ratio
        };
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(left_pct),
                Constraint::Percentage(100 - left_pct),
            ])
            .split(content_area);

        render_unified_list(frame, state, theme, content_chunks[0]);
        if state.mode == Mode::ViewOutput {
            render_output_pane(frame, state, theme, content_chunks[1]);
        } else {
            render_preview_pane(frame, state, theme, content_chunks[1]);
        }
    } else {
        render_unified_list(frame, state, theme, content_area);
    }

    // Power menu overlay — rendered last (on top of everything).
    if let Some(ref menu) = state.power_menu {
        render_power_menu(frame, menu, theme, frame.area());
    }

    // Theme picker overlay — rendered on top of power menu if both are open.
    if let Some(ref picker) = state.theme_picker {
        render_theme_picker(frame, picker, theme, frame.area());
    }

    // Widget picker overlay.
    if let Some(ref picker) = state.widget_picker {
        render_widget_picker(frame, picker, theme, frame.area());
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
                // Description (toggled via 'd' key).
                if state.show_descriptions && !description.is_empty() {
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(
                        description.as_str(),
                        Style::default().fg(theme.text_dimmed),
                    ));
                }
                // Group badge shown during search.
                if let Some(group) = group_name {
                    spans.push(Span::styled(
                        format!("  {group}"),
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
    // Build breadcrumb trail from navigation history + current plugin title.
    let breadcrumb = {
        let mut parts: Vec<String> = vec!["lark".to_string()];
        for entry in &state.navigation_history {
            if let Some(meta) = state.plugins.get(entry.plugin_index) {
                parts.push(meta.name.clone());
            }
        }
        let current = if let Some(ref output) = state.plugin_output {
            output.title.clone()
        } else {
            "output".to_string()
        };
        parts.push(current);
        parts.join(" \u{203A} ") // › separator
    };

    let title_text = if state.output_searching || !state.output_query.is_empty() {
        let total = state.plugin_output.as_ref().map_or(0, |o| o.items.len());
        let filtered = state.output_filtered_indices.len();
        format!(
            " {breadcrumb} /{} ({filtered}/{total}) ",
            state.output_query
        )
    } else {
        format!(" {breadcrumb} ")
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .title(Span::styled(
            title_text,
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

    // Copy menu overlay
    if let Some(ref menu) = state.copy_menu {
        let items: Vec<ListItem> = menu
            .entries
            .iter()
            .enumerate()
            .map(|(i, (label, value))| {
                let preview = if value.len() > 40 {
                    format!("{}…", &value[..40])
                } else {
                    value.clone()
                };
                let style = if i == menu.selected {
                    Style::default()
                        .bg(theme.highlight_bg)
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text)
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!(" {label}: "), style.add_modifier(Modifier::BOLD)),
                    Span::styled(preview, style),
                ]))
            })
            .collect();

        let copy_block = block.title(Span::styled(
            " Copy ",
            Style::default().fg(theme.accent).bold(),
        ));
        let list = List::new(items).block(copy_block);
        frame.render_widget(list, area);
        return;
    }

    // Action palette overlay.
    if let Some(ref palette) = state.action_palette {
        use crate::plugin::traits::ActionKind;

        let items: Vec<ListItem> = palette
            .filtered_indices
            .iter()
            .enumerate()
            .filter_map(|(vis_idx, &real_idx)| {
                let action = palette.actions.get(real_idx)?;
                let icon = match action.kind {
                    ActionKind::Open => "↗",
                    ActionKind::Clipboard => "⎘",
                    ActionKind::Shell => "▶",
                };
                let style = if vis_idx == palette.selected {
                    Style::default()
                        .bg(theme.highlight_bg)
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.text)
                };
                Some(ListItem::new(Line::from(vec![
                    Span::styled(format!(" {icon} "), style),
                    Span::styled(action.label.as_str(), style),
                ])))
            })
            .collect();

        let mut title_parts = vec![Span::styled(
            " Actions ",
            Style::default().fg(theme.accent).bold(),
        )];
        if !palette.query.is_empty() {
            title_parts.push(Span::styled(
                format!("/{} ", palette.query),
                Style::default().fg(theme.text_dimmed),
            ));
        }

        let palette_block = block.title(Line::from(title_parts));
        let list = List::new(items).block(palette_block);
        frame.render_widget(list, area);
        return;
    }

    // Form overlay (replaces items area when a form is active).
    if let Some(ref form) = state.form_state {
        render_form(frame, form, theme, block, area);
        return;
    }

    // Loading state
    if state.is_loading {
        let spinner = SPINNER_CHARS[state.spinner_tick as usize % 8];
        let elapsed = state
            .loading_started
            .map_or(0.0, |t| t.elapsed().as_secs_f32());
        let loading_title = state
            .plugin_output
            .as_ref()
            .map_or("plugin", |o| o.title.as_str());
        let loading_text = format!("{spinner} Running {loading_title}… ({elapsed:.1}s)");
        let paragraph = Paragraph::new(Line::from(Span::styled(
            loading_text,
            Style::default().fg(theme.accent),
        )))
        .block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    // Error state — show type, message, and recovery hints.
    if let Some(ref error) = state.plugin_error {
        let (icon, label) = if error.contains("timed out") {
            ("⏱", "Plugin timed out")
        } else if error.contains("invalid") || error.contains("syntax") {
            ("⚠", "Invalid plugin output")
        } else {
            ("✖", "Plugin failed")
        };

        // Word-wrap the error message for the available width.
        let inner_width = area.width.saturating_sub(4) as usize;
        let mut error_lines = Vec::new();
        for raw_line in error.lines() {
            if raw_line.len() <= inner_width {
                error_lines.push(Line::from(Span::styled(
                    raw_line.to_string(),
                    Style::default().fg(theme.error),
                )));
            } else {
                // Simple word wrap.
                let mut current = String::new();
                for word in raw_line.split_whitespace() {
                    if current.len() + word.len() + 1 > inner_width && !current.is_empty() {
                        error_lines.push(Line::from(Span::styled(
                            current.clone(),
                            Style::default().fg(theme.error),
                        )));
                        current.clear();
                    }
                    if !current.is_empty() {
                        current.push(' ');
                    }
                    current.push_str(word);
                }
                if !current.is_empty() {
                    error_lines.push(Line::from(Span::styled(
                        current,
                        Style::default().fg(theme.error),
                    )));
                }
            }
        }

        let mut lines = vec![
            Line::from(Span::styled(
                format!("{icon} {label}"),
                Style::default().fg(theme.error).bold(),
            )),
            Line::from(""),
        ];
        lines.extend(error_lines);
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press r to retry · h or Esc to go back",
            Style::default().fg(theme.text_dimmed),
        )));
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
                #[allow(clippy::cast_possible_truncation)]
                let scroll = state.scroll_offset as u16;
                if let Some(ref raw) = output.raw_text {
                    let text = raw
                        .as_bytes()
                        .into_text()
                        .unwrap_or_else(|_| ratatui::text::Text::raw(raw.as_str()));
                    let paragraph = Paragraph::new(text).block(block).scroll((scroll, 0));
                    frame.render_widget(paragraph, area);
                } else {
                    // Format items as plain text lines.
                    let text = output
                        .items
                        .iter()
                        .map(|i| i.label.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    let paragraph = Paragraph::new(text).block(block).scroll((scroll, 0));
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
            OutputMode::Markdown => {
                // Use cached rendered text if available, fall back to on-the-fly rendering.
                let text = if let Some(ref cached) = state.markdown_cache {
                    cached.clone()
                } else if let Some(ref raw) = output.raw_text {
                    crate::tui::markdown::markdown_to_text(raw, theme)
                } else {
                    ratatui::text::Text::raw("")
                };
                #[allow(clippy::cast_possible_truncation)]
                let scroll = state.scroll_offset as u16;
                let paragraph = Paragraph::new(text).block(block).scroll((scroll, 0));
                frame.render_widget(paragraph, area);
                return;
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
    // Use filtered indices when output search is active, else show all items.
    let visible_indices: Vec<usize> =
        if !state.output_filtered_indices.is_empty() || !state.output_query.is_empty() {
            state.output_filtered_indices.clone()
        } else {
            (0..output.items.len()).collect()
        };

    let items: Vec<ListItem> = visible_indices
        .iter()
        .filter_map(|&i| output.items.get(i))
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

    // Use filtered indices when output search is active.
    let visible_indices: Vec<usize> =
        if !state.output_filtered_indices.is_empty() || !state.output_query.is_empty() {
            state.output_filtered_indices.clone()
        } else {
            (0..output.items.len()).collect()
        };

    // Build data rows.
    let rows: Vec<Row> = visible_indices
        .iter()
        .filter_map(|&i| output.items.get(i))
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

#[allow(clippy::too_many_lines)]
fn render_form(
    frame: &mut Frame,
    form: &crate::app::FormState,
    theme: &Theme,
    block: Block,
    area: ratatui::layout::Rect,
) {
    use crate::plugin::traits::FieldType;

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::raw("")); // top padding

    for (i, field) in form.fields.iter().enumerate() {
        let is_focused = i == form.focused;
        let label_style = if is_focused {
            Style::default().fg(theme.accent).bold()
        } else {
            Style::default().fg(theme.text)
        };

        match &field.spec.field_type {
            FieldType::Text => {
                // Label row.
                let mut label_spans = vec![Span::styled(
                    format!("  {} ", field.spec.label),
                    label_style,
                )];
                if field.spec.required {
                    label_spans.push(Span::styled("*", Style::default().fg(theme.error)));
                }
                lines.push(Line::from(label_spans));

                // Input row.
                if field.value.is_empty() {
                    let placeholder = field.spec.placeholder.as_deref().unwrap_or("");
                    let ph_style = Style::default().fg(theme.text_dimmed);
                    let cursor = if is_focused { "█" } else { "" };
                    lines.push(Line::from(vec![
                        Span::raw("  ["),
                        Span::styled(placeholder, ph_style),
                        Span::styled(cursor, Style::default().fg(theme.accent)),
                        Span::raw("]"),
                    ]));
                } else if is_focused {
                    let before = &field.value[..field.cursor];
                    let after = &field.value[field.cursor..];
                    let val_style = Style::default().fg(theme.text);
                    lines.push(Line::from(vec![
                        Span::raw("  ["),
                        Span::styled(before, val_style),
                        Span::styled("█", Style::default().fg(theme.accent)),
                        Span::styled(after, val_style),
                        Span::raw("]"),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw("  ["),
                        Span::styled(field.value.as_str(), Style::default().fg(theme.text)),
                        Span::raw("]"),
                    ]));
                }
            }
            FieldType::Select { options } => {
                let mut label_spans = vec![Span::styled(
                    format!("  {} ", field.spec.label),
                    label_style,
                )];
                if field.spec.required {
                    label_spans.push(Span::styled("*", Style::default().fg(theme.error)));
                }
                lines.push(Line::from(label_spans));

                let selected = options
                    .get(field.selected_option)
                    .map_or("", String::as_str);
                let sel_style = if is_focused {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .bg(theme.highlight_bg)
                } else {
                    Style::default().fg(theme.text)
                };
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("[▼ {selected}]"), sel_style),
                ]));
            }
            FieldType::Toggle => {
                let check = if field.toggled { "x" } else { " " };
                let tog_style = if is_focused {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .bg(theme.highlight_bg)
                } else {
                    Style::default().fg(theme.text)
                };
                let mut spans = vec![
                    Span::raw("  "),
                    Span::styled(format!("[{check}] "), tog_style),
                    Span::styled(field.spec.label.as_str(), label_style),
                ];
                if field.spec.required {
                    spans.push(Span::styled(" *", Style::default().fg(theme.error)));
                }
                lines.push(Line::from(spans));
            }
        }
        lines.push(Line::raw("")); // spacing between fields
    }

    // Submit button.
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("[ {} ]", form.submit_label),
            Style::default().fg(theme.accent).bold(),
        ),
    ]));

    // Hint.
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "  Tab: next  Shift+Tab: prev  Enter: submit  Esc: cancel",
        Style::default().fg(theme.text_dimmed),
    )));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Styled key hint: accent-colored key + dimmed label.
fn key_hint<'a>(key: &str, label: &str, theme: &Theme) -> Vec<Span<'a>> {
    vec![
        Span::styled(format!(" {key}"), Style::default().fg(theme.accent).bold()),
        Span::styled(format!(" {label}"), Style::default().fg(theme.text_dimmed)),
    ]
}

#[allow(clippy::too_many_lines)]
fn render_widget_row(
    frame: &mut Frame,
    state: &AppState,
    theme: &Theme,
    area: ratatui::layout::Rect,
) {
    use crate::app::CachedResult;

    let indices = &state.widget_indices;
    if indices.is_empty() || area.width < 10 {
        return;
    }

    // Split horizontally into equal-width cards.
    let card_count = indices.len().min(6); // max 6 cards
    let constraints: Vec<Constraint> = (0..card_count)
        .map(|_| Constraint::Percentage(100 / u16::try_from(card_count).unwrap_or(1)))
        .collect();
    let card_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    for (i, &pidx) in indices.iter().take(card_count).enumerate() {
        let Some(meta) = state.plugins.get(pidx) else {
            continue;
        };

        let is_selected = state.widget_focused && state.widget_selected == i;
        let border_color = if is_selected {
            theme.accent
        } else {
            theme.text_dimmed
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(
                format!(" {} {} ", meta.icon, meta.name),
                Style::default().fg(theme.accent).bold(),
            ));

        // Build card content from cache.
        let mut lines: Vec<Line> = Vec::new();

        match state.result_cache.get(&pidx) {
            Some(CachedResult::Ready(output) | CachedResult::Revalidating(output)) => {
                let max_lines = (area.height as usize).saturating_sub(2); // border eats 2
                for item in output.items.iter().take(max_lines) {
                    let label = if item.label.len() > card_areas[i].width as usize - 3 {
                        format!(
                            "{}…",
                            item.label
                                .chars()
                                .take(card_areas[i].width as usize - 4)
                                .collect::<String>()
                        )
                    } else {
                        item.label.clone()
                    };
                    lines.push(Line::from(Span::styled(
                        label,
                        Style::default().fg(theme.text),
                    )));
                }
                if output.items.len() > max_lines {
                    lines.push(Line::from(Span::styled(
                        format!("+{} more", output.items.len() - max_lines),
                        Style::default().fg(theme.text_dimmed),
                    )));
                }
                if lines.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "No items",
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
                let msg = if e.len() > 20 { &e[..20] } else { e };
                lines.push(Line::from(Span::styled(
                    format!("⚠ {msg}"),
                    Style::default().fg(theme.error),
                )));
            }
            None => {
                lines.push(Line::from(Span::styled(
                    "…",
                    Style::default().fg(theme.text_dimmed),
                )));
            }
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, card_areas[i]);
    }
}

#[allow(clippy::too_many_lines)]
fn render_plugin_manager(
    frame: &mut Frame,
    pm: &crate::app::PluginManagerState,
    theme: &Theme,
    area: ratatui::layout::Rect,
) {
    use crate::app::{PluginManagerRow, SecretSource};

    let items: Vec<ListItem> = pm
        .rows
        .iter()
        .map(|row| match row {
            PluginManagerRow::PluginHeader {
                name,
                icon,
                category,
                version,
                enabled,
                expanded,
                command_count,
                ..
            } => {
                let arrow = if *command_count > 1 {
                    if *expanded { "▼" } else { "►" }
                } else {
                    " "
                };
                let check = if *enabled { "x" } else { " " };
                let line = Line::from(vec![
                    Span::styled(
                        format!("{arrow} [{check}] "),
                        Style::default().fg(if *enabled {
                            theme.accent
                        } else {
                            theme.text_dimmed
                        }),
                    ),
                    Span::styled(format!("{icon} "), Style::default().bold()),
                    Span::styled(
                        name.as_str(),
                        Style::default()
                            .fg(if *enabled {
                                theme.text
                            } else {
                                theme.text_dimmed
                            })
                            .bold(),
                    ),
                    Span::styled(
                        format!("  {category}  v{version}"),
                        Style::default().fg(theme.text_dimmed),
                    ),
                ]);
                ListItem::new(line)
            }
            PluginManagerRow::Command {
                name,
                quickkey,
                enabled,
                ..
            } => {
                let check = if *enabled { "x" } else { " " };
                let qk = quickkey
                    .as_deref()
                    .map_or(String::new(), |q| format!(" ({q})"));
                let line = Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("[{check}] "),
                        Style::default().fg(if *enabled {
                            theme.accent
                        } else {
                            theme.text_dimmed
                        }),
                    ),
                    Span::styled(
                        format!("{name}{qk}"),
                        Style::default().fg(if *enabled {
                            theme.text
                        } else {
                            theme.text_dimmed
                        }),
                    ),
                ]);
                ListItem::new(line)
            }
            PluginManagerRow::Setting { label, value, .. } => {
                let line = Line::from(vec![
                    Span::raw("    "),
                    Span::styled("⚙ ", Style::default().fg(theme.text_dimmed)),
                    Span::styled(label.as_str(), Style::default().fg(theme.text)),
                    Span::styled(
                        format!(" = {value}"),
                        Style::default().fg(theme.text_dimmed),
                    ),
                ]);
                ListItem::new(line)
            }
            PluginManagerRow::Secret { key, source } => {
                let (status, style) = match source {
                    SecretSource::DotEnv => ("✅ .env", Style::default().fg(theme.accent)),
                    SecretSource::EnvVar => ("✅ env", Style::default().fg(theme.accent)),
                    SecretSource::Keychain => ("✅ keychain", Style::default().fg(theme.accent)),
                    SecretSource::NotSet => ("❌ NOT SET", Style::default().fg(theme.error)),
                };
                let line = Line::from(vec![
                    Span::raw("    "),
                    Span::styled("🔑 ", Style::default().fg(theme.text_dimmed)),
                    Span::styled(key.as_str(), Style::default().fg(theme.text)),
                    Span::raw("  "),
                    Span::styled(status, style),
                ]);
                ListItem::new(line)
            }
        })
        .collect();

    let block = Block::default()
        .title(" Plugin Manager ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent));

    let highlight_style = Style::default()
        .bg(theme.highlight_bg)
        .fg(theme.highlight_fg)
        .add_modifier(Modifier::BOLD);

    let list = List::new(items)
        .block(block)
        .highlight_style(highlight_style);

    let mut list_state = ListState::default().with_selected(Some(pm.selected));
    frame.render_stateful_widget(list, area, &mut list_state);
}

#[allow(clippy::too_many_lines)]
fn render_status_bar(
    frame: &mut Frame,
    state: &AppState,
    theme: &Theme,
    area: ratatui::layout::Rect,
) {
    let badge_style = Style::default()
        .fg(theme.status_bar_bg)
        .bg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let sep = Span::styled(" │ ", Style::default().fg(theme.text_dimmed));

    // Config warnings take priority.
    if let Some(warning) = state.warnings.first() {
        let mut spans = vec![
            Span::styled(" ⚠ WARNING ", badge_style),
            sep,
            Span::styled(
                format!("{warning} "),
                Style::default().fg(theme.error).bold(),
            ),
        ];
        // Pad the rest of the bar.
        spans.push(Span::raw(""));
        let bar = Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.status_bar_bg));
        frame.render_widget(bar, area);
        return;
    }

    // Flash message (expires after 2 seconds).
    if let Some((ref msg, ref started)) = state.status_message {
        if started.elapsed().as_secs_f32() < 2.0 {
            let mode_text = match state.vim_mode {
                VimMode::Normal => " NORMAL ",
                VimMode::Insert => " INSERT ",
                VimMode::Command => " COMMAND ",
            };
            let spans = vec![
                Span::styled(mode_text, badge_style),
                sep,
                Span::styled(
                    format!("✓ {msg} "),
                    Style::default().fg(theme.accent).bold(),
                ),
            ];
            let bar =
                Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.status_bar_bg));
            frame.render_widget(bar, area);
            return;
        }
    }

    // Build the main status bar line.
    let mut spans: Vec<Span> = Vec::new();

    if state.pending_confirmation.is_some() {
        spans.push(Span::styled(" CONFIRM ", badge_style));
        spans.push(sep);
        spans.extend(key_hint("Y", "yes", theme));
        spans.extend(key_hint("N", "no", theme));
    } else {
        match state.vim_mode {
            VimMode::Command => {
                spans.push(Span::styled(" COMMAND ", badge_style));
                spans.push(sep.clone());
                spans.push(Span::styled(
                    format!(" :{}\u{2588}", state.command_input),
                    Style::default().fg(theme.text).bold(),
                ));
            }
            VimMode::Insert => {
                spans.push(Span::styled(" INSERT ", badge_style));
                spans.push(sep.clone());
                spans.push(Span::styled(
                    " type to search or use quickkeys",
                    Style::default().fg(theme.text_dimmed),
                ));
                spans.extend(key_hint("Esc", "normal", theme));
            }
            VimMode::Normal => {
                spans.push(Span::styled(" NORMAL ", badge_style));
                spans.push(sep.clone());

                match state.mode {
                    Mode::Unified => {
                        if state.widget_focused {
                            // Widget row is focused — show widget-specific keys.
                            spans.extend(key_hint("h/l", "reorder", theme));
                            spans.extend(key_hint("⏎", "open", theme));
                            spans.extend(key_hint("A", "add/remove", theme));
                            spans.extend(key_hint("D", "disable", theme));
                            spans.extend(key_hint("W", "hide all", theme));
                            spans.extend(key_hint("j/Esc", "back to list", theme));
                        } else {
                            spans.extend(key_hint("j/k", "nav", theme));
                            spans.extend(key_hint("⏎", "run", theme));
                            spans.extend(key_hint("/", "search", theme));
                            spans.extend(key_hint(":", "cmd", theme));
                            spans.extend(key_hint("SPC", "menu", theme));
                            spans.extend(key_hint("q", "quit", theme));
                            if !state.widget_indices.is_empty() {
                                if state.widgets_visible {
                                    // Widgets showing — hint how to focus them.
                                    spans.extend(key_hint("K", "widgets", theme));
                                } else {
                                    // Widgets hidden — hint how to show them.
                                    spans.extend(key_hint("W", "show widgets", theme));
                                }
                            }
                        }
                        if state.sort_mode != crate::app::SortMode::Alpha {
                            spans.push(Span::styled(
                                format!("  ↓ {}", state.sort_mode.label()),
                                Style::default().fg(theme.accent).bold(),
                            ));
                        }
                    }
                    Mode::PluginManager => {
                        spans.extend(key_hint("j/k", "nav", theme));
                        spans.extend(key_hint("SPC", "toggle", theme));
                        spans.extend(key_hint("⏎", "expand", theme));
                        spans.extend(key_hint("s", "set secret", theme));
                        spans.extend(key_hint("x", "del secret", theme));
                        spans.extend(key_hint("q", "back", theme));
                    }
                    Mode::ViewOutput => {
                        if state.is_loading {
                            let spinner = SPINNER_CHARS[state.spinner_tick as usize % 8];
                            let elapsed = state
                                .loading_started
                                .map_or(0.0, |t| t.elapsed().as_secs_f32());
                            let name = state
                                .plugin_output
                                .as_ref()
                                .map_or("plugin", |o| o.title.as_str());
                            spans.push(Span::styled(
                                format!(" {spinner} {name}… ({elapsed:.1}s)"),
                                Style::default().fg(theme.text),
                            ));
                        } else {
                            let name = state
                                .plugin_output
                                .as_ref()
                                .map_or("output", |o| o.title.as_str());
                            let n = state.plugin_output.as_ref().map_or(0, |o| o.items.len());
                            if n > 0 {
                                spans.push(Span::styled(
                                    format!(" {name} — {n} items"),
                                    Style::default().fg(theme.text),
                                ));
                            } else {
                                spans.push(Span::styled(
                                    format!(" {name}"),
                                    Style::default().fg(theme.text),
                                ));
                            }
                            spans.extend(key_hint("j/k", "nav", theme));
                            spans.extend(key_hint("⏎", "action", theme));
                            spans.extend(key_hint("SPC", "menu", theme));
                            spans.extend(key_hint("Esc", "back", theme));
                        }
                    }
                }
            }
        }
    }

    // Append update hint at the end of the status bar if available.
    if let Some(ref version) = state.update_hint {
        let hint = state.install_method.upgrade_hint();
        spans.push(Span::styled(
            format!("  ↑ v{version} available: {hint}"),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let bar = Paragraph::new(Line::from(spans)).style(Style::default().bg(theme.status_bar_bg));
    frame.render_widget(bar, area);
}

/// Render a centered popup with categorized actions and key hints.
fn render_power_menu(frame: &mut Frame, menu: &PowerMenuState, theme: &Theme, area: Rect) {
    const COLS: usize = 3;
    const COL_WIDTH: u16 = 18;

    // Calculate content height: 1 line per category header + ceil(items/COLS) rows per category
    // + 1 blank line between categories.
    #[allow(clippy::cast_possible_truncation)]
    let cols_u16 = COLS as u16;
    let content_height: u16 = menu
        .categories
        .iter()
        .enumerate()
        .map(|(i, cat)| {
            #[allow(clippy::cast_possible_truncation)]
            let item_count = cat.items.len() as u16;
            let item_rows = item_count.div_ceil(cols_u16);
            let gap = u16::from(i + 1 < menu.categories.len());
            1 + item_rows + gap
        })
        .sum();

    let popup_width = (cols_u16 * COL_WIDTH + 4).min(area.width.saturating_sub(4));
    let popup_height = (content_height + 2).min(area.height.saturating_sub(2)); // +2 for border

    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear the area behind the popup.
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .title(Span::styled(
            " Power Menu ",
            Style::default().fg(theme.accent).bold(),
        ));

    let inner = block.inner(popup_area);

    // Build lines for the popup content.
    let mut lines: Vec<Line> = Vec::new();
    for (cat_idx, category) in menu.categories.iter().enumerate() {
        // Category header.
        lines.push(Line::from(Span::styled(
            format!("  {}", category.name),
            Style::default()
                .fg(theme.text_dimmed)
                .add_modifier(Modifier::BOLD),
        )));

        // Items in rows of COLS columns.
        for chunk in category.items.chunks(COLS) {
            let mut spans = Vec::new();
            spans.push(Span::raw("  "));
            for (i, item) in chunk.iter().enumerate() {
                spans.push(Span::styled(
                    format!(" {} ", item.key_hint),
                    Style::default()
                        .fg(theme.accent)
                        .bg(theme.highlight_bg)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    format!(" {}", item.label),
                    Style::default().fg(theme.text),
                ));
                // Pad to column width (except last in row).
                if i + 1 < chunk.len() {
                    let used = item.key_hint.len() + item.label.len() + 3; // " X " + " label"
                    let pad = (COL_WIDTH as usize).saturating_sub(used);
                    spans.push(Span::raw(" ".repeat(pad)));
                }
            }
            lines.push(Line::from(spans));
        }

        // Blank line between categories (except after the last one).
        if cat_idx + 1 < menu.categories.len() {
            lines.push(Line::from(""));
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(block, popup_area);
    frame.render_widget(paragraph, inner);
}

/// Render the theme preset picker as a centered popup.
///
/// Shows all built-in presets; the selected one is highlighted. Navigating
/// with j/k swaps the live theme before this popup is drawn.
fn render_theme_picker(frame: &mut Frame, picker: &ThemePickerState, theme: &Theme, area: Rect) {
    let presets = crate::config::PRESET_NAMES;
    #[allow(clippy::cast_possible_truncation)]
    let popup_height = (presets.len() as u16 + 4).min(area.height.saturating_sub(2));
    let popup_width: u16 = 30_u16.min(area.width.saturating_sub(4));

    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .title(Span::styled(
            " Theme ",
            Style::default().fg(theme.accent).bold(),
        ));

    let items: Vec<ListItem> = presets
        .iter()
        .enumerate()
        .map(|(i, (_, label))| {
            let style = if i == picker.selected {
                Style::default()
                    .fg(theme.highlight_fg)
                    .bg(theme.highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            ListItem::new(format!("  {label}  ")).style(style)
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(picker.selected));

    let inner = block.inner(popup_area);

    // Split inner: list rows + footer hint.
    let inner_chunks = ratatui::layout::Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let list = List::new(items);
    frame.render_stateful_widget(list, inner_chunks[0], &mut list_state);

    let hint = Paragraph::new(Span::styled(
        " Enter confirm · Esc cancel",
        Style::default().fg(theme.text_dimmed),
    ));
    frame.render_widget(hint, inner_chunks[1]);
    frame.render_widget(block, popup_area);
}

/// Render the widget picker as a centered popup with toggle checkboxes.
fn render_widget_picker(
    frame: &mut Frame,
    picker: &crate::app::WidgetPickerState,
    theme: &Theme,
    area: Rect,
) {
    let visible = picker.visible_entries();
    let has_query = !picker.query.is_empty();
    let extra_rows = if has_query { 5 } else { 4 }; // +1 for search line
    #[allow(clippy::cast_possible_truncation)]
    let popup_height = (visible.len() as u16 + extra_rows).min(area.height.saturating_sub(2));
    let popup_width: u16 = 45_u16.min(area.width.saturating_sub(4));

    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let title = if has_query {
        format!(" Widgets [{}/{}] ", visible.len(), picker.entries.len())
    } else {
        " Widgets ".to_string()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .title(Span::styled(
            title,
            Style::default().fg(theme.accent).bold(),
        ));

    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(vi, (_orig_idx, entry))| {
            let check = if entry.enabled { "[x]" } else { "[ ]" };
            let text = format!(" {check} {} {}", entry.icon, entry.label);
            let style = if vi == picker.selected {
                Style::default()
                    .fg(theme.highlight_fg)
                    .bg(theme.highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else if entry.enabled {
                Style::default().fg(theme.text)
            } else {
                Style::default().fg(theme.text_dimmed)
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(picker.selected));

    let inner = block.inner(popup_area);

    let constraints = if has_query {
        vec![
            Constraint::Length(1), // search line
            Constraint::Min(1),    // list
            Constraint::Length(1), // hints
        ]
    } else {
        vec![
            Constraint::Min(1),    // list
            Constraint::Length(1), // hints
        ]
    };
    let inner_chunks = ratatui::layout::Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let (list_area, hint_area) = if has_query {
        let search_line = Paragraph::new(Span::styled(
            format!(" / {}", picker.query),
            Style::default().fg(theme.accent),
        ));
        frame.render_widget(search_line, inner_chunks[0]);
        (inner_chunks[1], inner_chunks[2])
    } else {
        (inner_chunks[0], inner_chunks[1])
    };

    let list = List::new(items);
    frame.render_stateful_widget(list, list_area, &mut list_state);

    let hint_text = if has_query {
        " Space toggle · ⌫ clear · Esc close"
    } else {
        " Space toggle · type to filter · Esc close"
    };
    let hint = Paragraph::new(Span::styled(
        hint_text,
        Style::default().fg(theme.text_dimmed),
    ));
    frame.render_widget(hint, hint_area);
    frame.render_widget(block, popup_area);
}
