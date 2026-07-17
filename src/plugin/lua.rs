//! Lua plugin backend — executes a Lua script in a sandboxed embedded VM.

use std::path::PathBuf;
use std::time::Duration;

use mlua::prelude::*;

use crate::plugin::registry::DiscoveredPlugin;
use crate::plugin::traits::{Plugin, PluginError, PluginMetadata, PluginOutput};

/// Serialize JSON / Rust values into Lua while mapping `null`/`None` to Lua `nil`
/// rather than the mlua null-sentinel userdata. This is the behavior plugins
/// expect — idioms like `x and x ~= ""` break when `x` is a truthy userdata.
fn null_safe_ser_options() -> mlua::serde::ser::Options {
    mlua::serde::ser::Options::new()
        .serialize_none_to_null(false)
        .serialize_unit_to_null(false)
}

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
        plugin_dir: &std::path::Path,
        store: std::sync::Arc<std::sync::Mutex<crate::plugin::store::PluginStore>>,
    ) -> Result<(), PluginError> {
        let lark = lua
            .create_table()
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // lark.plugin_dir — absolute path to the plugin's source directory.
        // Lets a plugin invoke sibling helper scripts (mail/mail_render.py etc).
        lark.set("plugin_dir", plugin_dir.to_string_lossy().into_owned())
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
        //
        // Spawn failure (binary not on PATH) returns an empty string
        // rather than raising a Lua error. Plugins universally guard with
        // `if not raw or raw == "" then <graceful error item>`, so a
        // missing CLI degrades cleanly instead of crashing the plugin.
        // (Before this, `lark.exec("kubectl", ...)` with no kubectl
        // installed errored out before the plugin's own guard could run.)
        // The richer `lark.exec_io` still surfaces spawn errors via its
        // exit_code for callers that need to distinguish them.
        let exec_fn = lua
            .create_async_function(|_, (cmd, args): (String, Option<Vec<String>>)| async move {
                let mut command = tokio::process::Command::new(&cmd);
                // Reap the child if the enclosing plugin-timeout future is dropped,
                // so a hung subprocess is not orphaned.
                command.kill_on_drop(true);
                if let Some(ref args) = args {
                    command.args(args);
                }
                match command.output().await {
                    Ok(output) => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
                    Err(e) => {
                        tracing::debug!(cmd = %cmd, error = %e, "lark.exec spawn failed; returning empty string");
                        Ok(String::new())
                    }
                }
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        lark.set("exec", exec_fn)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // lark.exec_io(cmd, args?, opts?) -> { stdout, stderr, exit_code }
        //
        // Richer subprocess primitive than lark.exec. Adds:
        //   - opts.stdin: string piped to the subprocess's stdin (used by the
        //     calendar plugin to send JSON requests to larkline-macos-helper)
        //   - opts.env: table of extra environment variables for the child —
        //     the secret-safe channel for tokens (argv is visible in `ps`;
        //     env is not). Used by bitwarden to pass BW_SESSION.
        //   - stderr in the return table (activates the v0.15.0 from_exit
        //     translator across shell plugins when they migrate)
        //   - exit_code (handlers can branch on non-zero without parsing stderr)
        //
        // Both stdout and stderr are returned as UTF-8 strings (lossy decode).
        // Exit code defaults to -1 if the process was killed by a signal.
        let exec_io_fn = lua
            .create_async_function(
                |lua, (cmd, args, opts): (String, Option<Vec<String>>, Option<mlua::Table>)| async move {
                    use tokio::io::AsyncWriteExt;
                    let mut command = tokio::process::Command::new(&cmd);
                    // Reap the child if the enclosing plugin-timeout future is dropped.
                    command.kill_on_drop(true);
                    if let Some(ref args) = args {
                        command.args(args);
                    }
                    let stdin_payload: Option<String> = match opts {
                        Some(ref t) => t.get("stdin").ok(),
                        None => None,
                    };
                    if let Some(ref t) = opts {
                        if let Ok(env) = t.get::<mlua::Table>("env") {
                            for pair in env.pairs::<String, String>() {
                                // Skip malformed entries (non-string key/value)
                                // rather than failing the whole call.
                                let Ok((k, v)) = pair else { continue };
                                command.env(k, v);
                            }
                        }
                    }
                    if stdin_payload.is_some() {
                        command.stdin(std::process::Stdio::piped());
                    }
                    command.stdout(std::process::Stdio::piped());
                    command.stderr(std::process::Stdio::piped());

                    // Spawn failure (binary not on PATH) is surfaced via the
                    // result table — stderr carries the OS message ("No such
                    // file or directory", which from_exit translates to a
                    // "<cli> not found" item) and exit_code = 127 (the shell
                    // convention for command-not-found) — rather than raising a
                    // Lua error. This matches lark.exec's graceful degradation
                    // and this fn's documented contract above; a plugin's own
                    // `if res.exit_code ~= 0` guard then handles it.
                    let mut child = match command.spawn() {
                        Ok(child) => child,
                        Err(e) => {
                            let result = lua.create_table()?;
                            result.set("stdout", "")?;
                            result.set("stderr", format!("{cmd}: {e}"))?;
                            result.set("exit_code", 127)?;
                            return Ok(result);
                        }
                    };
                    // Take stdin out before moving `child` into wait_with_output so the
                    // write runs CONCURRENTLY with draining stdout/stderr. Writing all of
                    // stdin first and only then reading output deadlocks when the child
                    // fills its stdout pipe buffer (~64KB) while we're still blocked
                    // writing its stdin — the classic pipe-buffer deadlock.
                    let mut stdin_handle = child.stdin.take();
                    let write_stdin = async {
                        if let (Some(payload), Some(mut stdin)) =
                            (stdin_payload, stdin_handle.take())
                        {
                            if let Err(e) = stdin.write_all(payload.as_bytes()).await {
                                // The child may close stdin / exit before consuming
                                // everything (e.g. `head`); a broken pipe is a normal
                                // end-of-write, not a failure.
                                if e.kind() != std::io::ErrorKind::BrokenPipe {
                                    return Err(e);
                                }
                            }
                            // Drop closes stdin so the child sees EOF.
                            drop(stdin);
                        }
                        Ok::<(), std::io::Error>(())
                    };
                    let (write_res, output_res) =
                        tokio::join!(write_stdin, child.wait_with_output());

                    // Failure to collect the child's output is surfaced via the
                    // result table (exit_code -1), not raised — same graceful
                    // contract as the spawn-failure path above.
                    let output = match output_res {
                        Ok(o) => o,
                        Err(e) => {
                            let result = lua.create_table()?;
                            result.set("stdout", "")?;
                            result.set("stderr", format!("{cmd}: {e}"))?;
                            result.set("exit_code", -1)?;
                            return Ok(result);
                        }
                    };
                    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                    let mut stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                    let exit_code = output.status.code().unwrap_or(-1);

                    // A non-BrokenPipe stdin write error is reported in stderr
                    // rather than raised (BrokenPipe is already treated as a
                    // normal early-EOF inside write_stdin), so plugins can branch
                    // on it via the result table like any other failure.
                    if let Err(e) = write_res {
                        use std::fmt::Write as _;
                        if !stderr.is_empty() {
                            stderr.push('\n');
                        }
                        let _ = write!(stderr, "stdin write failed: {e}");
                    }

                    let result = lua.create_table()?;
                    result.set("stdout", stdout)?;
                    result.set("stderr", stderr)?;
                    result.set("exit_code", exit_code)?;
                    Ok(result)
                },
            )
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        lark.set("exec_io", exec_io_fn)
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
                lua.to_value_with(&json_value, null_safe_ser_options())
                    .map_err(LuaError::external)
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        json_table
            .set("decode", decode_fn)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        lark.set("json", json_table)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // lark.base64 — standard-alphabet padded encode/decode. Plugins need this
        // for HTTP Basic auth headers and similar wire formats; the shell-out
        // workaround (printf '%s' 'x' | base64) is a fork per call and fragile
        // across platforms.
        let base64_table = lua
            .create_table()
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        let base64_encode = lua
            .create_function(|_, s: mlua::String| {
                use base64::Engine;
                Ok(base64::engine::general_purpose::STANDARD.encode(s.as_bytes()))
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        base64_table
            .set("encode", base64_encode)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        let base64_decode = lua
            .create_function(|lua, s: String| {
                use base64::Engine;
                match base64::engine::general_purpose::STANDARD.decode(s.as_bytes()) {
                    Ok(bytes) => Ok(LuaValue::String(lua.create_string(&bytes)?)),
                    Err(_) => Ok(LuaValue::Nil),
                }
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        base64_table
            .set("decode", base64_decode)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        lark.set("base64", base64_table)
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
                            // Skip a malformed header (non-string key/value)
                            // rather than failing the entire request.
                            let Ok((k, v)) = pair else { continue };
                            req = req.header(&k, &v);
                        }
                    }
                    if let Ok(timeout_secs) = opts.get::<f64>("timeout") {
                        // try_from rejects negative/NaN/overflow (1e20 would
                        // panic in from_secs_f64) — skip invalid timeouts.
                        if let Ok(timeout) = Duration::try_from_secs_f64(timeout_secs) {
                            req = req.timeout(timeout);
                        }
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
                                // Skip a malformed header rather than failing
                                // the entire request.
                                let Ok((k, v)) = pair else { continue };
                                req = req.header(&k, &v);
                            }
                        }
                        if let Ok(timeout_secs) = opts.get::<f64>("timeout") {
                            // Same guard as http.get — see above.
                            if let Ok(timeout) = Duration::try_from_secs_f64(timeout_secs) {
                                req = req.timeout(timeout);
                            }
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
                    Some(val) => lua
                        .to_value_with(val, null_safe_ser_options())
                        .map_err(LuaError::external),
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

                // Supports "Group:Command" to disambiguate same-named
                // commands across plugin groups; errors on an ambiguous bare
                // name rather than silently picking the first match.
                let plugin = crate::plugin::resolve_plugin(plugins.as_slice(), &name)
                    .map_err(|e| LuaError::external(format!("lark.invoke: {e}")))?
                    .clone();

                let output = PLUGIN_LIST
                    .scope(
                        plugins,
                        INVOKE_DEPTH.scope(depth + 1, async move { plugin.execute().await }),
                    )
                    .await
                    .map_err(LuaError::external)?;

                lua.to_value_with(&output, null_safe_ser_options())
                    .map_err(LuaError::external)
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

        // lark.is_cancelled() -> bool — true when the agent has signaled
        // this plugin should abort. Long-running plugin code should poll
        // this between iterations and return early when true.
        //
        // Outside the agent loop (TUI/CLI normal execution) the
        // task-local `CANCEL_TOKEN` is never cancelled, so this always
        // returns false. Plugins that ignore it work fine; they just
        // won't be agent-cancellable. v1.1 may expose a richer cancel
        // surface (deadline, reason).
        let is_cancelled_fn = lua
            .create_function(|_, ()| {
                // try_with returns Err if the task-local isn't set
                // (e.g. plugin invoked outside the engine's scope —
                // tests, fixtures). Treat that as "not cancelled".
                let cancelled = crate::plugin::engine::CANCEL_TOKEN
                    .try_with(tokio_util::sync::CancellationToken::is_cancelled)
                    .unwrap_or(false);
                Ok(cancelled)
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        lark.set("is_cancelled", is_cancelled_fn)
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        // lark.nvim_exec(cmd) -> bool — send an ex command to the parent nvim
        // via $NVIM socket. Returns false when not running under Neovim so
        // plugins can feature-detect.
        let nvim_exec_fn = lua
            .create_function(|_, cmd: String| {
                let Ok(socket) = std::env::var("NVIM") else {
                    return Ok(false);
                };
                let keys = format!("<Esc>:{cmd}<CR>");
                let result = std::process::Command::new("nvim")
                    .args(["--server", &socket, "--remote-send", &keys])
                    .output();
                match result {
                    Ok(output) if output.status.success() => Ok(true),
                    Ok(output) => {
                        tracing::warn!(
                            stderr = %String::from_utf8_lossy(&output.stderr),
                            "nvim_exec remote-send failed"
                        );
                        Ok(false)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "nvim_exec spawn failed");
                        Ok(false)
                    }
                }
            })
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;
        lark.set("nvim_exec", nvim_exec_fn)
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
        let plugin_dir = self.plugin_dir.clone();
        let timeout = self.metadata.timeout;

        // Load the plugin's persistent store.
        let store_path = crate::plugin::store::store_path_for(
            &self.metadata.name,
            self.metadata.plugin_group.as_deref(),
        );
        // Shared per-path instance so a same-group `lark.invoke` (or self-invoke)
        // operates on the same in-memory store and writes are not lost.
        let store = crate::plugin::store::shared(store_path);
        let store_for_save = store.clone();

        // Run the entire Lua execution inside a timeout.
        tokio::time::timeout(timeout, async move {
            let lua = Self::create_vm()?;
            Self::register_api(&lua, plugin_name, &plugin_dir, store)?;

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
        let plugin_dir = self.plugin_dir.clone();
        let timeout = self.metadata.timeout;
        let callback_id = callback_id.to_string();
        let context = context.to_string();

        let store_path = crate::plugin::store::store_path_for(
            &self.metadata.name,
            self.metadata.plugin_group.as_deref(),
        );
        // Shared per-path instance so a same-group `lark.invoke` (or self-invoke)
        // operates on the same in-memory store and writes are not lost.
        let store = crate::plugin::store::shared(store_path);
        let store_for_save = store.clone();

        tokio::time::timeout(timeout, async move {
            let lua = Self::create_vm()?;
            Self::register_api(&lua, plugin_name, &plugin_dir, store)?;

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
    async fn exec_io_env_option_reaches_the_child() {
        let plugin = lua_plugin_from_source(
            "exec-env-test",
            r#"
lark.register({
    on_run = function()
        local res = lark.exec_io("/bin/sh", { "-c", "printf %s \"$LARK_TEST_ENV\"" },
            { env = { LARK_TEST_ENV = "from-opts" } })
        return { title = res.stdout, items = {} }
    end
})
"#,
        );
        let output = plugin.execute().await.expect("execution failed");
        assert_eq!(
            output.title, "from-opts",
            "opts.env must be injected into the child environment"
        );
    }

    #[tokio::test]
    async fn http_timeout_overflow_does_not_panic() {
        let plugin = lua_plugin_from_source(
            "http-timeout-test",
            r#"
lark.register({
    on_run = function()
        -- 1e20 seconds overflows Duration; must not abort the app.
        lark.http.get("http://127.0.0.1:1/", { timeout = 1e20 })
        return { title = "reached", items = {} }
    end
})
"#,
        );
        // The request itself fails (nothing listens on port 1) — the panic
        // in the timeout conversion is the bug under test.
        let _ = plugin.execute().await;
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
    async fn lark_base64_roundtrips_basic_auth_string() {
        // The canonical Atlassian Basic auth header shape: email:token, base64.
        // Verifies both encode + decode and the nil-on-invalid-input contract.
        let plugin = lua_plugin_from_source(
            "base64-test",
            r#"
lark.register({
    on_run = function()
        local creds = "taylor@example.com:abc123"
        local encoded = lark.base64.encode(creds)
        local decoded = lark.base64.decode(encoded)
        local invalid = lark.base64.decode("!!!not-valid-base64!!!")
        local parts = {
            encoded,
            (decoded == creds) and "roundtrip-ok" or "roundtrip-FAIL",
            (invalid == nil) and "invalid-is-nil" or "invalid-FAIL",
        }
        return { title = table.concat(parts, "|"), items = {} }
    end
})
"#,
        );
        let output = plugin.execute().await.expect("execution failed");
        assert_eq!(
            output.title,
            "dGF5bG9yQGV4YW1wbGUuY29tOmFiYzEyMw==|roundtrip-ok|invalid-is-nil"
        );
    }

    #[tokio::test]
    async fn lark_json_null_maps_to_lua_nil() {
        // Regression: mlua's default to_value maps JSON null to a null-sentinel
        // userdata, which is truthy. Plugins write `x and x ~= ""` expecting
        // nil — a userdata sentinel silently slipped through and later crashed
        // table.concat. lark.json.decode must produce nil for null.
        let plugin = lua_plugin_from_source(
            "json-null-test",
            r#"
lark.register({
    on_run = function()
        local decoded = lark.json.decode('{"a": null, "b": "ok"}')
        local parts = {}
        if decoded.a == nil then parts[#parts + 1] = "a-is-nil" end
        -- The dangerous idiom — truthy + non-empty-string check.
        if decoded.a and decoded.a ~= "" then parts[#parts + 1] = "a-leaked" end
        if decoded.b == "ok" then parts[#parts + 1] = "b-is-ok" end
        return { title = table.concat(parts, ","), items = {} }
    end
})
"#,
        );
        let output = plugin.execute().await.expect("execution failed");
        assert_eq!(output.title, "a-is-nil,b-is-ok");
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

    #[tokio::test]
    async fn lark_exec_io_pipes_stdin_and_returns_struct() {
        // `cat` echoes stdin to stdout. We pipe a known JSON payload and
        // assert the round-trip plus that stderr is empty and exit_code is 0.
        let plugin = lua_plugin_from_source(
            "exec-io-stdin-test",
            r#"
lark.register({
    on_run = function()
        local r = lark.exec_io("cat", nil, { stdin = "hello-io\n" })
        return {
            title = string.format("stdout=%s|stderr=%s|code=%d",
                r.stdout:match("^(.-)%s*$"), r.stderr, r.exit_code),
            items = {}
        }
    end
})
"#,
        );
        let output = plugin.execute().await.expect("execution failed");
        assert_eq!(output.title, "stdout=hello-io|stderr=|code=0");
    }

    #[tokio::test]
    async fn lark_exec_io_captures_stderr_and_nonzero_exit() {
        // `sh -c 'echo err >&2; exit 7'` writes to stderr and exits non-zero.
        let plugin = lua_plugin_from_source(
            "exec-io-stderr-test",
            r#"
lark.register({
    on_run = function()
        local r = lark.exec_io("sh", { "-c", "echo my-stderr >&2; exit 7" })
        return {
            title = string.format("stdout=%s|stderr=%s|code=%d",
                r.stdout, r.stderr:match("^(.-)%s*$"), r.exit_code),
            items = {}
        }
    end
})
"#,
        );
        let output = plugin.execute().await.expect("execution failed");
        assert_eq!(output.title, "stdout=|stderr=my-stderr|code=7");
    }

    #[tokio::test]
    async fn lark_exec_io_surfaces_spawn_failure_via_table() {
        // A binary that does not exist must NOT raise — exec_io surfaces the
        // spawn error through the result table (exit_code 127 + stderr) so a
        // plugin's `if res.exit_code ~= 0` guard / from_exit can handle it
        // gracefully. Locks in the documented contract.
        let plugin = lua_plugin_from_source(
            "exec-io-spawn-fail-test",
            r#"
lark.register({
    on_run = function()
        local r = lark.exec_io("larkline-no-such-binary-xyz", { "arg" })
        return {
            title = string.format("code=%d|has_stderr=%s",
                r.exit_code, tostring(r.stderr ~= nil and r.stderr ~= "")),
            items = {}
        }
    end
})
"#,
        );
        let output = plugin.execute().await.expect("execution must not raise");
        assert_eq!(output.title, "code=127|has_stderr=true");
    }
}
