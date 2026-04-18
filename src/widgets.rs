//! Widget state helpers: ordering, indexing, and preview sync for dashboard widgets.

use crate::app::{AppState, UnifiedRow};
use crate::config::PluginManagerConfig;

/// Ensure `widget_order` contains all current widget keys (for reordering).
pub fn ensure_widget_order(state: &AppState, pm_config: &mut PluginManagerConfig) {
    let current: Vec<String> = state
        .widget_indices
        .iter()
        .map(|&i| {
            let m = &state.plugins[i];
            format!(
                "{}:{}",
                m.plugin_group.as_deref().unwrap_or(&m.name),
                m.name
            )
        })
        .collect();
    for key in &current {
        if !pm_config.widget_order.contains(key) {
            pm_config.widget_order.push(key.clone());
        }
    }
    pm_config.widget_order.retain(|k| current.contains(k));
}

/// Rebuild the list of widget plugin indices.
pub fn rebuild_widget_indices(state: &mut AppState, pm_config: &PluginManagerConfig) {
    let mut indices: Vec<usize> = state
        .plugins
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            m.widget && {
                let gk = m.plugin_group.as_deref().unwrap_or(&m.name);
                !pm_config.is_widget_disabled(gk, &m.name)
            }
        })
        .map(|(i, _)| i)
        .collect();

    if !pm_config.widget_order.is_empty() {
        let order = &pm_config.widget_order;
        indices.sort_by_key(|&i| {
            let m = &state.plugins[i];
            let key = format!(
                "{}:{}",
                m.plugin_group.as_deref().unwrap_or(&m.name),
                m.name
            );
            order.iter().position(|k| k == &key).unwrap_or(usize::MAX)
        });
    }

    state.widget_indices = indices;
    state.widgets_visible = !state.widget_indices.is_empty();
    if state.widget_selected >= state.widget_indices.len() {
        state.widget_selected = 0;
    }
}

/// Update `preview_plugin_index` to match the currently selected unified row.
pub fn sync_preview_index(state: &mut AppState) {
    state.preview_plugin_index = state
        .unified_rows
        .get(state.unified_selected)
        .map(|r| match r {
            UnifiedRow::Command { plugin_index, .. } => *plugin_index,
        });
}
