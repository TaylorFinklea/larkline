//! Markdown-to-ratatui conversion.
//!
//! Converts a markdown string into a `ratatui::text::Text` for rendering in the output pane.
//! Uses `pulldown-cmark` for parsing and maps events to styled `Span`/`Line` sequences.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::config::Theme;

/// Convert a markdown string into styled ratatui `Text`.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn markdown_to_text<'a>(input: &str, theme: &Theme) -> Text<'a> {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let parser = Parser::new_ext(input, options);

    let mut lines: Vec<Line<'a>> = Vec::new();
    let mut current_spans: Vec<Span<'a>> = Vec::new();
    let mut style_stack: Vec<Style> = vec![Style::default().fg(theme.text)];
    let mut in_code_block = false;
    let mut code_block_buf = String::new();
    let mut code_block_lang = String::new();
    let mut list_depth: usize = 0;
    let mut in_blockquote = false;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    let style = match level {
                        HeadingLevel::H1 => Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                        HeadingLevel::H2 => Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                        _ => Style::default()
                            .fg(theme.text)
                            .add_modifier(Modifier::BOLD),
                    };
                    style_stack.push(style);
                }
                Tag::Emphasis => {
                    let base = current_style(&style_stack);
                    style_stack.push(base.add_modifier(Modifier::ITALIC));
                }
                Tag::Strong => {
                    let base = current_style(&style_stack);
                    style_stack.push(base.add_modifier(Modifier::BOLD));
                }
                Tag::Strikethrough => {
                    let base = current_style(&style_stack);
                    style_stack.push(base.add_modifier(Modifier::CROSSED_OUT));
                }
                Tag::CodeBlock(kind) => {
                    in_code_block = true;
                    code_block_buf.clear();
                    code_block_lang = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                            lang.split_whitespace()
                                .next()
                                .unwrap_or("")
                                .to_string()
                        }
                        pulldown_cmark::CodeBlockKind::Indented => String::new(),
                    };
                }
                Tag::Link { dest_url, .. } => {
                    let style = Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::UNDERLINED);
                    style_stack.push(style);
                    // Store URL to append after link text.
                    // We'll handle it in the End event.
                    let _ = dest_url;
                }
                Tag::List(_) => {
                    list_depth += 1;
                }
                Tag::Item => {
                    // Flush current line before starting a new list item.
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    let indent = "  ".repeat(list_depth);
                    current_spans
                        .push(Span::styled(format!("{indent}• "), current_style(&style_stack)));
                }
                Tag::BlockQuote(_) => {
                    in_blockquote = true;
                }
                _ => {}
            },

            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    style_stack.pop();
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    lines.push(Line::raw("")); // spacing after heading
                }
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => {
                    style_stack.pop();
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    if code_block_lang.is_empty() {
                        // No language — plain dimmed text.
                        for code_line in code_block_buf.lines() {
                            lines.push(Line::from(Span::styled(
                                format!("  {code_line}"),
                                Style::default().fg(theme.text_dimmed),
                            )));
                        }
                    } else {
                        // Syntax-highlighted code.
                        let highlighted =
                            crate::tui::highlight::highlight_code(
                                &code_block_buf,
                                &code_block_lang,
                            );
                        lines.extend(highlighted);
                    }
                    lines.push(Line::raw("")); // spacing after code block
                    code_block_buf.clear();
                }
                TagEnd::List(_) => {
                    list_depth = list_depth.saturating_sub(1);
                    if list_depth == 0 {
                        // Flush and add spacing after top-level list.
                        if !current_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                        }
                        lines.push(Line::raw(""));
                    }
                }
                TagEnd::Item => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                }
                TagEnd::BlockQuote(_) => {
                    in_blockquote = false;
                }
                TagEnd::Paragraph => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                    lines.push(Line::raw("")); // paragraph spacing
                }
                _ => {}
            },

            Event::Text(text) => {
                if in_code_block {
                    code_block_buf.push_str(&text);
                } else {
                    let style = current_style(&style_stack);
                    let prefix = if in_blockquote { "│ " } else { "" };
                    current_spans
                        .push(Span::styled(format!("{prefix}{text}"), style));
                }
            }

            Event::Code(code) => {
                // Inline code.
                current_spans.push(Span::styled(
                    format!("`{code}`"),
                    Style::default().fg(theme.text_dimmed),
                ));
            }

            Event::SoftBreak | Event::HardBreak => {
                if !current_spans.is_empty() {
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                }
            }

            Event::Rule => {
                if !current_spans.is_empty() {
                    lines.push(Line::from(std::mem::take(&mut current_spans)));
                }
                lines.push(Line::from(Span::styled(
                    "────────────────────────────────────────",
                    Style::default().fg(theme.text_dimmed),
                )));
                lines.push(Line::raw(""));
            }

            _ => {}
        }
    }

    // Flush remaining spans.
    if !current_spans.is_empty() {
        lines.push(Line::from(current_spans));
    }

    Text::from(lines)
}

/// Get the current style from the stack (top element or default).
fn current_style(stack: &[Style]) -> Style {
    stack.last().copied().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn test_theme() -> Theme {
        Theme {
            accent: Color::Cyan,
            text: Color::White,
            text_dimmed: Color::Gray,
            highlight_bg: Color::DarkGray,
            highlight_fg: Color::White,
            error: Color::Red,
            status_bar_bg: Color::DarkGray,
        }
    }

    #[test]
    fn heading_renders_bold_accent() {
        let text = markdown_to_text("# Hello", &test_theme());
        let first_line = &text.lines[0];
        assert!(!first_line.spans.is_empty());
        let span = &first_line.spans[0];
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn inline_code_renders_dimmed() {
        let text = markdown_to_text("Use `cargo build`", &test_theme());
        let line = &text.lines[0];
        // Should have at least two spans: text + inline code
        assert!(line.spans.len() >= 2);
        let code_span = line.spans.iter().find(|s| s.content.contains("cargo"));
        assert!(code_span.is_some());
    }

    #[test]
    fn code_block_renders_indented() {
        let md = "```\nlet x = 1;\n```";
        let text = markdown_to_text(md, &test_theme());
        let code_line = text
            .lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("let x")));
        assert!(code_line.is_some());
    }

    #[test]
    fn list_renders_bullets() {
        let md = "- item one\n- item two";
        let text = markdown_to_text(md, &test_theme());
        let bullet_lines: Vec<_> = text
            .lines
            .iter()
            .filter(|l| l.spans.iter().any(|s| s.content.contains('•')))
            .collect();
        assert_eq!(bullet_lines.len(), 2);
    }

    #[test]
    fn horizontal_rule_renders() {
        let text = markdown_to_text("---", &test_theme());
        let rule_line = text
            .lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains('─')));
        assert!(rule_line.is_some());
    }

    #[test]
    fn empty_input_returns_empty() {
        let text = markdown_to_text("", &test_theme());
        assert!(text.lines.is_empty());
    }

    #[test]
    fn backward_compat_plain_text() {
        let text = markdown_to_text("just plain text", &test_theme());
        assert!(!text.lines.is_empty());
        assert!(text.lines[0].spans[0].content.contains("just plain text"));
    }
}
