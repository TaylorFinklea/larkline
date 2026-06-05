//! Widget state helpers: ordering, indexing, and preview sync for dashboard widgets.

use crate::app::{AppState, CachedResult, UnifiedRow};
use crate::config::PluginManagerConfig;

/// Whether a plugin's cached result is degraded — a hard execution error, or a
/// successful result the plugin flagged `level = "warn"`/`"error"` (e.g. a CLI
/// not installed). Loading / absent / healthy results are not degraded.
#[must_use]
pub fn is_degraded(state: &AppState, plugin_index: usize) -> bool {
    match state.result_cache.get(&plugin_index) {
        Some(CachedResult::Error(_)) => true,
        Some(CachedResult::Ready(o) | CachedResult::Revalidating(o)) => {
            matches!(o.level.as_deref(), Some("warn" | "error"))
        }
        _ => false,
    }
}

/// Rebuild [`AppState::glance_indices`] — the strip's displayed/focusable set:
/// every status chip, plus any widget currently degraded (demoted from its
/// card). Degraded widgets that are also status chips appear once. Clears
/// focus when the strip empties; otherwise keeps the cursor on the SAME plugin
/// across the rebuild (so focus doesn't silently jump to a different chip when
/// a degraded widget heals and leaves the strip), clamping if it's gone.
pub fn rebuild_glance_indices(state: &mut AppState) {
    let focused_plugin = state.glance_indices.get(state.status_selected).copied();

    let mut items = state.status_indices.clone();
    for &w in &state.widget_indices {
        if !items.contains(&w) && is_degraded(state, w) {
            items.push(w);
        }
    }
    state.glance_indices = items;

    if state.glance_indices.is_empty() {
        state.status_focused = false;
        state.status_selected = 0;
    } else if let Some(pidx) = focused_plugin {
        // Follow the plugin to its new slot; if it left the strip, clamp.
        state.status_selected = state
            .glance_indices
            .iter()
            .position(|&i| i == pidx)
            .unwrap_or_else(|| state.status_selected.min(state.glance_indices.len() - 1));
    } else if state.status_selected >= state.glance_indices.len() {
        state.status_selected = 0;
    }
}

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
    fn rebuild_glance_indices_demotes_degraded_widget() {
        use crate::app::CachedResult;
        use crate::plugin::traits::PluginOutput;

        let mut state = AppState {
            widget_indices: vec![3],
            ..Default::default()
        };
        // A widget flagged level=warn (e.g. "Docker not installed") demotes.
        state.result_cache.insert(
            3,
            CachedResult::Ready(PluginOutput {
                title: "Containers".into(),
                level: Some("warn".into()),
                ..Default::default()
            }),
        );
        rebuild_glance_indices(&mut state);
        assert_eq!(state.glance_indices, vec![3], "degraded widget -> strip");

        // A healthy widget stays a card (not in the strip).
        state.result_cache.insert(
            3,
            CachedResult::Ready(PluginOutput {
                title: "Containers".into(),
                ..Default::default()
            }),
        );
        rebuild_glance_indices(&mut state);
        assert!(
            state.glance_indices.is_empty(),
            "healthy widget stays a card"
        );
    }

    #[test]
    fn rebuild_glance_indices_focus_follows_plugin() {
        use crate::app::CachedResult;
        use crate::plugin::traits::PluginOutput;

        let warn = || {
            CachedResult::Ready(PluginOutput {
                title: "x".into(),
                level: Some("warn".into()),
                ..Default::default()
            })
        };
        let mut state = AppState {
            widget_indices: vec![3, 4],
            status_focused: true,
            ..Default::default()
        };
        state.result_cache.insert(3, warn());
        state.result_cache.insert(4, warn());
        rebuild_glance_indices(&mut state);
        assert_eq!(state.glance_indices, vec![3, 4]);

        // Focus the second chip (plugin 4), then heal plugin 3 so the strip
        // collapses to [4] and plugin 4 shifts from slot 1 to slot 0.
        state.status_selected = 1;
        state.result_cache.insert(
            3,
            CachedResult::Ready(PluginOutput {
                title: "x".into(),
                ..Default::default()
            }),
        );
        rebuild_glance_indices(&mut state);
        assert_eq!(state.glance_indices, vec![4]);
        assert_eq!(
            state.status_selected, 0,
            "focus follows plugin 4 to its new slot, not the old index"
        );
    }

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
