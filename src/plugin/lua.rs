//! Lua plugin backend — executes a Lua script in a sandboxed embedded VM.

use std::path::PathBuf;
use std::time::Duration;

use mlua::prelude::*;

use crate::plugin::registry::DiscoveredPlugin;
use crate::plugin::traits::{Plugin, PluginError, PluginMetadata, PluginOutput};

/// A plugin that runs Lua code in an embedded VM with access to the `lark.*` host API.
///
/// Each call to [`execute()`](Plugin::execute) creates a fresh Lua VM — no state leaks between runs.
pub struct LuaPlugin {
    metadata: PluginMetadata,
    script_path: PathBuf,
    #[allow(dead_code)] // Reserved for lark.exec working directory context.
    plugin_dir: PathBuf,
}

impl LuaPlugin {
    /// Create a `LuaPlugin` from a [`DiscoveredPlugin`].
    #[must_use]
    pub fn from_discovered(mut discovered: DiscoveredPlugin) -> Self {
        let script_path = discovered.plugin_dir.join(&discovered.entry);
        discovered.metadata.entry_path = Some(script_path.clone());
        Self {
            metadata: discovered.metadata,
            script_path,
            plugin_dir: discovered.plugin_dir,
        }
    }

    /// Create a sandboxed Lua VM with only safe standard libraries.
    fn create_vm() -> Result<Lua, PluginError> {
        let lua = Lua::new_with(
            LuaStdLib::COROUTINE
                | LuaStdLib::TABLE
                | LuaStdLib::STRING
                | LuaStdLib::MATH
                | LuaStdLib::UTF8,
            LuaOptions::default(),
        )
        .map_err(|e| PluginError::ExecutionFailed(format!("failed to create Lua VM: {e}")))?;

        // 32 MB memory limit to prevent runaway plugins.
        let _ = lua.set_memory_limit(32 * 1024 * 1024);

        Ok(lua)
    }

    /// Register the `lark.*` host API on the given Lua VM.
    #[allow(clippy::too_many_lines)]
    fn register_api(
        lua: &Lua,
        plugin_name: String,
        store: std::sync::Arc<std::sync::Mutex<crate::plugin::store::PluginStore>>,
    ) -> Result<(), PluginError> {
        let lark = lua
            .create_table()
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // lark.env(name) -> string? — read from .env secrets first, then process env.
        let env_fn = lua
            .create_function(|_, key: String| {
                // Check .env secrets first (via task-local), then fall back to process env.
                let from_secrets = crate::plugin::engine::SECRETS
                    .try_with(|s| s.get(&key).cloned())
                    .ok()
                    .flatten();
                Ok(from_secrets.or_else(|| std::env::var(&key).ok()))
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        lark.set("env", env_fn)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // lark.log(msg) — log at info level with the plugin name.
        let name_for_log = plugin_name;
        let log_fn = lua
            .create_function(move |_, msg: String| {
                tracing::info!(plugin = %name_for_log, "{msg}");
                Ok(())
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        lark.set("log", log_fn)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // lark.exec(cmd, args?) -> string — run a command, return stdout.
        // Uses tokio::process::Command with explicit args (no shell interpolation).
        let exec_fn = lua
            .create_async_function(|_, (cmd, args): (String, Option<Vec<String>>)| async move {
                let mut command = tokio::process::Command::new(&cmd);
                if let Some(ref args) = args {
                    command.args(args);
                }
                let output = command.output().await.map_err(LuaError::external)?;
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                Ok(stdout)
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        lark.set("exec", exec_fn)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // lark.json — encode/decode sub-table.
        let json_table = lua
            .create_table()
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        let encode_fn = lua
            .create_function(|lua, value: LuaValue| {
                let json_value: serde_json::Value =
                    lua.from_value(value).map_err(LuaError::external)?;
                serde_json::to_string(&json_value).map_err(LuaError::external)
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        json_table
            .set("encode", encode_fn)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        let decode_fn = lua
            .create_function(|lua, s: String| {
                let json_value: serde_json::Value =
                    serde_json::from_str(&s).map_err(LuaError::external)?;
                lua.to_value(&json_value).map_err(LuaError::external)
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        json_table
            .set("decode", decode_fn)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        lark.set("json", json_table)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // lark.http — get/post sub-table.
        let http_table = lua
            .create_table()
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        let http_get = lua
            .create_async_function(|lua, (url, opts): (String, Option<LuaTable>)| async move {
                let client = reqwest::Client::builder()
                    .user_agent(concat!("larkline/", env!("CARGO_PKG_VERSION")))
                    .build()
                    .map_err(LuaError::external)?;
                let mut req = client.get(&url);

                if let Some(ref opts) = opts {
                    if let Ok(headers) = opts.get::<LuaTable>("headers") {
                        for pair in headers.pairs::<String, String>() {
                            let (k, v) = pair?;
                            req = req.header(&k, &v);
                        }
                    }
                    if let Ok(timeout_secs) = opts.get::<f64>("timeout") {
                        req = req.timeout(Duration::from_secs_f64(timeout_secs));
                    }
                }

                let resp = req.send().await.map_err(LuaError::external)?;
                let status = resp.status().as_u16();
                let body = resp.text().await.map_err(LuaError::external)?;

                let result = lua.create_table()?;
                result.set("status", status)?;
                result.set("body", body)?;
                Ok(result)
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        http_table
            .set("get", http_get)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        let http_post = lua
            .create_async_function(
                |lua, (url, body, opts): (String, String, Option<LuaTable>)| async move {
                    let client = reqwest::Client::builder()
                        .user_agent(concat!("larkline/", env!("CARGO_PKG_VERSION")))
                        .build()
                        .map_err(LuaError::external)?;
                    let mut req = client.post(&url).body(body);

                    if let Some(ref opts) = opts {
                        if let Ok(headers) = opts.get::<LuaTable>("headers") {
                            for pair in headers.pairs::<String, String>() {
                                let (k, v) = pair?;
                                req = req.header(&k, &v);
                            }
                        }
                        if let Ok(timeout_secs) = opts.get::<f64>("timeout") {
                            req = req.timeout(Duration::from_secs_f64(timeout_secs));
                        }
                    }

                    let resp = req.send().await.map_err(LuaError::external)?;
                    let status = resp.status().as_u16();
                    let resp_body = resp.text().await.map_err(LuaError::external)?;

                    let result = lua.create_table()?;
                    result.set("status", status)?;
                    result.set("body", resp_body)?;
                    Ok(result)
                },
            )
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        http_table
            .set("post", http_post)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        lark.set("http", http_table)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // lark.store — persistent key-value storage sub-table.
        let store_table = lua
            .create_table()
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        let store_for_get = store.clone();
        let store_get = lua
            .create_function(move |lua, key: String| {
                let guard = store_for_get.lock().expect("store lock");
                match guard.get(&key) {
                    Some(val) => lua.to_value(val).map_err(LuaError::external),
                    None => Ok(LuaValue::Nil),
                }
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        store_table
            .set("get", store_get)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        let store_for_set = store.clone();
        let store_set = lua
            .create_function(move |lua, (key, value): (String, LuaValue)| {
                let json_value: serde_json::Value =
                    lua.from_value(value).map_err(LuaError::external)?;
                let mut guard = store_for_set.lock().expect("store lock");
                guard.set(key, json_value).map_err(LuaError::external)?;
                Ok(())
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        store_table
            .set("set", store_set)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        let store_for_delete = store.clone();
        let store_delete = lua
            .create_function(move |_, key: String| {
                let mut guard = store_for_delete.lock().expect("store lock");
                Ok(guard.delete(&key))
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        store_table
            .set("delete", store_delete)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        let store_for_keys = store;
        let store_keys = lua
            .create_function(move |lua, ()| {
                let guard = store_for_keys.lock().expect("store lock");
                let keys = guard.keys();
                let table = lua.create_sequence_from(keys)?;
                Ok(table)
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        store_table
            .set("keys", store_keys)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        lark.set("store", store_table)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // lark.invoke(name) -> table — invoke another plugin by name.
        let invoke_fn = lua
            .create_async_function(|lua, name: String| async move {
                use crate::plugin::engine::{INVOKE_DEPTH, PLUGIN_LIST};

                let depth = INVOKE_DEPTH.try_with(|d| *d).unwrap_or(0);
                if depth >= 5 {
                    return Err(LuaError::external(
                        "lark.invoke: max recursion depth (5) exceeded",
                    ));
                }

                let plugins = PLUGIN_LIST.try_with(Clone::clone).map_err(|_| {
                    LuaError::external("lark.invoke: plugin list not available in this context")
                })?;

                let plugin = plugins
                    .iter()
                    .find(|p| p.metadata().name == name)
                    .ok_or_else(|| {
                        LuaError::external(format!("lark.invoke: plugin '{name}' not found"))
                    })?
                    .clone();

                let output = PLUGIN_LIST
                    .scope(
                        plugins,
                        INVOKE_DEPTH.scope(depth + 1, async move { plugin.execute().await }),
                    )
                    .await
                    .map_err(LuaError::external)?;

                lua.to_value(&output).map_err(LuaError::external)
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        lark.set("invoke", invoke_fn)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // lark.register(config) — store the plugin config in a named registry slot.
        let register_fn = lua
            .create_function(|lua, config: LuaTable| {
                lua.set_named_registry_value("_lark_plugin_config", config)?;
                Ok(())
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        lark.set("register", register_fn)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // lark.clipboard_read() -> string — read current system clipboard text.
        let clipboard_read_fn = lua
            .create_function(|_, ()| {
                let text = arboard::Clipboard::new()
                    .and_then(|mut cb| cb.get_text())
                    .unwrap_or_default();
                Ok(text)
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        lark.set("clipboard_read", clipboard_read_fn)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // Install `lark` as a global.
        lua.globals()
            .set("lark", lark)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl Plugin for LuaPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn execute(&self) -> Result<PluginOutput, PluginError> {
        self.execute_inner(None).await
    }

    async fn execute_with_form(
        &self,
        form_values: std::collections::HashMap<String, String>,
    ) -> Result<PluginOutput, PluginError> {
        self.execute_inner(Some(form_values)).await
    }

    async fn execute_action(
        &self,
        callback_id: &str,
        context: &str,
    ) -> Result<PluginOutput, PluginError> {
        self.execute_action_inner(callback_id, context).await
    }
}

impl LuaPlugin {
    /// Shared execution logic for `execute()` and `execute_with_form()`.
    async fn execute_inner(
        &self,
        form_values: Option<std::collections::HashMap<String, String>>,
    ) -> Result<PluginOutput, PluginError> {
        if !self.script_path.exists() {
            return Err(PluginError::ExecutionFailed(format!(
                "Lua script not found: {}",
                self.script_path.display()
            )));
        }

        let script = std::fs::read_to_string(&self.script_path)
            .map_err(|e| PluginError::ExecutionFailed(format!("failed to read Lua script: {e}")))?;

        let plugin_name = self.metadata.name.clone();
        let plugin_name_for_save = self.metadata.name.clone();
        let timeout = self.metadata.timeout;

        // Load the plugin's persistent store.
        let store_path = crate::plugin::store::store_path_for(
            &self.metadata.name,
            self.metadata.plugin_group.as_deref(),
        );
        let store = std::sync::Arc::new(std::sync::Mutex::new(
            crate::plugin::store::PluginStore::load(store_path),
        ));
        let store_for_save = store.clone();

        // Run the entire Lua execution inside a timeout.
        tokio::time::timeout(timeout, async move {
            let lua = Self::create_vm()?;
            Self::register_api(&lua, plugin_name, store)?;

            // Inject form values as lark.form_values table (if present).
            if let Some(values) = form_values {
                if !values.is_empty() {
                    let lark: LuaTable = lua
                        .globals()
                        .get("lark")
                        .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
                    let fv_table = lua
                        .create_table()
                        .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
                    for (key, value) in &values {
                        fv_table
                            .set(key.as_str(), value.as_str())
                            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
                    }
                    lark.set("form_values", fv_table)
                        .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
                }
            }

            // Load the plugin script (defines on_run via lark.register).
            lua.load(&script)
                .exec()
                .map_err(|e| PluginError::InvalidOutput(format!("Lua syntax/load error: {e}")))?;

            // Retrieve the registered config table.
            let config: LuaTable =
                lua.named_registry_value("_lark_plugin_config")
                    .map_err(|_| {
                        PluginError::InvalidOutput(
                            "plugin did not call lark.register()".to_string(),
                        )
                    })?;

            // Get the on_run function.
            let on_run: LuaFunction = config.get("on_run").map_err(|_| {
                PluginError::InvalidOutput(
                    "lark.register() config missing 'on_run' function".to_string(),
                )
            })?;

            // Call on_run as an async thread (supports lark.http/lark.exec async calls).
            let thread = lua
                .create_thread(on_run)
                .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
            let run_result: Result<LuaValue, _> = thread
                .into_async::<LuaValue>(())
                .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?
                .await
                .map_err(|e| PluginError::ExecutionFailed(format!("on_run error: {e}")));

            // Always save the store, even if on_run failed.
            if let Err(e) = store_for_save.lock().expect("store lock").save() {
                tracing::warn!(plugin = %plugin_name_for_save, error = %e, "failed to save plugin store");
            }

            let result = run_result?;

            // Deserialize the returned table into PluginOutput.
            let output: PluginOutput = lua
                .from_value(result)
                .map_err(|e| PluginError::InvalidOutput(format!("invalid plugin output: {e}")))?;

            Ok(output)
        })
        .await
        .map_err(|_| PluginError::Timeout(timeout))?
    }

    /// Execute the plugin's `on_action` callback.
    ///
    /// Creates a fresh Lua VM, loads the script, and calls `on_action(callback_id, context)`.
    /// Returns the resulting `PluginOutput` which replaces the current view.
    async fn execute_action_inner(
        &self,
        callback_id: &str,
        context: &str,
    ) -> Result<PluginOutput, PluginError> {
        if !self.script_path.exists() {
            return Err(PluginError::ExecutionFailed(format!(
                "Lua script not found: {}",
                self.script_path.display()
            )));
        }

        let script = std::fs::read_to_string(&self.script_path)
            .map_err(|e| PluginError::ExecutionFailed(format!("failed to read Lua script: {e}")))?;

        let plugin_name = self.metadata.name.clone();
        let plugin_name_for_save = self.metadata.name.clone();
        let timeout = self.metadata.timeout;
        let callback_id = callback_id.to_string();
        let context = context.to_string();

        let store_path = crate::plugin::store::store_path_for(
            &self.metadata.name,
            self.metadata.plugin_group.as_deref(),
        );
        let store = std::sync::Arc::new(std::sync::Mutex::new(
            crate::plugin::store::PluginStore::load(store_path),
        ));
        let store_for_save = store.clone();

        tokio::time::timeout(timeout, async move {
            let lua = Self::create_vm()?;
            Self::register_api(&lua, plugin_name, store)?;

            // Load the plugin script.
            lua.load(&script)
                .exec()
                .map_err(|e| PluginError::InvalidOutput(format!("Lua syntax/load error: {e}")))?;

            // Retrieve the registered config table.
            let config: LuaTable =
                lua.named_registry_value("_lark_plugin_config")
                    .map_err(|_| {
                        PluginError::InvalidOutput(
                            "plugin did not call lark.register()".to_string(),
                        )
                    })?;

            // Get the on_action function.
            let on_action: LuaFunction = config.get("on_action").map_err(|_| {
                PluginError::ActionNotSupported {
                    action_id: callback_id.clone(),
                }
            })?;

            // Call on_action(callback_id, context).
            let thread = lua
                .create_thread(on_action)
                .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
            let run_result: Result<LuaValue, _> = thread
                .into_async::<LuaValue>((callback_id.as_str(), context.as_str()))
                .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?
                .await
                .map_err(|e| PluginError::ExecutionFailed(format!("on_action error: {e}")));

            // Always save the store.
            if let Err(e) = store_for_save.lock().expect("store lock").save() {
                tracing::warn!(plugin = %plugin_name_for_save, error = %e, "failed to save plugin store");
            }

            let result = run_result?;

            let output: PluginOutput = lua
                .from_value(result)
                .map_err(|e| PluginError::InvalidOutput(format!("invalid action output: {e}")))?;

            Ok(output)
        })
        .await
        .map_err(|_| PluginError::Timeout(timeout))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lua_plugin_from_source(name: &str, script: &str) -> LuaPlugin {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugin_dir = dir.path().join(name);
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("init.lua"), script).unwrap();
        std::fs::write(
            plugin_dir.join("manifest.toml"),
            format!(
                r#"
[plugin]
name = "{name}"
description = "test"
version = "0.1.0"
author = "test"
icon = "T"
entry = "init.lua"
timeout_seconds = 5
"#
            ),
        )
        .unwrap();

        let discovered = crate::plugin::registry::parse_manifest(&plugin_dir)
            .unwrap()
            .remove(0);
        // Keep the tempdir alive by leaking it (test only).
        std::mem::forget(dir);
        LuaPlugin::from_discovered(discovered)
    }

    #[tokio::test]
    async fn executes_hardcoded_lua_plugin() {
        let plugin = lua_plugin_from_source(
            "hello-lua",
            r#"
lark.register({
    on_run = function()
        return {
            title = "Hello from Lua",
            items = {
                { label = "Greeting", detail = "world", icon = "L" },
            }
        }
    end
})
"#,
        );
        let output = plugin.execute().await.expect("execution failed");
        assert_eq!(output.title, "Hello from Lua");
        assert_eq!(output.items.len(), 1);
        assert_eq!(output.items[0].label, "Greeting");
    }

    #[tokio::test]
    async fn lark_env_reads_environment() {
        // Use PATH which is always set on all platforms.
        let plugin = lua_plugin_from_source(
            "env-test",
            r#"
lark.register({
    on_run = function()
        local val = lark.env("PATH") or "missing"
        return {
            title = val,
            items = {}
        }
    end
})
"#,
        );
        let output = plugin.execute().await.expect("execution failed");
        assert!(!output.title.is_empty());
        assert_ne!(output.title, "missing");
    }

    #[tokio::test]
    async fn lark_json_roundtrip() {
        let plugin = lua_plugin_from_source(
            "json-test",
            r#"
lark.register({
    on_run = function()
        local encoded = lark.json.encode({ key = "value" })
        local decoded = lark.json.decode(encoded)
        return {
            title = decoded.key,
            items = {}
        }
    end
})
"#,
        );
        let output = plugin.execute().await.expect("execution failed");
        assert_eq!(output.title, "value");
    }

    #[tokio::test]
    async fn missing_register_returns_error() {
        let plugin = lua_plugin_from_source(
            "no-register",
            "-- Plugin doesn't call lark.register()\nlocal x = 1 + 1\n",
        );
        let result = plugin.execute().await;
        assert!(
            matches!(result, Err(PluginError::InvalidOutput(_))),
            "expected InvalidOutput error, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn syntax_error_returns_invalid_output() {
        let plugin = lua_plugin_from_source("syntax-err", "this is not valid lua!!!\n");
        let result = plugin.execute().await;
        assert!(
            matches!(result, Err(PluginError::InvalidOutput(_))),
            "expected InvalidOutput error, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn runtime_error_returns_execution_failed() {
        let plugin = lua_plugin_from_source(
            "runtime-err",
            r"
lark.register({
    on_run = function()
        local x = nil
        x()
    end
})
",
        );
        let result = plugin.execute().await;
        assert!(
            matches!(result, Err(PluginError::ExecutionFailed(_))),
            "expected ExecutionFailed error, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn lark_exec_runs_command() {
        let plugin = lua_plugin_from_source(
            "exec-test",
            r#"
lark.register({
    on_run = function()
        local output = lark.exec("echo", {"hello from exec"})
        return {
            title = output:match("^(.-)%s*$"),
            items = {}
        }
    end
})
"#,
        );
        let output = plugin.execute().await.expect("execution failed");
        assert_eq!(output.title, "hello from exec");
    }
}
