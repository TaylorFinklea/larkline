//! Builder for the Plugin Manager overlay state.
//!
//! Row building is a pure function of a [`PmSnapshot`] — the gathered scan,
//! `.env` secrets, and keychain-presence lookups. Gathering the snapshot is
//! the expensive part (fs scan + one `security` subprocess per declared
//! secret) and runs off the key path; expand/toggle keypresses only rebuild
//! rows from the cached snapshot.

use std::collections::{HashMap, HashSet};

use crate::app::{PluginManagerRow, PluginManagerState, SecretSource};
use crate::config::PluginManagerConfig;
use crate::plugin::registry;
use crate::plugin::traits::PluginMetadata;

/// Inputs for plugin-manager row building, gathered once (off the key path
/// on the async path) instead of per keypress.
#[derive(Debug)]
pub struct PmSnapshot {
    /// ALL discovered plugins (unfiltered), so disabled ones still appear.
    pub all_meta: Vec<PluginMetadata>,
    /// Secrets present in `~/.config/larkline/.env`.
    pub env_secrets: HashMap<String, String>,
    /// Keychain presence per declared secret key (`security` subprocess
    /// each — the reason this is gathered up front, not per keypress).
    pub keychain: HashMap<String, bool>,
}

/// Gather the full snapshot. Blocking — run via `spawn_blocking` on the
/// async path (`security` subprocess per declared secret).
pub fn scan_snapshot(
    plugin_dirs: &[std::path::PathBuf],
    active_plugins: &[PluginMetadata],
) -> PmSnapshot {
    // Scan ALL plugins (unfiltered) so disabled ones still appear as [ ] in the manager.
    let all_meta: Vec<PluginMetadata> = match registry::scan(plugin_dirs) {
        Ok(discovered) => discovered.iter().map(|d| d.metadata.clone()).collect(),
        Err(_) => active_plugins.to_vec(), // fallback to active set
    };
    let env_secrets = crate::config::load_secrets();
    let mut keychain = HashMap::new();
    for meta in &all_meta {
        for key in &meta.secrets {
            keychain
                .entry(key.clone())
                .or_insert_with(|| crate::config::keychain_has(key));
        }
    }
    PmSnapshot {
        all_meta,
        env_secrets,
        keychain,
    }
}

/// Instant snapshot from the active plugin set — shown while the full
/// background scan runs. Keychain presence is unknown (empty map), so
/// keychain-only secrets briefly show as `NotSet` until the scan lands.
pub fn fallback_snapshot(active_plugins: &[PluginMetadata]) -> PmSnapshot {
    PmSnapshot {
        all_meta: active_plugins.to_vec(),
        env_secrets: crate::config::load_secrets(),
        keychain: HashMap::new(),
    }
}

/// Build plugin manager state with no groups expanded.
pub fn build(snapshot: &PmSnapshot, pm_config: &PluginManagerConfig) -> PluginManagerState {
    build_with_expanded(snapshot, pm_config, &HashSet::new())
}

/// Build plugin manager state with the given groups expanded.
///
/// Pure row building — no registry scan, no keychain subprocesses. The only
/// I/O is a per-expanded-group `PluginStore` read so settings values stay
/// fresh after an edit.
#[allow(clippy::type_complexity, clippy::too_many_lines)]
pub fn build_with_expanded(
    snapshot: &PmSnapshot,
    pm_config: &PluginManagerConfig,
    expanded_keys: &HashSet<String>,
) -> PluginManagerState {
    // Collect unique plugin groups from metadata.
    let mut seen_groups: Vec<String> = Vec::new();
    let mut group_meta: std::collections::HashMap<
        String,
        (
            String,
            String,
            String,
            String,
            Vec<(String, Option<String>)>,
            Vec<crate::plugin::traits::FormField>,
            Vec<String>,
        ),
    > = std::collections::HashMap::new();

    for meta in &snapshot.all_meta {
        let gk = meta
            .plugin_group
            .as_deref()
            .unwrap_or(&meta.name)
            .to_string();
        let entry = group_meta.entry(gk.clone()).or_insert_with(|| {
            seen_groups.push(gk.clone());
            (
                meta.icon.clone(),
                meta.category.clone().unwrap_or_default(),
                meta.version.clone(),
                gk.clone(),
                Vec::new(),
                meta.settings_spec.clone(),
                meta.secrets.clone(),
            )
        });
        entry.4.push((meta.name.clone(), meta.quickkey.clone()));
    }

    let mut rows = Vec::new();
    for gk in &seen_groups {
        let (icon, cat, ver, _display, commands, settings, secrets) = &group_meta[gk];
        let is_expanded = expanded_keys.contains(gk);
        let plugin_enabled = !pm_config.is_plugin_disabled(gk);

        rows.push(PluginManagerRow::PluginHeader {
            group_key: gk.clone(),
            name: gk.clone(),
            icon: icon.clone(),
            category: cat.clone(),
            version: ver.clone(),
            enabled: plugin_enabled,
            expanded: is_expanded,
            command_count: commands.len(),
        });

        if is_expanded {
            // Command rows.
            for (cmd_name, qk) in commands {
                let cmd_enabled = plugin_enabled && !pm_config.is_command_disabled(gk, cmd_name);
                rows.push(PluginManagerRow::Command {
                    group_key: gk.clone(),
                    name: cmd_name.clone(),
                    quickkey: qk.clone(),
                    enabled: cmd_enabled,
                });
            }
            // Setting rows.
            let store_path = crate::plugin::store::store_path_for(gk, None);
            let store = crate::plugin::store::PluginStore::load(store_path);
            for spec in settings {
                let value = store
                    .get(&spec.id)
                    .and_then(|v| v.as_str().map(str::to_string))
                    .or_else(|| spec.default_value.clone())
                    .unwrap_or_else(|| "(not set)".to_string());
                rows.push(PluginManagerRow::Setting {
                    group_key: gk.clone(),
                    id: spec.id.clone(),
                    label: spec.label.clone(),
                    value,
                });
            }
            // Secret rows — presence resolved from the snapshot, never a
            // subprocess on the key path.
            for key in secrets {
                let source = if snapshot.env_secrets.contains_key(key) {
                    SecretSource::DotEnv
                } else if std::env::var(key).is_ok() {
                    SecretSource::EnvVar
                } else if snapshot.keychain.get(key).copied().unwrap_or(false) {
                    SecretSource::Keychain
                } else {
                    SecretSource::NotSet
                };
                rows.push(PluginManagerRow::Secret {
                    key: key.clone(),
                    source,
                });
            }
        }
    }

    PluginManagerState {
        rows,
        selected: 0,
        expanded: expanded_keys.clone(),
    }
}
