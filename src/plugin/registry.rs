//! Plugin discovery — scans directories and parses `manifest.toml` files.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;

use crate::plugin::PluginMetadata;

/// Errors that can occur when loading a plugin manifest.
#[derive(Debug, Error)]
pub enum RegistryError {
    /// Failed to read the manifest file from disk.
    #[error("failed to read manifest at {path}: {source}")]
    ManifestRead {
        /// Path to the manifest file that could not be read.
        path: PathBuf,
        /// The underlying I/O error.
        source: std::io::Error,
    },
    /// Failed to parse the manifest TOML.
    #[error("failed to parse manifest at {path}: {source}")]
    ManifestParse {
        /// Path to the manifest file that could not be parsed.
        path: PathBuf,
        /// The TOML parse error.
        source: toml::de::Error,
    },
    /// The entry script declared in the manifest does not exist.
    /// Kept for documentation; returned at execution time by [`crate::plugin::script::ScriptPlugin`].
    #[allow(dead_code)]
    #[error("entry script not found: {path}")]
    EntryNotFound {
        /// Path to the missing entry script.
        path: PathBuf,
    },
    /// Neither `entry` nor `[[commands]]` was declared in the manifest.
    #[error("manifest at {path} must declare either `entry` or `[[commands]]`")]
    MissingEntry {
        /// Path to the manifest file.
        path: PathBuf,
    },
}

#[derive(Deserialize)]
struct ManifestFile {
    plugin: ManifestPlugin,
    #[serde(default)]
    commands: Vec<ManifestCommand>,
}

#[derive(Deserialize)]
struct ManifestPlugin {
    name: String,
    description: String,
    version: String,
    author: String,
    icon: String,
    /// Entry script — required for single-command plugins; ignored when `[[commands]]` is declared.
    entry: Option<String>,
    timeout_seconds: Option<u64>,
    category: Option<String>,
    keybinding: Option<String>,
    streaming: Option<bool>,
    icon_nerd: Option<String>,
    prefetch: Option<bool>,
    cache: Option<bool>,
}

/// A single command within a multi-command plugin manifest.
#[derive(Deserialize)]
struct ManifestCommand {
    /// Command name shown in the unified list (e.g., "Recent Branches").
    name: String,
    /// One-line description (defaults to parent plugin description if absent).
    description: Option<String>,
    /// Entry script filename (relative to plugin directory).
    entry: String,
    /// Quick-launch key badge (e.g., `"gb"`).
    quickkey: Option<String>,
    timeout_seconds: Option<u64>,
    streaming: Option<bool>,
    prefetch: Option<bool>,
    cache: Option<bool>,
}

/// Which backend should execute this plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginKind {
    /// External process (shell script, Python, etc.).
    Script,
    /// Embedded Lua VM via mlua.
    Lua,
}

/// A plugin discovered on disk — metadata plus the location info needed to execute it.
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    /// Parsed plugin metadata.
    pub metadata: PluginMetadata,
    /// The directory containing the manifest and entry script.
    pub plugin_dir: PathBuf,
    /// The entry point filename (relative to `plugin_dir`).
    pub entry: String,
    /// Which backend should execute this plugin.
    pub kind: PluginKind,
}

fn kind_for_entry(entry: &str) -> PluginKind {
    if std::path::Path::new(entry)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("lua"))
    {
        PluginKind::Lua
    } else {
        PluginKind::Script
    }
}

/// Parse a single plugin directory's `manifest.toml`.
///
/// Returns one `DiscoveredPlugin` per declared command. Single-entry (legacy) manifests
/// return a `Vec` with exactly one element. Multi-command manifests return one element
/// per `[[commands]]` entry.
pub fn parse_manifest(plugin_dir: &Path) -> Result<Vec<DiscoveredPlugin>, RegistryError> {
    let manifest_path = plugin_dir.join("manifest.toml");
    let contents =
        std::fs::read_to_string(&manifest_path).map_err(|source| RegistryError::ManifestRead {
            path: manifest_path.clone(),
            source,
        })?;
    let manifest: ManifestFile =
        toml::from_str(&contents).map_err(|source| RegistryError::ManifestParse {
            path: manifest_path.clone(),
            source,
        })?;

    let p = manifest.plugin;
    let plugin_dir_buf = plugin_dir.to_path_buf();

    if manifest.commands.is_empty() {
        // ── Legacy single-entry path ──────────────────────────────────────────
        let entry = p.entry.ok_or(RegistryError::MissingEntry {
            path: manifest_path,
        })?;
        let kind = kind_for_entry(&entry);
        Ok(vec![DiscoveredPlugin {
            metadata: PluginMetadata {
                name: p.name,
                description: p.description,
                version: p.version,
                author: p.author,
                icon: p.icon,
                category: p.category,
                keybinding: p.keybinding,
                timeout: Duration::from_secs(p.timeout_seconds.unwrap_or(10)),
                streaming: p.streaming.unwrap_or(false),
                entry_path: None, // Set by the plugin backend constructors.
                icon_nerd: p.icon_nerd,
                prefetch: p.prefetch.unwrap_or(true),
                plugin_group: None,
                quickkey: None,
                cache: p.cache.unwrap_or(true),
            },
            plugin_dir: plugin_dir_buf,
            entry,
            kind,
        }])
    } else {
        // ── Multi-command path ────────────────────────────────────────────────
        if p.entry.is_some() {
            tracing::warn!(
                plugin = %p.name,
                "manifest declares both `entry` and `[[commands]]`; `entry` is ignored"
            );
        }
        let plugin_group = p.name.clone();
        let plugin_default_timeout = p.timeout_seconds.unwrap_or(10);
        let plugin_default_streaming = p.streaming.unwrap_or(false);
        let plugin_default_prefetch = p.prefetch.unwrap_or(false); // commands default lazy
        let plugin_default_cache = p.cache.unwrap_or(true);

        let discovered = manifest
            .commands
            .into_iter()
            .map(|cmd| {
                let kind = kind_for_entry(&cmd.entry);
                DiscoveredPlugin {
                    metadata: PluginMetadata {
                        name: cmd.name,
                        description: cmd.description.unwrap_or_else(|| p.description.clone()),
                        version: p.version.clone(),
                        author: p.author.clone(),
                        icon: p.icon.clone(),
                        category: p.category.clone(),
                        keybinding: None, // commands use quickkey instead
                        timeout: Duration::from_secs(
                            cmd.timeout_seconds.unwrap_or(plugin_default_timeout),
                        ),
                        streaming: cmd.streaming.unwrap_or(plugin_default_streaming),
                        entry_path: None,
                        icon_nerd: p.icon_nerd.clone(),
                        prefetch: cmd.prefetch.unwrap_or(plugin_default_prefetch),
                        plugin_group: Some(plugin_group.clone()),
                        quickkey: cmd.quickkey,
                        cache: cmd.cache.unwrap_or(plugin_default_cache),
                    },
                    plugin_dir: plugin_dir_buf.clone(),
                    entry: cmd.entry,
                    kind,
                }
            })
            .collect();
        Ok(discovered)
    }
}

/// Scan directories for plugin subdirectories containing `manifest.toml`.
///
/// Skips directories with missing or malformed manifests (logs a warning).
/// Returns plugins sorted alphabetically by name.
pub fn scan(dirs: &[PathBuf]) -> anyhow::Result<Vec<DiscoveredPlugin>> {
    let mut plugins = Vec::new();
    for dir in dirs {
        if !dir.exists() {
            tracing::debug!(path = %dir.display(), "plugin directory does not exist, skipping");
            continue;
        }
        let entries = std::fs::read_dir(dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            match parse_manifest(&path) {
                Ok(discovered) => {
                    for plugin in &discovered {
                        tracing::info!(name = %plugin.metadata.name, "discovered plugin");
                    }
                    plugins.extend(discovered);
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skipping plugin directory");
                }
            }
        }
    }
    plugins.sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));
    Ok(plugins)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hello_world_manifest() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/plugins/hello-world");
        let mut plugins = parse_manifest(&path).expect("parse failed");
        assert_eq!(plugins.len(), 1);
        let plugin = plugins.remove(0);
        assert_eq!(plugin.metadata.name, "Hello World");
        assert_eq!(plugin.entry, "run.sh");
        assert!(plugin.metadata.plugin_group.is_none());
        assert!(plugin.metadata.quickkey.is_none());
        assert!(plugin.metadata.cache);
    }

    #[test]
    fn scan_discovers_example_plugins() {
        let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/plugins");
        let plugins = scan(&[examples_dir]).expect("scan failed");
        let names: Vec<&str> = plugins.iter().map(|p| p.metadata.name.as_str()).collect();
        assert!(
            names.contains(&"Hello World"),
            "expected 'Hello World' in {names:?}"
        );
        assert!(
            names.contains(&"System Info"),
            "expected 'System Info' in {names:?}"
        );
    }

    #[test]
    fn scan_skips_missing_directory() {
        let nonexistent = PathBuf::from("/tmp/larkline-nonexistent-test-dir");
        let plugins = scan(&[nonexistent]).expect("scan should not fail on missing dir");
        assert!(plugins.is_empty());
    }

    #[test]
    fn scan_includes_plugin_with_missing_entry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugin_dir = dir.path().join("missing-entry-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("manifest.toml"),
            r#"
[plugin]
name = "Missing Entry"
description = "test"
version = "0.1.0"
author = "test"
icon = "T"
entry = "nonexistent.sh"
"#,
        )
        .unwrap();

        // Entry existence is not checked at scan time.
        let plugins = scan(&[dir.path().to_path_buf()]).expect("scan failed");
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].metadata.name, "Missing Entry");
    }

    #[test]
    fn parse_manifest_multi_command() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugin_dir = dir.path().join("git-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("manifest.toml"),
            r#"
[plugin]
name = "Git"
description = "Git operations"
version = "0.1.0"
author = "taylor"
icon = "🌿"

[[commands]]
name = "Recent Branches"
description = "Local branches"
entry = "branches.lua"
quickkey = "gb"

[[commands]]
name = "Status"
entry = "status.sh"
quickkey = "gs"
"#,
        )
        .unwrap();

        let plugins = parse_manifest(&plugin_dir).expect("parse failed");
        assert_eq!(plugins.len(), 2);

        let branches = &plugins[0];
        assert_eq!(branches.metadata.name, "Recent Branches");
        assert_eq!(branches.metadata.plugin_group.as_deref(), Some("Git"));
        assert_eq!(branches.metadata.quickkey.as_deref(), Some("gb"));
        assert_eq!(branches.entry, "branches.lua");
        assert_eq!(branches.kind, PluginKind::Lua);
        assert!(!branches.metadata.prefetch); // multi-command defaults to lazy

        let status = &plugins[1];
        assert_eq!(status.metadata.name, "Status");
        assert_eq!(status.metadata.plugin_group.as_deref(), Some("Git"));
        assert_eq!(status.metadata.quickkey.as_deref(), Some("gs"));
        // Description falls back to plugin description when absent.
        assert_eq!(status.metadata.description, "Git operations");
        assert_eq!(status.kind, PluginKind::Script);
    }

    #[test]
    fn parse_manifest_missing_entry_and_no_commands_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugin_dir = dir.path().join("bad-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("manifest.toml"),
            r#"
[plugin]
name = "Bad Plugin"
description = "no entry"
version = "0.1.0"
author = "test"
icon = "T"
"#,
        )
        .unwrap();

        let result = parse_manifest(&plugin_dir);
        assert!(matches!(result, Err(RegistryError::MissingEntry { .. })));
    }
}
