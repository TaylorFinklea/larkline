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
    /// Copy the selected item's label to the system clipboard.
    CopyLabel,
    /// Open the copy menu overlay (label / detail / JSON / URL).
    CopyMenu,
    /// Select the highlighted entry in the copy menu.
    CopyMenuSelect,
    /// Dismiss the copy menu without copying.
    CopyMenuDismiss,
    /// Enter output search mode (triggered by `/` in `ViewOutput`).
    OutputEnterSearch,
    /// Append a character to the output search query.
    OutputSearch(char),
    /// Delete the last character from the output search query.
    OutputBackspaceSearch,
    /// Clear output search and exit search mode.
    OutputClearSearch,
    /// Open the selected item's URL in the system browser.
    OpenUrl,
    /// Move focus to the next form field.
    FormNextField,
    /// Move focus to the previous form field.
    FormPrevField,
    /// Append a character to the focused text field.
    FormInput(char),
    /// Delete the character before the cursor in the focused text field.
    FormBackspace,
    /// Move cursor left in a text field.
    FormCursorLeft,
    /// Move cursor right in a text field.
    FormCursorRight,
    /// Cycle a Select field to the next option.
    FormSelectNext,
    /// Cycle a Select field to the previous option.
    FormSelectPrev,
    /// Toggle a Toggle field or cycle a Select field.
    FormToggle,
    /// Submit the form and re-execute the plugin with collected values.
    FormSubmit,
    /// Cancel the form and go back.
    FormCancel,
}
