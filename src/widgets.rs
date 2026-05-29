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
    if state.widget_indices.is_empty() {
        // Don't leave focus on a row that no longer exists.
        state.widget_focused = false;
    }
    if state.widget_selected >= state.widget_indices.len() {
        state.widget_selected = 0;
    }
}

/// Rebuild the list of glance-strip status plugin indices. Mirrors
/// [`rebuild_widget_indices`] but for the compact status strip (uses
/// `disabled_status`, discovery order — no separate ordering in v1.0).
pub fn rebuild_status_indices(state: &mut AppState, pm_config: &PluginManagerConfig) {
    let indices: Vec<usize> = state
        .plugins
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            m.status && {
                let gk = m.plugin_group.as_deref().unwrap_or(&m.name);
                !pm_config.is_status_disabled(gk, &m.name)
            }
        })
        .map(|(i, _)| i)
        .collect();

    state.status_indices = indices;
    state.status_visible = !state.status_indices.is_empty();
    if state.status_indices.is_empty() {
        // Don't strand focus on a strip that no longer renders.
        state.status_focused = false;
    }
    if state.status_selected >= state.status_indices.len() {
        state.status_selected = 0;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::config::PluginManagerConfig;

    #[test]
    fn rebuild_status_indices_clears_focus_when_strip_empties() {
        // No plugins → no status items; a lingering focus must be dropped so
        // the input layer doesn't capture h/l/Enter against an invisible strip.
        let mut state = AppState {
            status_focused: true,
            status_visible: true,
            ..Default::default()
        };
        rebuild_status_indices(&mut state, &PluginManagerConfig::default());
        assert!(state.status_indices.is_empty());
        assert!(!state.status_visible);
        assert!(
            !state.status_focused,
            "focus must clear when the strip empties"
        );
    }
}
