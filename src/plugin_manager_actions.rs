//! Action handlers for the Plugin Manager overlay.

use crate::action::Action;
use crate::app::{App, Mode, PluginManagerRow, SecretSource, VimMode};

pub fn open(app: &mut App) {
    app.state.power_menu = None;
    app.state.mode = Mode::PluginManager;
    app.state.vim_mode = VimMode::Normal;
    // widget_focused is a Unified-mode concept. If a widget card was focused
    // when the manager was opened, leaving the flag set makes the MoveUp/Down
    // guards swallow j/k navigation in the manager.
    app.state.widget_focused = false;
    app.state.plugin_manager = Some(crate::plugin_manager_state::build(
        &app.plugin_dirs,
        &app.state.plugins,
        &app.pm_config,
    ));
}

pub fn close(app: &mut App) {
    app.state.plugin_manager = None;
    app.state.mode = Mode::Unified;
    // Trigger full refresh so disabled plugins are filtered out.
    app.handle_action(Action::RefreshPlugins);
}

pub fn toggle(app: &mut App) {
    let Some(ref mut pm) = app.state.plugin_manager else {
        return;
    };
    let changed = match pm.rows.get(pm.selected).cloned() {
        Some(PluginManagerRow::PluginHeader { group_key, .. }) => {
            app.pm_config.toggle_plugin(&group_key);
            true
        }
        Some(PluginManagerRow::Command {
            group_key, name, ..
        }) => {
            app.pm_config.toggle_command(&group_key, &name);
            true
        }
        _ => false,
    };
    if changed {
        if let Err(e) = crate::config::save_plugin_manager_config(&app.pm_config) {
            tracing::warn!(error = %e, "failed to save plugin manager config");
        }
        app.state.plugin_manager = Some(crate::plugin_manager_state::build(
            &app.plugin_dirs,
            &app.state.plugins,
            &app.pm_config,
        ));
        if let Some(ref mut pm) = app.state.plugin_manager {
            pm.selected = pm.selected.min(pm.rows.len().saturating_sub(1));
        }
    }
}

pub fn expand(app: &mut App) {
    let Some(ref mut pm) = app.state.plugin_manager else {
        return;
    };
    let Some(PluginManagerRow::PluginHeader { group_key, .. }) = pm.rows.get(pm.selected) else {
        return;
    };
    let key = group_key.clone();
    if pm.expanded.contains(&key) {
        pm.expanded.remove(&key);
    } else {
        pm.expanded.insert(key);
    }
    let expanded = pm.expanded.clone();
    let sel = pm.selected;
    let mut new_pm = crate::plugin_manager_state::build_with_expanded(
        &app.plugin_dirs,
        &app.state.plugins,
        &app.pm_config,
        &expanded,
    );
    new_pm.selected = sel.min(new_pm.rows.len().saturating_sub(1));
    new_pm.expanded = expanded;
    app.state.plugin_manager = Some(new_pm);
}

pub fn set_secret(app: &mut App) {
    let Some(ref pm) = app.state.plugin_manager else {
        return;
    };
    if let Some(PluginManagerRow::Secret { key, .. }) = pm.rows.get(pm.selected) {
        app.state.status_message = Some((
            format!("Run: lark secret set {key}"),
            std::time::Instant::now(),
        ));
    }
}

pub fn delete_secret(app: &mut App) {
    let Some(ref pm) = app.state.plugin_manager else {
        return;
    };
    if let Some(PluginManagerRow::Secret { key, source, .. }) = pm.rows.get(pm.selected) {
        if *source != SecretSource::NotSet {
            let key = key.clone();
            let _ = std::process::Command::new("security")
                .args(["delete-generic-password", "-s", &key])
                .stderr(std::process::Stdio::null())
                .status();
            app.state.plugin_manager = Some(crate::plugin_manager_state::build(
                &app.plugin_dirs,
                &app.state.plugins,
                &app.pm_config,
            ));
            app.state.status_message = Some((format!("Deleted {key}"), std::time::Instant::now()));
        }
    }
}
