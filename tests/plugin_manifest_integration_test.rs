//! Integration tests for shipped example plugin manifests.

use std::fs;
use std::path::PathBuf;

use larkline::plugin::registry;

#[test]
fn shipped_plugin_manifests_parse_and_entries_exist() {
    let plugins_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/plugins");

    let mut plugin_dirs: Vec<PathBuf> = fs::read_dir(&plugins_root)
        .expect("failed to read examples/plugins")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        // Mirror `registry::scan`: skip `_`/`.`-prefixed dirs (e.g. `_shared/`).
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| !n.starts_with('_') && !n.starts_with('.'))
        })
        .collect();
    plugin_dirs.sort();

    assert!(
        !plugin_dirs.is_empty(),
        "expected shipped example plugins in {}",
        plugins_root.display()
    );

    for plugin_dir in plugin_dirs {
        let manifest_path = plugin_dir.join("manifest.toml");
        assert!(
            manifest_path.exists(),
            "missing manifest: {}",
            manifest_path.display()
        );

        let discovered = registry::parse_manifest(&plugin_dir).unwrap_or_else(|err| {
            panic!(
                "failed to parse manifest {}: {err}",
                manifest_path.display()
            )
        });

        assert!(
            !discovered.is_empty(),
            "manifest produced no commands: {}",
            manifest_path.display()
        );

        for plugin in discovered {
            let entry_path = plugin.plugin_dir.join(&plugin.entry);
            assert!(
                entry_path.exists(),
                "missing entry for {}: {}",
                manifest_path.display(),
                entry_path.display()
            );
            assert!(
                entry_path.is_file(),
                "entry is not a file for {}: {}",
                manifest_path.display(),
                entry_path.display()
            );
        }
    }
}
