//! Mini app mode helpers — layout tree traversal and manipulation.

use crate::app::{AppState, MiniAppState, Mode, PaneState};
use crate::plugin::traits::{LayoutChild, MiniAppLayout, PaneContent, PaneId, SplitDirection};
use std::collections::HashMap;

/// Compute the depth-first leaf order of pane IDs in a layout tree.
///
/// This ordering is used for Tab/Shift+Tab focus cycling between panes.
#[allow(dead_code)]
pub fn pane_order(layout: &MiniAppLayout) -> Vec<PaneId> {
    let mut order = Vec::new();
    collect_pane_ids(layout, &mut order);
    order
}

fn collect_pane_ids(layout: &MiniAppLayout, out: &mut Vec<PaneId>) {
    match layout {
        MiniAppLayout::Pane { id, .. } => {
            out.push(id.clone());
        }
        MiniAppLayout::Split { children, .. } => {
            for child in children {
                collect_pane_ids(&child.layout, out);
            }
        }
    }
}

/// Build a `MiniAppState` from a `PluginOutput` that contains a layout.
///
/// Initializes per-pane state from the layout tree's content declarations.
#[allow(dead_code)]
pub fn build_mini_app_state(
    plugin_index: usize,
    layout: MiniAppLayout,
) -> MiniAppState {
    let order = pane_order(&layout);
    let focused = order.first().cloned().unwrap_or_default();

    let mut panes = HashMap::new();
    collect_pane_states(&layout, &mut panes);

    MiniAppState {
        plugin_index,
        layout,
        panes,
        focused_pane: focused,
        pane_order: order,
    }
}

fn collect_pane_states(layout: &MiniAppLayout, panes: &mut HashMap<PaneId, PaneState>) {
    match layout {
        MiniAppLayout::Pane { id, content } => {
            let output_mode = if !content.columns.is_empty() {
                crate::app::OutputMode::Table
            } else if content.raw_text.is_some() {
                if content.output_format.as_deref() == Some("markdown") {
                    crate::app::OutputMode::Markdown
                } else {
                    crate::app::OutputMode::RawText
                }
            } else {
                crate::app::OutputMode::List
            };
            panes.insert(
                id.clone(),
                PaneState {
                    content: content.clone(),
                    output_mode,
                    ..PaneState::default()
                },
            );
        }
        MiniAppLayout::Split { children, .. } => {
            for child in children {
                collect_pane_states(&child.layout, panes);
            }
        }
    }
}

/// Split the focused pane into two panes along the given direction.
///
/// The focused pane becomes the left/top child; a new empty pane is the right/bottom child.
/// Returns the ID of the new pane, or `None` if the focused pane wasn't found.
pub fn split_pane(
    state: &mut MiniAppState,
    direction: SplitDirection,
) -> Option<PaneId> {
    let focused_id = state.focused_pane.clone();
    let new_id = format!("{focused_id}_split_{}", state.panes.len());

    // Find the original pane content to preserve it.
    let original_content = state
        .panes
        .get(&focused_id)
        .map(|p| p.content.clone())
        .unwrap_or_default();

    let replacement = MiniAppLayout::Split {
        direction,
        children: vec![
            LayoutChild {
                size: 50,
                layout: MiniAppLayout::Pane {
                    id: focused_id.clone(),
                    content: original_content,
                },
            },
            LayoutChild {
                size: 50,
                layout: MiniAppLayout::Pane {
                    id: new_id.clone(),
                    content: PaneContent::default(),
                },
            },
        ],
    };

    if !replace_pane_in_tree(&mut state.layout, &focused_id, replacement) {
        return None;
    }

    state.panes.insert(new_id.clone(), PaneState::default());
    state.pane_order = pane_order(&state.layout);
    Some(new_id)
}

/// Close the focused pane. Its sibling takes the parent's place.
///
/// If the focused pane is the only pane, does nothing and returns `false`.
pub fn close_pane(state: &mut MiniAppState) -> bool {
    if state.pane_order.len() <= 1 {
        return false;
    }

    let focused_id = state.focused_pane.clone();
    if !remove_pane_from_tree(&mut state.layout, &focused_id) {
        return false;
    }

    state.panes.remove(&focused_id);
    state.pane_order = pane_order(&state.layout);
    // Move focus to the next available pane.
    state.focused_pane = state.pane_order.first().cloned().unwrap_or_default();
    true
}

/// Grow the focused pane by `amount` percentage points (shrinks siblings proportionally).
pub fn resize_pane(state: &mut MiniAppState, amount: i16) {
    adjust_size_in_tree(&mut state.layout, &state.focused_pane, amount);
}

// ---------------------------------------------------------------------------
// Tree mutation internals
// ---------------------------------------------------------------------------

/// Replace a pane node in the tree with a new layout.
/// Returns `true` if the pane was found and replaced.
#[allow(clippy::match_wildcard_for_single_variants)]
fn replace_pane_in_tree(
    layout: &mut MiniAppLayout,
    target_id: &str,
    replacement: MiniAppLayout,
) -> bool {
    match layout {
        MiniAppLayout::Pane { id, .. } if id == target_id => {
            *layout = replacement;
            true
        }
        MiniAppLayout::Split { children, .. } => {
            for child in children {
                if replace_pane_in_tree(&mut child.layout, target_id, replacement.clone()) {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

/// Remove a pane from the tree. Its sibling replaces the parent split.
/// Returns `true` if successful.
#[allow(clippy::cast_possible_truncation)]
fn remove_pane_from_tree(layout: &mut MiniAppLayout, target_id: &str) -> bool {
    match layout {
        MiniAppLayout::Split { children, .. } => {
            // Check if any direct child is the target pane.
            if let Some(pos) = children.iter().position(|c| {
                matches!(&c.layout, MiniAppLayout::Pane { id, .. } if id == target_id)
            }) {
                if children.len() == 2 {
                    // Replace the split with the surviving sibling.
                    let sibling_idx = 1 - pos;
                    let sibling = children.remove(sibling_idx);
                    *layout = sibling.layout;
                    return true;
                } else if children.len() > 2 {
                    // Redistribute removed child's size to remaining children.
                    let removed_size = children[pos].size;
                    children.remove(pos);
                    let extra = removed_size / children.len() as u16;
                    for child in children.iter_mut() {
                        child.size += extra;
                    }
                    return true;
                }
            }
            // Recurse into children.
            for child in children {
                if remove_pane_from_tree(&mut child.layout, target_id) {
                    return true;
                }
            }
            false
        }
        MiniAppLayout::Pane { .. } => false,
    }
}

/// Adjust the size of the child containing `target_id` within its parent split.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
fn adjust_size_in_tree(layout: &mut MiniAppLayout, target_id: &str, amount: i16) {
    if let MiniAppLayout::Split { children, .. } = layout {
        // Find which child contains the target.
        let target_idx = children.iter().position(|c| pane_ids_contain(&c.layout, target_id));
        if let Some(idx) = target_idx {
            let current = children[idx].size as i16;
            let new_size = (current + amount).clamp(10, 90) as u16;
            let delta = new_size as i16 - current;

            if delta != 0 && children.len() >= 2 {
                children[idx].size = new_size;
                // Distribute the delta evenly across siblings.
                let sibling_count = (children.len() - 1) as i16;
                let per_sibling = delta / sibling_count;
                for (i, child) in children.iter_mut().enumerate() {
                    if i != idx {
                        child.size = (child.size as i16 - per_sibling).clamp(10, 90) as u16;
                    }
                }
            }
        } else {
            // Recurse into children.
            for child in children {
                adjust_size_in_tree(&mut child.layout, target_id, amount);
            }
        }
    }
}

/// Check if a layout tree contains a pane with the given ID.
fn pane_ids_contain(layout: &MiniAppLayout, target_id: &str) -> bool {
    match layout {
        MiniAppLayout::Pane { id, .. } => id == target_id,
        MiniAppLayout::Split { children, .. } => {
            children.iter().any(|c| pane_ids_contain(&c.layout, target_id))
        }
    }
}

// ---------------------------------------------------------------------------
// Action handlers (invoked from handle_action in src/app.rs)
// ---------------------------------------------------------------------------

pub fn focus_next(state: &mut AppState) {
    if let Some(ref mut mini) = state.mini_app {
        if let Some(pos) = mini.pane_order.iter().position(|id| *id == mini.focused_pane) {
            let next = (pos + 1) % mini.pane_order.len();
            mini.focused_pane = mini.pane_order[next].clone();
        }
    }
}

pub fn focus_prev(state: &mut AppState) {
    if let Some(ref mut mini) = state.mini_app {
        if let Some(pos) = mini.pane_order.iter().position(|id| *id == mini.focused_pane) {
            let prev = if pos == 0 {
                mini.pane_order.len().saturating_sub(1)
            } else {
                pos - 1
            };
            mini.focused_pane = mini.pane_order[prev].clone();
        }
    }
}

pub fn close(state: &mut AppState) {
    state.mini_app = None;
    state.mode = Mode::Unified;
    state.viewing_plugin_index = None;
}

/// Expand current `ViewOutput` into a single-pane mini app.
pub fn expand(state: &mut AppState) {
    if state.mode == Mode::ViewOutput {
        if let Some(ref output) = state.plugin_output {
            if let Some(ref layout) = output.layout {
                let plugin_index = state.viewing_plugin_index.unwrap_or(0);
                state.mini_app = Some(build_mini_app_state(plugin_index, layout.clone()));
                state.mode = Mode::MiniApp;
            }
        }
    }
}

pub fn split_h(state: &mut AppState) {
    if let Some(ref mut mini) = state.mini_app {
        split_pane(mini, SplitDirection::Horizontal);
    }
}

pub fn split_v(state: &mut AppState) {
    if let Some(ref mut mini) = state.mini_app {
        split_pane(mini, SplitDirection::Vertical);
    }
}

pub fn close_focused_pane(state: &mut AppState) {
    if let Some(ref mut mini) = state.mini_app {
        close_pane(mini);
    }
}

pub fn resize_grow(state: &mut AppState) {
    if let Some(ref mut mini) = state.mini_app {
        resize_pane(mini, 5);
    }
}

pub fn resize_shrink(state: &mut AppState) {
    if let Some(ref mut mini) = state.mini_app {
        resize_pane(mini, -5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::traits::{LayoutChild, PaneContent, SplitDirection};

    fn pane(id: &str) -> MiniAppLayout {
        MiniAppLayout::Pane {
            id: id.to_string(),
            content: PaneContent {
                title: id.to_string(),
                ..PaneContent::default()
            },
        }
    }

    #[test]
    fn pane_order_single_pane() {
        let layout = pane("main");
        assert_eq!(pane_order(&layout), vec!["main"]);
    }

    #[test]
    fn pane_order_horizontal_split() {
        let layout = MiniAppLayout::Split {
            direction: SplitDirection::Horizontal,
            children: vec![
                LayoutChild {
                    size: 30,
                    layout: pane("left"),
                },
                LayoutChild {
                    size: 70,
                    layout: pane("right"),
                },
            ],
        };
        assert_eq!(pane_order(&layout), vec!["left", "right"]);
    }

    #[test]
    fn pane_order_nested_splits() {
        let layout = MiniAppLayout::Split {
            direction: SplitDirection::Horizontal,
            children: vec![
                LayoutChild {
                    size: 30,
                    layout: pane("nav"),
                },
                LayoutChild {
                    size: 70,
                    layout: MiniAppLayout::Split {
                        direction: SplitDirection::Vertical,
                        children: vec![
                            LayoutChild {
                                size: 60,
                                layout: pane("detail"),
                            },
                            LayoutChild {
                                size: 40,
                                layout: pane("actions"),
                            },
                        ],
                    },
                },
            ],
        };
        assert_eq!(pane_order(&layout), vec!["nav", "detail", "actions"]);
    }

    #[test]
    fn split_pane_creates_two_children() {
        let layout = pane("main");
        let mut state = build_mini_app_state(0, layout);
        assert_eq!(state.pane_order.len(), 1);

        let new_id = split_pane(&mut state, SplitDirection::Horizontal);
        assert!(new_id.is_some());
        assert_eq!(state.pane_order.len(), 2);
        assert_eq!(state.pane_order[0], "main");
        assert!(state.panes.contains_key(&new_id.unwrap()));
    }

    #[test]
    fn close_pane_removes_and_promotes_sibling() {
        let layout = MiniAppLayout::Split {
            direction: SplitDirection::Horizontal,
            children: vec![
                LayoutChild {
                    size: 50,
                    layout: pane("left"),
                },
                LayoutChild {
                    size: 50,
                    layout: pane("right"),
                },
            ],
        };
        let mut state = build_mini_app_state(0, layout);
        state.focused_pane = "left".to_string();

        assert!(close_pane(&mut state));
        assert_eq!(state.pane_order, vec!["right"]);
        assert!(!state.panes.contains_key("left"));
        assert_eq!(state.focused_pane, "right");
    }

    #[test]
    fn close_pane_single_pane_does_nothing() {
        let layout = pane("only");
        let mut state = build_mini_app_state(0, layout);
        assert!(!close_pane(&mut state));
        assert_eq!(state.pane_order.len(), 1);
    }

    #[test]
    fn resize_pane_adjusts_sizes() {
        let layout = MiniAppLayout::Split {
            direction: SplitDirection::Horizontal,
            children: vec![
                LayoutChild {
                    size: 50,
                    layout: pane("left"),
                },
                LayoutChild {
                    size: 50,
                    layout: pane("right"),
                },
            ],
        };
        let mut state = build_mini_app_state(0, layout);
        state.focused_pane = "left".to_string();

        resize_pane(&mut state, 10);
        if let MiniAppLayout::Split { children, .. } = &state.layout {
            assert_eq!(children[0].size, 60);
            assert_eq!(children[1].size, 40);
        } else {
            panic!("expected split");
        }
    }

    #[test]
    fn build_state_initializes_panes() {
        let layout = MiniAppLayout::Split {
            direction: SplitDirection::Horizontal,
            children: vec![
                LayoutChild {
                    size: 50,
                    layout: pane("left"),
                },
                LayoutChild {
                    size: 50,
                    layout: pane("right"),
                },
            ],
        };
        let state = build_mini_app_state(0, layout);
        assert_eq!(state.focused_pane, "left");
        assert_eq!(state.pane_order, vec!["left", "right"]);
        assert!(state.panes.contains_key("left"));
        assert!(state.panes.contains_key("right"));
    }
}
