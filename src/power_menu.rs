//! Builds the power menu categories shown in the which-key-style overlay.
//!
//! The menu adapts to the current [`Mode`] and, in `Unified`, to whether a
//! widget card is focused.

use crate::action::Action;
use crate::app::{AppState, Mode, PowerMenuCategory, PowerMenuItem};

/// Build the list of power menu categories for the current app state.
#[allow(clippy::too_many_lines)]
pub fn build_power_menu_categories(state: &AppState) -> Vec<PowerMenuCategory> {
    match state.mode {
        Mode::Unified if state.widget_focused => {
            // Widget-focused context menu.
            vec![
                PowerMenuCategory {
                    name: "Widget".to_string(),
                    items: vec![
                        PowerMenuItem {
                            key: 'h',
                            key_hint: "h/l".to_string(),
                            label: "Navigate cards".to_string(),
                            action: Action::Back,
                        },
                        PowerMenuItem {
                            key: 'j',
                            key_hint: "j".to_string(),
                            label: "Back to list".to_string(),
                            action: Action::MoveDown,
                        },
                        PowerMenuItem {
                            key: 'H',
                            key_hint: "H".to_string(),
                            label: "Move card left".to_string(),
                            action: Action::WidgetMoveLeft,
                        },
                        PowerMenuItem {
                            key: 'L',
                            key_hint: "L".to_string(),
                            label: "Move card right".to_string(),
                            action: Action::WidgetMoveRight,
                        },
                        PowerMenuItem {
                            key: 'D',
                            key_hint: "D".to_string(),
                            label: "Hide this widget".to_string(),
                            action: Action::WidgetDisable,
                        },
                        PowerMenuItem {
                            key: 'W',
                            key_hint: "W".to_string(),
                            label: "Hide all widgets".to_string(),
                            action: Action::WidgetToggleVisibility,
                        },
                    ],
                },
                PowerMenuCategory {
                    name: "App".to_string(),
                    items: {
                        let mut items = vec![PowerMenuItem {
                            key: 'P',
                            key_hint: "P".to_string(),
                            label: "Plugins".to_string(),
                            action: Action::PluginManagerOpen,
                        }];
                        if state.update_hint.is_some() {
                            items.push(PowerMenuItem {
                                key: 'U',
                                key_hint: "U".to_string(),
                                label: "Upgrade lark".to_string(),
                                action: Action::RunUpgrade,
                            });
                        }
                        items.push(PowerMenuItem {
                            key: 'q',
                            key_hint: "q".to_string(),
                            label: "Quit".to_string(),
                            action: Action::Quit,
                        });
                        items
                    },
                },
            ]
        }
        Mode::Unified => {
            let mut widget_items = vec![
                PowerMenuItem {
                    key: 'K',
                    key_hint: "K".to_string(),
                    label: "Focus widgets".to_string(),
                    action: Action::WidgetFocusUp,
                },
                PowerMenuItem {
                    key: 'W',
                    key_hint: "W".to_string(),
                    label: if state.widgets_visible {
                        "Hide widgets".to_string()
                    } else {
                        "Show widgets".to_string()
                    },
                    action: Action::WidgetToggleVisibility,
                },
            ];
            // Only show widget items if there are widgets.
            if state.widget_indices.is_empty() {
                widget_items.clear();
            }

            vec![
                PowerMenuCategory {
                    name: "Navigation".to_string(),
                    items: vec![
                        PowerMenuItem {
                            key: '/',
                            key_hint: "/".to_string(),
                            label: "Search".to_string(),
                            action: Action::EnterInsertMode,
                        },
                        PowerMenuItem {
                            key: ':',
                            key_hint: ":".to_string(),
                            label: "Command".to_string(),
                            action: Action::EnterCommandMode,
                        },
                    ],
                },
                PowerMenuCategory {
                    name: "Display".to_string(),
                    items: {
                        let mut items = vec![
                            PowerMenuItem {
                                key: 'd',
                                key_hint: "d".to_string(),
                                label: "Descriptions".to_string(),
                                action: Action::ToggleDescriptions,
                            },
                            PowerMenuItem {
                                key: 'O',
                                key_hint: "O".to_string(),
                                label: format!("Sort: {}", state.sort_mode.next().label()),
                                action: Action::CycleSort,
                            },
                            PowerMenuItem {
                                key: 'R',
                                key_hint: "R".to_string(),
                                label: "Refresh".to_string(),
                                action: Action::RefreshPlugins,
                            },
                            PowerMenuItem {
                                key: 's',
                                key_hint: "s".to_string(),
                                label: "Sidebar".to_string(),
                                action: Action::ToggleSidebar,
                            },
                            PowerMenuItem {
                                key: 'T',
                                key_hint: "T".to_string(),
                                label: "Theme".to_string(),
                                action: Action::ThemePickerOpen,
                            },
                        ];
                        items.extend(widget_items);
                        items
                    },
                },
                PowerMenuCategory {
                    name: "App".to_string(),
                    items: {
                        let mut items = vec![PowerMenuItem {
                            key: 'P',
                            key_hint: "P".to_string(),
                            label: "Plugins".to_string(),
                            action: Action::PluginManagerOpen,
                        }];
                        if state.update_hint.is_some() {
                            items.push(PowerMenuItem {
                                key: 'U',
                                key_hint: "U".to_string(),
                                label: "Upgrade lark".to_string(),
                                action: Action::RunUpgrade,
                            });
                        }
                        items.push(PowerMenuItem {
                            key: 'q',
                            key_hint: "q".to_string(),
                            label: "Quit".to_string(),
                            action: Action::Quit,
                        });
                        items
                    },
                },
            ]
        }
        Mode::PluginManager => vec![
            PowerMenuCategory {
                name: "Plugin Manager".to_string(),
                items: vec![
                    PowerMenuItem {
                        key: ' ',
                        key_hint: "SPC".to_string(),
                        label: "Toggle enable".to_string(),
                        action: Action::PluginManagerToggle,
                    },
                    PowerMenuItem {
                        key: '\n',
                        key_hint: "⏎".to_string(),
                        label: "Expand/collapse".to_string(),
                        action: Action::PluginManagerExpand,
                    },
                    PowerMenuItem {
                        key: 's',
                        key_hint: "s".to_string(),
                        label: "Set secret".to_string(),
                        action: Action::PluginManagerSetSecret,
                    },
                    PowerMenuItem {
                        key: 'x',
                        key_hint: "x".to_string(),
                        label: "Delete secret".to_string(),
                        action: Action::PluginManagerDeleteSecret,
                    },
                ],
            },
            PowerMenuCategory {
                name: "App".to_string(),
                items: vec![PowerMenuItem {
                    key: 'q',
                    key_hint: "q".to_string(),
                    label: "Back".to_string(),
                    action: Action::PluginManagerClose,
                }],
            },
        ],
        Mode::ViewOutput => {
            let has_settings = state
                .viewing_plugin_index
                .and_then(|i| state.plugins.get(i))
                .is_some_and(|p| !p.settings_spec.is_empty());

            // "This item" category lists the focused item's actions inline
            // with digit keys (1-9). Discoverable without remembering `:`
            // to open the searchable palette. Falls through to lowercase
            // letters if the row carries more than 9 actions.
            let item_category = crate::app_output::selected_output_item(state).and_then(|item| {
                if item.actions.is_empty() {
                    return None;
                }
                let items = item
                    .actions
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, action)| {
                        // 1-9, then a-z for overflow.
                        let key = if idx < 9 {
                            char::from_digit(u32::try_from(idx + 1).ok()?, 10)?
                        } else if idx - 9 < 26 {
                            let off = u8::try_from(idx - 9).ok()?;
                            (b'a' + off) as char
                        } else {
                            return None;
                        };
                        Some(PowerMenuItem {
                            key,
                            key_hint: key.to_string(),
                            label: action.label.clone(),
                            action: Action::RunFocusedItemAt(idx),
                        })
                    })
                    .collect::<Vec<_>>();
                Some(PowerMenuCategory {
                    name: "This item".to_string(),
                    items,
                })
            });

            let actions_category = {
                let mut action_items = vec![
                    PowerMenuItem {
                        key: ':',
                        key_hint: ":".to_string(),
                        label: "Palette (search)".to_string(),
                        action: Action::PaletteOpen,
                    },
                    PowerMenuItem {
                        key: 'o',
                        key_hint: "o".to_string(),
                        label: "Open URL".to_string(),
                        action: Action::OpenUrl,
                    },
                    PowerMenuItem {
                        key: 'y',
                        key_hint: "y".to_string(),
                        label: "Copy".to_string(),
                        action: Action::CopyLabel,
                    },
                    PowerMenuItem {
                        key: 'Y',
                        key_hint: "Y".to_string(),
                        label: "Copy Menu".to_string(),
                        action: Action::CopyMenu,
                    },
                ];
                if has_settings {
                    action_items.push(PowerMenuItem {
                        key: 'S',
                        key_hint: "S".to_string(),
                        label: "Settings".to_string(),
                        action: Action::OpenSettings,
                    });
                }
                PowerMenuCategory {
                    name: "Actions".to_string(),
                    items: action_items,
                }
            };

            let mut cats: Vec<PowerMenuCategory> = Vec::new();
            if let Some(c) = item_category {
                cats.push(c);
            }
            cats.push(actions_category);
            cats.extend(vec![
                PowerMenuCategory {
                    name: "Display".to_string(),
                    items: vec![
                        PowerMenuItem {
                            key: 't',
                            key_hint: "t".to_string(),
                            label: "Toggle View".to_string(),
                            action: Action::ToggleOutputMode,
                        },
                        PowerMenuItem {
                            key: '/',
                            key_hint: "/".to_string(),
                            label: "Search".to_string(),
                            action: Action::OutputEnterSearch,
                        },
                        PowerMenuItem {
                            key: 's',
                            key_hint: "s".to_string(),
                            label: "Sidebar".to_string(),
                            action: Action::ToggleSidebar,
                        },
                        PowerMenuItem {
                            key: 'T',
                            key_hint: "T".to_string(),
                            label: "Theme".to_string(),
                            action: Action::ThemePickerOpen,
                        },
                    ],
                },
                PowerMenuCategory {
                    name: "App".to_string(),
                    items: vec![
                        PowerMenuItem {
                            key: 'r',
                            key_hint: "r".to_string(),
                            label: "Rerun".to_string(),
                            action: Action::RerunCommand,
                        },
                        PowerMenuItem {
                            key: 'd',
                            key_hint: "d".to_string(),
                            label: "Descriptions".to_string(),
                            action: Action::ToggleDescriptions,
                        },
                        PowerMenuItem {
                            key: 'q',
                            key_hint: "q".to_string(),
                            label: "Quit".to_string(),
                            action: Action::Quit,
                        },
                    ],
                },
            ]);
            cats
        }
        Mode::MiniApp => vec![], // TODO(Phase D): mini app power menu
    }
}
