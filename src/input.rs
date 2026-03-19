//! Keyboard event to [`Action`] mapping.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::app::Mode;

/// Maps a raw crossterm key event to a semantic [`Action`], depending on the current UI mode.
///
/// Returns `None` for keys that have no binding in the current mode.
pub fn handle_key(event: KeyEvent, mode: &Mode) -> Option<Action> {
    match mode {
        Mode::Browse => handle_browse(event),
        Mode::Search => handle_search(event),
        Mode::ViewOutput => handle_view_output(event),
    }
}

fn handle_browse(event: KeyEvent) -> Option<Action> {
    match event.code {
        // Navigation — vim keys and arrows
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
        // Selection
        KeyCode::Enter => Some(Action::Select),
        // Enter search mode by typing '/' or any printable character
        KeyCode::Char('/') => Some(Action::Search('/')),
        KeyCode::Char(c) if !c.is_control() => Some(Action::Search(c)),
        // Quit
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        _ => None,
    }
}

fn handle_search(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Char(c) if !c.is_control() => Some(Action::Search(c)),
        KeyCode::Backspace | KeyCode::Delete => Some(Action::BackspaceSearch),
        KeyCode::Esc => Some(Action::ClearSearch),
        KeyCode::Enter => Some(Action::Select),
        KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        _ => None,
    }
}

fn handle_view_output(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => Some(Action::Back),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Enter => Some(Action::Execute),
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        _ => None,
    }
}
