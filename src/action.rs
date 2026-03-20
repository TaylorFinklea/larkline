//! Semantic actions derived from raw keyboard input.

/// Application-level actions produced by the input handler.
///
/// Raw key events are mapped to `Action` variants by [`crate::input`].
/// The app state machine in [`crate::app::App`] processes actions to drive state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Move selection up one item.
    MoveUp,
    /// Move selection down one item.
    MoveDown,
    /// Confirm the current selection.
    Select,
    /// Append a character to the active search query.
    Search(char),
    /// Delete the last character from the search query.
    BackspaceSearch,
    /// Go back / dismiss the current view.
    Back,
    /// Quit the application.
    Quit,
    /// Execute the default action on the selected output item.
    Execute,
    /// Directly launch a plugin by name.
    LaunchPlugin(String),
    /// Scroll the output pane down by half a page.
    ScrollHalfPageDown,
    /// Scroll the output pane up by half a page.
    ScrollHalfPageUp,
    /// Toggle between list and raw-text output view.
    ToggleOutputMode,
    /// Re-scan plugin directories and reload the plugin list.
    RefreshPlugins,
    /// Switch to Vim Normal mode (navigation, j/k/q active).
    EnterNormalMode,
    /// Switch to Vim Insert mode (quickkeys and search active).
    EnterInsertMode,
    /// Switch to Vim Command mode (`:command` input).
    EnterCommandMode,
    /// Append a character to the command-mode input buffer.
    CommandChar(char),
    /// Delete the last character from the command-mode input buffer.
    CommandBackspace,
    /// Submit the current command-mode input.
    CommandSubmit,
    /// User confirmed the pending shell action.
    Confirm,
    /// User cancelled the pending shell action.
    Cancel,
}
