//! Mini app mode helpers — layout tree traversal and manipulation.

use crate::app::{MiniAppState, PaneState};
use crate::plugin::traits::{MiniAppLayout, PaneId};
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
