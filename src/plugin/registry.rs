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
    #[error("entry script not found: {path}")]
    EntryNotFound {
        /// Path to the missing entry script.
        path: PathBuf,
    },
}

#[derive(Deserialize)]
struct ManifestFile {
    plugin: ManifestPlugin,
}

#[derive(Deserialize)]
struct ManifestPlugin {
    name: String,
    description: String,
    version: String,
    author: String,
    icon: String,
    entry: String,
    timeout_seconds: Option<u64>,
    category: Option<String>,
    keybinding: Option<String>,
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
}

/// Parse a single plugin directory's `manifest.toml`.
pub fn parse_manifest(plugin_dir: &Path) -> Result<DiscoveredPlugin, RegistryError> {
    let manifest_path = plugin_dir.join("manifest.toml");
    let contents =
        std::fs::read_to_string(&manifest_path).map_err(|source| RegistryError::ManifestRead {
            path: manifest_path.clone(),
            source,
        })?;
    let manifest: ManifestFile =
        toml::from_str(&contents).map_err(|source| RegistryError::ManifestParse {
            path: manifest_path,
            source,
        })?;

    let entry_path = plugin_dir.join(&manifest.plugin.entry);
    if !entry_path.exists() {
        return Err(RegistryError::EntryNotFound { path: entry_path });
    }

    let p = manifest.plugin;
    Ok(DiscoveredPlugin {
        metadata: PluginMetadata {
            name: p.name,
            description: p.description,
            version: p.version,
            author: p.author,
            icon: p.icon,
            category: p.category,
            keybinding: p.keybinding,
            timeout: Duration::from_secs(p.timeout_seconds.unwrap_or(10)),
        },
        plugin_dir: plugin_dir.to_path_buf(),
        entry: p.entry,
    })
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
                Ok(plugin) => {
                    tracing::info!(name = %plugin.metadata.name, "discovered plugin");
                    plugins.push(plugin);
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
        let plugin = parse_manifest(&path).expect("parse failed");
        assert_eq!(plugin.metadata.name, "Hello World");
        assert_eq!(plugin.entry, "run.sh");
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
}
