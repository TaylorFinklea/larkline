//! Semantic actions derived from raw keyboard input.

/// Application-level actions produced by the input handler.
///
/// Raw key events are mapped to `Action` variants by [`crate::input::InputHandler`].
/// The app state machine in [`crate::app::App`] processes actions to drive state transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Clear the search query entirely.
    ClearSearch,
    /// Go back / dismiss the current view.
    Back,
    /// Quit the application.
    Quit,
    /// Execute the default action on the selected output item.
    Execute,
}
