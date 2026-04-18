//! Action handlers for dashboard widgets and the widget picker overlay.
//!
//! All functions take `&mut App` and mutate widget state / `pm_config` in place.
//! They are invoked from the main `handle_action` dispatch in [`crate::app`].

use crate::app::{App, VimMode, WidgetPickerEntry, WidgetPickerState};

pub fn widget_focus_up(app: &mut App) {
    if app.state.widgets_visible && !app.state.widget_indices.is_empty() {
        app.state.widget_focused = true;
        app.state.vim_mode = VimMode::Normal;
    }
}

pub fn widget_disable(app: &mut App) {
    if app.state.widget_focused {
        if let Some(&pidx) = app.state.widget_indices.get(app.state.widget_selected) {
            let meta = &app.state.plugins[pidx];
            let gk = meta
                .plugin_group
                .as_deref()
                .unwrap_or(&meta.name)
                .to_string();
            let name = meta.name.clone();
            app.pm_config.toggle_widget(&gk, &name);
            if let Err(e) = crate::config::save_plugin_manager_config(&app.pm_config) {
                tracing::warn!(error = %e, "failed to save widget config");
            }
            crate::widgets::rebuild_widget_indices(&mut app.state, &app.pm_config);
            app.state.status_message =
                Some((format!("Hidden widget: {name}"), std::time::Instant::now()));
        }
    }
}

pub fn widget_move_left(app: &mut App) {
    if app.state.widget_focused && app.state.widget_selected > 0 {
        crate::widgets::ensure_widget_order(&app.state, &mut app.pm_config);
        if let Some(&pidx) = app.state.widget_indices.get(app.state.widget_selected) {
            let meta = &app.state.plugins[pidx];
            let gk = meta.plugin_group.as_deref().unwrap_or(&meta.name);
            app.pm_config.move_widget_up(gk, &meta.name);
            if let Err(e) = crate::config::save_plugin_manager_config(&app.pm_config) {
                tracing::warn!(error = %e, "failed to save widget order");
            }
            app.state.widget_selected -= 1;
            crate::widgets::rebuild_widget_indices(&mut app.state, &app.pm_config);
        }
    }
}

pub fn widget_move_right(app: &mut App) {
    if app.state.widget_focused
        && app.state.widget_selected + 1 < app.state.widget_indices.len()
    {
        crate::widgets::ensure_widget_order(&app.state, &mut app.pm_config);
        if let Some(&pidx) = app.state.widget_indices.get(app.state.widget_selected) {
            let meta = &app.state.plugins[pidx];
            let gk = meta.plugin_group.as_deref().unwrap_or(&meta.name);
            app.pm_config.move_widget_down(gk, &meta.name);
            if let Err(e) = crate::config::save_plugin_manager_config(&app.pm_config) {
                tracing::warn!(error = %e, "failed to save widget order");
            }
            app.state.widget_selected += 1;
            crate::widgets::rebuild_widget_indices(&mut app.state, &app.pm_config);
        }
    }
}

pub fn widget_toggle_visibility(app: &mut App) {
    if !app.state.widget_indices.is_empty() {
        app.state.widgets_visible = !app.state.widgets_visible;
        app.state.widget_focused = false;
    }
}

pub fn widget_picker_open(app: &mut App) {
    let entries: Vec<WidgetPickerEntry> = app
        .state
        .plugins
        .iter()
        .filter(|m| m.widget)
        .map(|m| {
            let gk = m.plugin_group.as_deref().unwrap_or(&m.name);
            let key = format!("{gk}:{}", m.name);
            let label = if let Some(ref pg) = m.plugin_group {
                format!("{pg}: {}", m.name)
            } else {
                m.name.clone()
            };
            let enabled = !app.pm_config.is_widget_disabled(gk, &m.name);
            WidgetPickerEntry {
                label,
                icon: m.icon.clone(),
                key,
                enabled,
            }
        })
        .collect();

    if entries.is_empty() {
        app.state.status_message = Some((
            "No widget-eligible plugins found".to_string(),
            std::time::Instant::now(),
        ));
    } else {
        app.state.widget_picker = Some(WidgetPickerState {
            entries,
            selected: 0,
            query: String::new(),
            filtered_indices: Vec::new(),
        });
    }
}

pub fn widget_picker_close(app: &mut App) {
    app.state.widget_picker = None;
}

pub fn widget_picker_up(app: &mut App) {
    if let Some(ref mut picker) = app.state.widget_picker {
        if picker.selected > 0 {
            picker.selected -= 1;
        }
    }
}

pub fn widget_picker_down(app: &mut App) {
    if let Some(ref mut picker) = app.state.widget_picker {
        let count = picker.visible_entries().len();
        if picker.selected + 1 < count {
            picker.selected += 1;
        }
    }
}

pub fn widget_picker_toggle(app: &mut App) {
    if let Some(ref mut picker) = app.state.widget_picker {
        let actual_idx = if picker.query.is_empty() {
            picker.selected
        } else {
            picker
                .filtered_indices
                .get(picker.selected)
                .copied()
                .unwrap_or(picker.selected)
        };
        if let Some(entry) = picker.entries.get_mut(actual_idx) {
            if let Some((gk, cmd)) = entry.key.split_once(':') {
                app.pm_config.toggle_widget(gk, cmd);
                entry.enabled = !app.pm_config.is_widget_disabled(gk, cmd);
                if let Err(e) = crate::config::save_plugin_manager_config(&app.pm_config) {
                    tracing::warn!(error = %e, "failed to save widget config");
                }
                crate::widgets::rebuild_widget_indices(&mut app.state, &app.pm_config);
                app.state.widgets_visible = !app.state.widget_indices.is_empty();
            }
        }
    }
}

pub fn widget_picker_search(app: &mut App, c: char) {
    if let Some(ref mut picker) = app.state.widget_picker {
        picker.query.push(c);
        picker.rebuild_filter();
        picker.selected = 0;
    }
}

pub fn widget_picker_backspace(app: &mut App) {
    if let Some(ref mut picker) = app.state.widget_picker {
        picker.query.pop();
        picker.rebuild_filter();
        picker.selected = 0;
    }
}
