//! Keyboard event to [`Action`] mapping.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::app::Mode;
use crate::config::{BrowseAction, ResolvedKeybindings, ViewOutputAction};

/// Maps a raw crossterm key event to a semantic [`Action`], depending on the current UI mode.
///
/// The configurable keybinding map is checked first; hardcoded fallbacks handle keys that
/// aren't in the map (search-mode char catch-all, Ctrl+C).
///
/// Returns `None` for keys that have no binding in the current mode.
pub fn handle_key(
    event: KeyEvent,
    mode: &Mode,
    keybindings: &ResolvedKeybindings,
) -> Option<Action> {
    match mode {
        Mode::Browse => handle_browse(event, keybindings),
        Mode::Search => handle_search(event),
        Mode::ViewOutput => handle_view_output(event, keybindings),
    }
}

fn handle_browse(event: KeyEvent, keybindings: &ResolvedKeybindings) -> Option<Action> {
    // Check direct-launch map first.
    if let Some(plugin_name) = keybindings.launch_map.get(&event) {
        return Some(Action::LaunchPlugin(plugin_name.clone()));
    }

    // Check configurable browse map.
    if let Some(action) = keybindings.browse_map.get(&event) {
        return Some(match action {
            BrowseAction::MoveUp => Action::MoveUp,
            BrowseAction::MoveDown => Action::MoveDown,
            BrowseAction::Select => Action::Select,
            BrowseAction::Quit => Action::Quit,
        });
    }

    // Hardcoded fallbacks not covered by the configurable map.
    match event.code {
        // Ctrl+C is always quit — non-configurable.
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        // Enter search mode on '/' or any printable char not already mapped.
        KeyCode::Char('/') => Some(Action::Search('/')),
        KeyCode::Char(c) if !c.is_control() => Some(Action::Search(c)),
        _ => None,
    }
}

fn handle_search(event: KeyEvent) -> Option<Action> {
    // Search mode is fully hardcoded — no user overrides.
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

fn handle_view_output(event: KeyEvent, keybindings: &ResolvedKeybindings) -> Option<Action> {
    // Check configurable view_output map.
    if let Some(action) = keybindings.view_output_map.get(&event) {
        return Some(match action {
            ViewOutputAction::MoveUp => Action::MoveUp,
            ViewOutputAction::MoveDown => Action::MoveDown,
            ViewOutputAction::Back => Action::Back,
            ViewOutputAction::Execute => Action::Execute,
            ViewOutputAction::Quit => Action::Quit,
        });
    }

    // Hardcoded fallback: Ctrl+C.
    match event.code {
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        _ => None,
    }
}
