//! Syntax highlighting for code blocks.
//!
//! Uses `syntect` to highlight fenced code blocks in markdown output.
//! Unknown languages fall back to plain monospace text.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Highlight a code block and return styled ratatui `Line` objects.
///
/// Each line is indented with two spaces. If the language is not recognized,
/// the code is returned as plain dimmed text.
#[must_use]
pub fn highlight_code<'a>(code: &str, language: &str) -> Vec<Line<'a>> {
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    let syntax = ss
        .find_syntax_by_token(language)
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    let theme = &ts.themes["base16-ocean.dark"];
    let mut highlighter = HighlightLines::new(syntax, theme);

    let mut lines = Vec::new();

    for line in LinesWithEndings::from(code) {
        let Ok(regions) = highlighter.highlight_line(line, &ss) else {
            // Fallback: plain text.
            lines.push(Line::from(Span::styled(
                format!("  {}", line.trim_end()),
                Style::default().fg(Color::Gray),
            )));
            continue;
        };

        let mut spans: Vec<Span<'a>> = vec![Span::raw("  ")]; // indent
        for (style, text) in regions {
            let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
            spans.push(Span::styled(
                text.trim_end_matches('\n').to_string(),
                Style::default().fg(fg),
            ));
        }
        lines.push(Line::from(spans));
    }

    lines
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust_code() {
        let code = "fn main() {\n    println!(\"hello\");\n}\n";
        let lines = highlight_code(code, "rust");
        assert_eq!(lines.len(), 3);
        // Each line should have multiple colored spans (not just one plain span).
        assert!(lines[0].spans.len() > 1);
    }

    #[test]
    fn unknown_language_falls_back_to_plain() {
        let code = "some unknown content\n";
        let lines = highlight_code(code, "nonexistent_lang_xyz");
        assert_eq!(lines.len(), 1);
        // Should still render (plain text syntax).
        assert!(!lines[0].spans.is_empty());
    }

    #[test]
    fn empty_code_returns_empty() {
        let lines = highlight_code("", "rust");
        assert!(lines.is_empty());
    }
}
