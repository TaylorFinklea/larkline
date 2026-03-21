//! Keyboard event to [`Action`] mapping.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::app::{Mode, VimMode};
use crate::config::{BrowseAction, ResolvedKeybindings, ViewOutputAction};

/// Maps a raw crossterm key event to a semantic [`Action`].
///
/// Routing priority:
/// 1. `VimMode::Command` → command input handler (regardless of UI mode)
/// 2. `VimMode::Insert` + Browse/Search → quickkey / search handler
/// 3. `VimMode::Normal` + Browse → normal browse handler (j/k/q active, no quickkeys)
/// 4. `VimMode::Normal` + `ViewOutput` → output navigation handler
///
/// Returns `None` for keys with no binding in the current mode combination.
pub fn handle_key(
    event: KeyEvent,
    mode: &Mode,
    vim_mode: &VimMode,
    keybindings: &ResolvedKeybindings,
    has_pending_confirmation: bool,
) -> Option<Action> {
    // Confirmation dialog intercepts all keys.
    if has_pending_confirmation {
        return handle_confirmation(event);
    }

    match vim_mode {
        VimMode::Command => handle_command(event),
        VimMode::Insert => handle_insert(event, keybindings),
        VimMode::Normal => match mode {
            Mode::Unified => handle_browse_normal(event, keybindings),
            Mode::ViewOutput => handle_view_output(event, keybindings),
        },
    }
}

/// Confirmation dialog handler: y/Enter confirms, n/Esc cancels.
fn handle_confirmation(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Char('y' | 'Y') | KeyCode::Enter => Some(Action::Confirm),
        KeyCode::Char('n' | 'N') | KeyCode::Esc => Some(Action::Cancel),
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        _ => None,
    }
}

/// Normal-mode browse handler: navigation keys active, no quickkeys or char search.
fn handle_browse_normal(event: KeyEvent, keybindings: &ResolvedKeybindings) -> Option<Action> {
    // Check configurable browse map (j/k/q/R/Enter).
    if let Some(action) = keybindings.browse_map.get(&event) {
        return Some(match action {
            BrowseAction::MoveUp => Action::MoveUp,
            BrowseAction::MoveDown => Action::MoveDown,
            BrowseAction::Select => Action::Select,
            BrowseAction::Quit => Action::Quit,
            BrowseAction::Refresh => Action::RefreshPlugins,
            BrowseAction::ScrollHalfPageDown => Action::ScrollHalfPageDown,
            BrowseAction::ScrollHalfPageUp => Action::ScrollHalfPageUp,
        });
    }

    match event.code {
        // Ctrl+C is always quit — non-configurable.
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        // Enter Insert mode via 'i' (quickkeys) or '/' (search). Both activate Insert.
        KeyCode::Char('i' | '/') if event.modifiers == KeyModifiers::NONE => {
            Some(Action::EnterInsertMode)
        }
        // Enter Command mode.
        KeyCode::Char(':') if event.modifiers == KeyModifiers::NONE => {
            Some(Action::EnterCommandMode)
        }
        _ => None,
    }
}

/// Insert-mode handler: quickkeys checked first, then char → search, arrows still navigate.
fn handle_insert(event: KeyEvent, keybindings: &ResolvedKeybindings) -> Option<Action> {
    // Ctrl+C always quits.
    if let KeyCode::Char('c') = event.code {
        if event.modifiers.contains(KeyModifiers::CONTROL) {
            return Some(Action::Quit);
        }
    }

    // Escape exits Insert mode → Normal mode (also clears search via EnterNormalMode handler).
    if event.code == KeyCode::Esc {
        return Some(Action::EnterNormalMode);
    }

    // Arrow keys navigate the list unambiguously.
    if event.code == KeyCode::Up {
        return Some(Action::MoveUp);
    }
    if event.code == KeyCode::Down {
        return Some(Action::MoveDown);
    }

    // Enter selects the highlighted plugin.
    if event.code == KeyCode::Enter {
        return Some(Action::Select);
    }

    // Backspace / Delete edit the search query.
    if matches!(event.code, KeyCode::Backspace | KeyCode::Delete) {
        return Some(Action::BackspaceSearch);
    }

    // Check launch map first — j/k/q are valid quickkeys in Insert mode.
    if let Some(plugin_name) = keybindings.launch_map.get(&event) {
        return Some(Action::LaunchPlugin(plugin_name.clone()));
    }

    // Any remaining printable char goes to the search query.
    if let KeyCode::Char(c) = event.code {
        if !c.is_control() {
            return Some(Action::Search(c));
        }
    }

    None
}

/// Command-mode handler: accumulate `:command` input, Esc cancels, Enter submits.
fn handle_command(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Esc => Some(Action::EnterNormalMode),
        KeyCode::Enter => Some(Action::CommandSubmit),
        KeyCode::Backspace | KeyCode::Delete => Some(Action::CommandBackspace),
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        KeyCode::Char(c) if !c.is_control() => Some(Action::CommandChar(c)),
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
            ViewOutputAction::ScrollHalfPageDown => Action::ScrollHalfPageDown,
            ViewOutputAction::ScrollHalfPageUp => Action::ScrollHalfPageUp,
            ViewOutputAction::ToggleOutputMode => Action::ToggleOutputMode,
        });
    }

    // Hardcoded fallback: Ctrl+C.
    match event.code {
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        _ => None,
    }
}
