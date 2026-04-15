//! Keyboard event to [`Action`] mapping.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::app::{Mode, PowerMenuCategory, VimMode};
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
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
pub fn handle_key(
    event: KeyEvent,
    mode: &Mode,
    vim_mode: &VimMode,
    keybindings: &ResolvedKeybindings,
    has_pending_confirmation: bool,
    has_copy_menu: bool,
    has_output_search: bool,
    has_form: bool,
    has_action_palette: bool,
    has_theme_picker: bool,
    has_widget_picker: bool,
    power_menu: Option<&[PowerMenuCategory]>,
    pending_g: bool,
    widget_focused: bool,
) -> Option<Action> {
    // Confirmation dialog intercepts all keys.
    if has_pending_confirmation {
        return handle_confirmation(event);
    }

    // Copy menu intercepts keys when open.
    if has_copy_menu {
        return handle_copy_menu(event);
    }

    // Theme picker intercepts keys when open.
    if has_theme_picker {
        return handle_theme_picker(event);
    }

    // Widget picker intercepts keys when open.
    if has_widget_picker {
        return handle_widget_picker(event);
    }

    // Action palette intercepts keys when open.
    if has_action_palette && *mode == Mode::ViewOutput {
        return handle_action_palette(event);
    }

    // Power menu intercepts keys when open.
    if let Some(categories) = power_menu {
        return handle_power_menu(event, categories);
    }

    // Form input intercepts all keys when a form is active.
    if has_form && *mode == Mode::ViewOutput {
        return handle_form_input(event);
    }

    // Output search mode intercepts keys in ViewOutput.
    if has_output_search && *mode == Mode::ViewOutput {
        return handle_output_search(event);
    }

    // Pending `g` — if the user pressed `g` in Normal mode, the next `g` triggers GoToFirst.
    // Any other key cancels the pending state and falls through to normal handling.
    if pending_g
        && *vim_mode == VimMode::Normal
        && event.code == KeyCode::Char('g')
        && event.modifiers == KeyModifiers::NONE
    {
        return Some(Action::GoToFirst);
    }

    match vim_mode {
        VimMode::Command => handle_command(event),
        VimMode::Insert => handle_insert(event, keybindings),
        VimMode::Normal => match mode {
            Mode::Unified => handle_browse_normal(event, keybindings, widget_focused),
            Mode::ViewOutput => handle_view_output(event, keybindings),
            Mode::MiniApp => handle_mini_app(event, keybindings),
            Mode::PluginManager => handle_plugin_manager(event),
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
fn handle_browse_normal(
    event: KeyEvent,
    keybindings: &ResolvedKeybindings,
    widget_focused: bool,
) -> Option<Action> {
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

    // Widget-focused Enter: drill into widget card output.
    if widget_focused
        && matches!(event.code, KeyCode::Enter)
        && event.modifiers == KeyModifiers::NONE
    {
        return Some(Action::WidgetCardOpen);
    }

    // Widget-focused h/Left: navigate to previous widget card.
    if widget_focused
        && matches!(event.code, KeyCode::Char('h') | KeyCode::Left)
        && event.modifiers == KeyModifiers::NONE
    {
        return Some(Action::Back);
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
        // Toggle description visibility.
        KeyCode::Char('d') if event.modifiers == KeyModifiers::NONE => {
            Some(Action::ToggleDescriptions)
        }
        // Power menu (which-key style overlay).
        KeyCode::Char(' ') if event.modifiers == KeyModifiers::NONE => Some(Action::PowerMenuOpen),
        // G → jump to last item.
        KeyCode::Char('G') => Some(Action::GoToLast),
        // g → start pending-g sequence (gg = jump to first).
        KeyCode::Char('g') if event.modifiers == KeyModifiers::NONE => Some(Action::PendingG),
        // Toggle sidebar/preview pane visibility.
        KeyCode::Char('s') if event.modifiers == KeyModifiers::NONE => Some(Action::ToggleSidebar),
        // Cycle sort order (O = Shift+O).
        KeyCode::Char('O') => Some(Action::CycleSort),
        // Esc in Normal mode: clear search query (if any).
        KeyCode::Esc => Some(Action::EnterNormalMode),
        // Widget management (Shift keys).
        KeyCode::Char('W') => Some(Action::WidgetToggleVisibility),
        KeyCode::Char('K') => Some(Action::WidgetFocusUp),
        KeyCode::Char('H') => Some(Action::WidgetMoveLeft),
        KeyCode::Char('L') => Some(Action::WidgetMoveRight),
        KeyCode::Char('D') => Some(Action::WidgetDisable),
        // Widget picker — choose which widgets to show.
        KeyCode::Char('A') => Some(Action::WidgetPickerOpen),
        // Plugin manager (P = Shift+P).
        KeyCode::Char('P') => Some(Action::PluginManagerOpen),
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

/// Action palette handler: j/k navigate, Enter selects, Esc dismisses, type to filter.
fn handle_action_palette(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Enter => Some(Action::PaletteSelect),
        KeyCode::Esc | KeyCode::Char('q') => Some(Action::PaletteDismiss),
        KeyCode::Backspace | KeyCode::Delete => Some(Action::PaletteBackspace),
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        KeyCode::Char(c) if !c.is_control() => Some(Action::PaletteSearch(c)),
        _ => None,
    }
}

/// Power menu handler: single-key dispatch — press a shortcut key to execute immediately.
fn handle_power_menu(event: KeyEvent, categories: &[PowerMenuCategory]) -> Option<Action> {
    match event.code {
        KeyCode::Esc | KeyCode::Char(' ') => Some(Action::PowerMenuDismiss),
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        KeyCode::Char(c) => {
            // Find matching item across all categories.
            for cat in categories {
                for item in &cat.items {
                    if item.key == c {
                        return Some(item.action.clone());
                    }
                }
            }
            None
        }
        KeyCode::Enter => Some(Action::Execute),
        _ => None,
    }
}

/// Copy menu handler: j/k navigate, Enter selects, Esc dismisses.
fn handle_copy_menu(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Enter => Some(Action::CopyMenuSelect),
        KeyCode::Esc | KeyCode::Char('q') => Some(Action::CopyMenuDismiss),
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
            ViewOutputAction::ScrollHalfPageDown => Action::ScrollHalfPageDown,
            ViewOutputAction::ScrollHalfPageUp => Action::ScrollHalfPageUp,
            ViewOutputAction::ToggleOutputMode => Action::ToggleOutputMode,
            ViewOutputAction::CopyLabel => Action::CopyLabel,
            ViewOutputAction::CopyMenu => Action::CopyMenu,
            ViewOutputAction::Search => Action::OutputEnterSearch,
            ViewOutputAction::OpenUrl => Action::OpenUrl,
            ViewOutputAction::ActionPalette => Action::PaletteOpen,
        });
    }

    // Hardcoded fallbacks.
    match event.code {
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        KeyCode::Char(' ') if event.modifiers == KeyModifiers::NONE => Some(Action::PowerMenuOpen),
        KeyCode::Char('G') => Some(Action::GoToLast),
        KeyCode::Char('g') if event.modifiers == KeyModifiers::NONE => Some(Action::PendingG),
        KeyCode::Char('s') if event.modifiers == KeyModifiers::NONE => Some(Action::ToggleSidebar),
        KeyCode::Char('r') if event.modifiers == KeyModifiers::NONE => Some(Action::RerunCommand),
        // F → expand to mini app mode (single-pane wrapper).
        KeyCode::Char('F') => Some(Action::MiniAppExpand),
        _ => None,
    }
}

/// Mini app mode handler: pane navigation, item actions, and exit.
fn handle_mini_app(event: KeyEvent, keybindings: &ResolvedKeybindings) -> Option<Action> {
    // Check configurable view_output map for item navigation (j/k/Enter/etc).
    if let Some(action) = keybindings.view_output_map.get(&event) {
        return Some(match action {
            ViewOutputAction::MoveUp => Action::MoveUp,
            ViewOutputAction::MoveDown => Action::MoveDown,
            ViewOutputAction::Back => Action::MiniAppClose,
            ViewOutputAction::Execute => Action::Execute,
            ViewOutputAction::Quit => Action::Quit,
            ViewOutputAction::ScrollHalfPageDown => Action::ScrollHalfPageDown,
            ViewOutputAction::ScrollHalfPageUp => Action::ScrollHalfPageUp,
            ViewOutputAction::ToggleOutputMode => Action::ToggleOutputMode,
            ViewOutputAction::CopyLabel => Action::CopyLabel,
            ViewOutputAction::CopyMenu => Action::CopyMenu,
            ViewOutputAction::Search => Action::OutputEnterSearch,
            ViewOutputAction::OpenUrl => Action::OpenUrl,
            ViewOutputAction::ActionPalette => Action::PaletteOpen,
        });
    }

    match event.code {
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        // Tab / Shift+Tab to cycle pane focus.
        KeyCode::Tab if event.modifiers.contains(KeyModifiers::SHIFT) => {
            Some(Action::MiniAppFocusPrev)
        }
        KeyCode::Tab => Some(Action::MiniAppFocusNext),
        // Ctrl+h / Ctrl+l to cycle pane focus (vim-style).
        KeyCode::Char('h') if event.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::MiniAppFocusPrev)
        }
        KeyCode::Char('l') if event.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::MiniAppFocusNext)
        }
        // Space for power menu.
        KeyCode::Char(' ') if event.modifiers == KeyModifiers::NONE => Some(Action::PowerMenuOpen),
        // Split panes: Ctrl+\ horizontal, Ctrl+- vertical (like tmux).
        KeyCode::Char('\\') if event.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::MiniAppSplitH)
        }
        KeyCode::Char('-') if event.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::MiniAppSplitV)
        }
        // Close focused pane.
        KeyCode::Char('x') if event.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::MiniAppClosePane)
        }
        // Resize: +/- to grow/shrink focused pane.
        KeyCode::Char('+') => Some(Action::MiniAppResizeGrow),
        KeyCode::Char('_') => Some(Action::MiniAppResizeShrink),
        // G / gg for jump.
        KeyCode::Char('G') => Some(Action::GoToLast),
        KeyCode::Char('g') if event.modifiers == KeyModifiers::NONE => Some(Action::PendingG),
        _ => None,
    }
}

/// Form input handler: Tab cycles fields, chars go to text, Enter submits, Esc cancels.
fn handle_form_input(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Tab => {
            if event.modifiers.contains(KeyModifiers::SHIFT) {
                Some(Action::FormPrevField)
            } else {
                Some(Action::FormNextField)
            }
        }
        KeyCode::Enter => Some(Action::FormSubmit),
        KeyCode::Esc => Some(Action::FormCancel),
        KeyCode::Backspace | KeyCode::Delete => Some(Action::FormBackspace),
        KeyCode::Left => Some(Action::FormCursorLeft),
        KeyCode::Right => Some(Action::FormCursorRight),
        KeyCode::Up => Some(Action::FormSelectPrev),
        KeyCode::Down => Some(Action::FormSelectNext),
        KeyCode::Char(' ') => Some(Action::FormToggle),
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        KeyCode::Char(c) if !c.is_control() => Some(Action::FormInput(c)),
        _ => None,
    }
}

/// Output search handler: chars append to query, Backspace removes, Esc clears.
fn handle_output_search(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Backspace | KeyCode::Delete => Some(Action::OutputBackspaceSearch),
        // Esc exits search mode but keeps the filter; Enter also exits.
        KeyCode::Esc | KeyCode::Enter => Some(Action::OutputExitSearch),
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        // Navigation still works during search.
        KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Char(c) if !c.is_control() => Some(Action::OutputSearch(c)),
        _ => None,
    }
}

/// Widget picker handler: j/k navigate, Space toggles, type to filter, Esc/q closes.
fn handle_widget_picker(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::WidgetPickerDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::WidgetPickerUp),
        KeyCode::Char(' ') | KeyCode::Enter => Some(Action::WidgetPickerToggle),
        KeyCode::Esc | KeyCode::Char('q') => Some(Action::WidgetPickerClose),
        KeyCode::Backspace | KeyCode::Delete => Some(Action::WidgetPickerBackspace),
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        KeyCode::Char(c) if !c.is_control() => Some(Action::WidgetPickerSearch(c)),
        _ => None,
    }
}

/// Theme picker handler: j/k or arrows navigate, Enter confirms, Esc/q cancels.
fn handle_theme_picker(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Enter => Some(Action::ThemePickerClose { confirmed: true }),
        KeyCode::Esc | KeyCode::Char('q') => Some(Action::ThemePickerClose { confirmed: false }),
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        _ => None,
    }
}

/// Plugin manager keybinding handler.
fn handle_plugin_manager(event: KeyEvent) -> Option<Action> {
    match event.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Char(' ') => Some(Action::PluginManagerToggle),
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => Some(Action::PluginManagerExpand),
        KeyCode::Char('s') => Some(Action::PluginManagerSetSecret),
        KeyCode::Char('x') => Some(Action::PluginManagerDeleteSecret),
        KeyCode::Char('G') => Some(Action::GoToLast),
        KeyCode::Char('g') => Some(Action::PendingG),
        KeyCode::Esc | KeyCode::Char('q' | 'h') | KeyCode::Left => Some(Action::PluginManagerClose),
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        _ => None,
    }
}
