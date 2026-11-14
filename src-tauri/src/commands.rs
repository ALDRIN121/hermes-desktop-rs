//! The IPC surface: one Tauri command, `hermes_ipc`, dispatches on the same
//! channel names the Electron main process registers. Every channel the shell
//! does not implement fails with `unimplemented:<channel>` instead of silently
//! pretending to work.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::fs_ops;
use crate::git_ops;
use crate::system_ops;
use hermes_contract::discovery;
use hermes_contract::types::{HermesApiRequest, HermesConnection, UiPreferences};
use serde_json::{json, Value};

pub struct ShellState {
    pub home: PathBuf,
    /// Attach is a process spawn, so the descriptor is reused briefly.
    cached: Mutex<Option<(Instant, HermesConnection, String)>>,
    pub terminals: std::sync::Arc<crate::terminal::TerminalState>,
    keep_awake: crate::system_ops::KeepAwake,
    zoom: crate::system_ops::Zoom,
}

impl ShellState {
    pub fn new(home: PathBuf) -> Self {
        Self {
            home,
            cached: Mutex::new(None),
            terminals: std::sync::Arc::new(crate::terminal::TerminalState::default()),
            keep_awake: crate::system_ops::KeepAwake::default(),
            zoom: crate::system_ops::Zoom::default(),
        }
    }

    fn attach(&self) -> Result<(HermesConnection, String), String> {
        if let Some((at, conn, instance)) = self.cached.lock().unwrap().as_ref() {
            if at.elapsed() < Duration::from_secs(5) {
                return Ok((conn.clone(), instance.clone()));
            }
        }
        let receipt = discovery::attach(&self.home, 10)?;
        let conn = discovery::connection_from_receipt(&receipt);
        let instance = receipt.endpoint.instance_id.clone().unwrap_or_default();
        *self.cached.lock().unwrap() = Some((Instant::now(), conn.clone(), instance.clone()));
        Ok((conn, instance))
    }

    fn connection(&self) -> Result<HermesConnection, String> {
        self.attach().map(|(conn, _)| conn)
    }

    /// A REST grant for this profile. Electron mints one per call with the
    /// `native-http` purpose; the gateway validates it per request.
    fn rest_ticket(&self, instance: &str) -> Result<String, String> {
        self.mint(instance, "native-http")
    }

    fn mint(&self, instance: &str, purpose: &str) -> Result<String, String> {
        let profile = self.home.to_string_lossy().to_string();
        discovery::mint_ticket(&self.home, instance, &profile, purpose)
    }

    fn forget(&self) {
        *self.cached.lock().unwrap() = None;
    }
}

fn preferences_path(home: &Path) -> PathBuf {
    home.join("ui-preferences.json")
}

pub fn load_preferences(home: &Path) -> UiPreferences {
    std::fs::read_to_string(preferences_path(home))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_preferences(home: &Path, prefs: &UiPreferences) -> Result<(), String> {
    let path = preferences_path(home);
    let body = serde_json::to_vec_pretty(prefs).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, body).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("rename {}: {e}", path.display()))
}

fn arg(args: &[Value], index: usize) -> Value {
    args.get(index).cloned().unwrap_or(Value::Null)
}

/// Proxy one REST call to the resolved backend.
///
/// Electron mints a private per-call grant here; the Rust shell sends the
/// descriptor token, which is what the remote/token mode already uses. Local
/// native mode answers 401 for guarded routes until the grant channel is
/// ported — that failure is surfaced verbatim, not hidden.
fn api_call(
    conn: &HermesConnection,
    ticket: Option<&str>,
    request: &HermesApiRequest,
) -> Result<Value, String> {
    let base = conn.base_url.trim_end_matches('/');
    let path = if request.path.starts_with('/') {
        request.path.clone()
    } else {
        format!("/{}", request.path)
    };
    let method = request
        .method
        .clone()
        .unwrap_or_else(|| "GET".into())
        .to_ascii_uppercase();
    let timeout = Duration::from_millis(request.timeout_ms.unwrap_or(30_000));

    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let mut req = agent.request(&method, &format!("{base}{path}"));
    if let Some(ticket) = ticket {
        // Local native mode: a one-use grant minted over the control socket.
        req = req.set("X-Hermes-Gateway-Ticket", ticket);
    }
    if !conn.token.is_empty() {
        req = req.set("Authorization", &format!("Bearer {}", conn.token));
    }

    let outcome = match &request.body {
        Some(body) if method != "GET" && method != "HEAD" => req.send_json(body.clone()),
        _ => req.call(),
    };

    match outcome {
        Ok(response) => {
            let text = response.into_string().map_err(|e| e.to_string())?;
            Ok(serde_json::from_str(&text).unwrap_or_else(|_| json!({ "raw": text })))
        }
        // The renderer's `unwrapExpectedNotFound` maps `"<status>: <body>"`.
        Err(ureq::Error::Status(code, response)) => Err(format!(
            "{code}: {}",
            response.into_string().unwrap_or_default()
        )),
        Err(other) => Err(other.to_string()),
    }
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("m4a") => "audio/mp4",
        Some("pdf") => "application/pdf",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

/// `hermes-media://stream/<percent-encoded path>` → the file itself.
/// Remote paths (`hermes-media://remote/...`) need the grant channel first.
pub fn media_response(uri: &str) -> tauri::http::Response<Vec<u8>> {
    let not_found = || {
        tauri::http::Response::builder()
            .status(404)
            .header("content-type", "text/plain")
            .body(b"not found".to_vec())
            .unwrap()
    };

    let rest = uri.strip_prefix("hermes-media://").unwrap_or(uri);
    let (host, tail) = rest.split_once('/').unwrap_or((rest, ""));
    if host != "stream" {
        return tauri::http::Response::builder()
            .status(501)
            .header("content-type", "text/plain")
            .body(format!("unsupported hermes-media host: {host}").into_bytes())
            .unwrap();
    }
    let raw = tail.split('?').next().unwrap_or(tail);
    let path = urlencoding::decode(raw)
        .map(|p| p.into_owned())
        .unwrap_or_else(|_| raw.to_string());
    let file = Path::new(&path);
    match std::fs::read(file) {
        Ok(bytes) => tauri::http::Response::builder()
            .status(200)
            .header("content-type", mime_for(file))
            .header("access-control-allow-origin", "*")
            .body(bytes)
            .unwrap(),
        Err(_) => not_found(),
    }
}

#[tauri::command]
pub async fn hermes_ws_open(
    app: tauri::AppHandle,
    registry: tauri::State<'_, std::sync::Arc<crate::gateway_ws::SocketRegistry>>,
    url: String,
    protocols: Option<Vec<String>>,
) -> Result<u64, String> {
    crate::gateway_ws::open(
        app,
        registry.inner().clone(),
        url,
        protocols.unwrap_or_default(),
    )
    .await
}

#[tauri::command]
pub fn hermes_ws_send(
    registry: tauri::State<'_, std::sync::Arc<crate::gateway_ws::SocketRegistry>>,
    id: u64,
    data: String,
) -> Result<(), String> {
    if crate::gateway_ws::send(registry.inner(), id, &data) {
        Ok(())
    } else {
        Err("socket is not open".into())
    }
}

#[tauri::command]
pub fn hermes_ws_close(
    registry: tauri::State<'_, std::sync::Arc<crate::gateway_ws::SocketRegistry>>,
    id: u64,
) -> Result<(), String> {
    crate::gateway_ws::close(registry.inner(), id);
    Ok(())
}

/// Canonical JSON-RPC for shell-owned windows (workspace/task views).
#[tauri::command]
pub async fn task_call(
    state: tauri::State<'_, ShellState>,
    method: String,
    params: Value,
    timeout_ms: Option<u64>,
) -> Result<Value, String> {
    let (conn, instance) = state.attach()?;
    let ticket = state.mint(&instance, "interactive")?;
    let budget = std::time::Duration::from_millis(timeout_ms.unwrap_or(20_000));
    crate::task_ops::call(&conn, &ticket, &method, params, budget).await
}

/// Submit a prompt to a session (attach + submit on one socket).
#[tauri::command]
pub async fn task_submit(
    state: tauri::State<'_, ShellState>,
    session_id: String,
    profile: String,
    text: String,
    timeout_ms: Option<u64>,
) -> Result<Value, String> {
    let (conn, instance) = state.attach()?;
    let ticket = state.mint(&instance, "interactive")?;
    let budget = std::time::Duration::from_millis(timeout_ms.unwrap_or(30_000));
    crate::task_ops::submit(&conn, &ticket, &session_id, &profile, &text, budget).await
}

/// Resource usage of the gateway process the shell is attached to.
/// Read from its pid file, then `ps` — the same numbers Activity Monitor shows.
#[tauri::command]
pub fn task_stats() -> Value {
    let home = std::env::var_os("HERMES_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".hermes")
        });
    // gateway.pid is JSON ({"pid": 90832, "kind": "hermes-gateway", …}), with a
    // bare integer in older installs — accept both.
    let pid = std::fs::read_to_string(home.join("gateway.pid"))
        .ok()
        .and_then(|raw| {
            let trimmed = raw.trim();
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if let Some(pid) = value.get("pid").and_then(|p| p.as_u64()) {
                    return Some(pid as u32);
                }
            }
            trimmed.parse::<u32>().ok()
        });
    let Some(pid) = pid else {
        return json!({ "pid": Value::Null, "cpu": Value::Null, "rss": Value::Null });
    };
    let output = std::process::Command::new("ps")
        .args(["-o", "%cpu=,rss=", "-p", &pid.to_string()])
        .output();
    let Ok(output) = output else {
        return json!({ "pid": pid, "cpu": Value::Null, "rss": Value::Null });
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut parts = text.split_whitespace();
    let cpu = parts.next().and_then(|v| v.parse::<f64>().ok());
    let rss_kb = parts.next().and_then(|v| v.parse::<u64>().ok());
    json!({
        "pid": pid,
        "cpu": cpu,
        "rss": rss_kb.map(|kb| kb * 1024),
    })
}

#[tauri::command]
pub fn open_workspace(app: tauri::AppHandle) -> Result<(), String> {
    crate::open_workspace_window(&app, false)
}

#[tauri::command]
pub fn open_task(app: tauri::AppHandle) -> Result<(), String> {
    crate::open_task_window(&app, false)
}

#[tauri::command]
pub fn shell_diag(payload: Value) {
    let dir = std::env::temp_dir();
    // Windows report independently: a shared file would let the chat window's
    // later report overwrite the workspace window's results.
    let suffix = payload
        .get("window")
        .and_then(|v| v.as_str())
        .map(|label| format!("-{label}"))
        .unwrap_or_default();
    let path = dir.join(format!("hermes-rust-diag{suffix}.json"));
    let _ = std::fs::write(&path, serde_json::to_vec_pretty(&payload).unwrap_or_default());
    eprintln!("diag{}: {}", suffix, payload);
}

#[tauri::command]
pub fn shell_info(state: tauri::State<'_, ShellState>) -> Value {
    let ledger = hermes_contract::PortLedger::measure();
    json!({
        "shell": "rust",
        "runtime": "tauri",
        "version": env!("CARGO_PKG_VERSION"),
        "hermesHome": state.home,
        "channels": {
            "total": ledger.total,
            "ported": ledger.ported,
            "stubbed": ledger.stubbed,
            "requests": ledger.requests,
            "requestsPorted": ledger.requests_ported,
            "events": ledger.events
        },
        "portedChannels": hermes_contract::registry::PORTED,
    })
}

#[tauri::command]
pub async fn hermes_ipc(
    app: tauri::AppHandle,
    channel: String,
    args: Option<Vec<Value>>,
    state: tauri::State<'_, ShellState>,
    window: tauri::WebviewWindow,
) -> Result<Value, String> {
    let args = args.unwrap_or_default();
    match channel.as_str() {
        "hermes:feature-flags" => Ok(json!({
            "localModels": false,
            "assumeFreshInstall": false
        })),

        "hermes:skin:local" => Ok(std::fs::read_to_string(state.home.join("skin.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .unwrap_or(Value::Null)),

        "hermes:ui-preferences:snapshot" | "hermes:ui-preferences:read" => {
            Ok(serde_json::to_value(load_preferences(&state.home)).map_err(|e| e.to_string())?)
        }

        "hermes:ui-preferences:write" => {
            let mut prefs = load_preferences(&state.home);
            let payload = arg(&args, 0);
            if let Some(values) = payload.get("values").and_then(|v| v.as_object()) {
                for (key, value) in values {
                    prefs.values.insert(key.clone(), value.clone());
                }
            } else if let Some(map) = payload.as_object() {
                for (key, value) in map {
                    prefs.values.insert(key.clone(), value.clone());
                }
            }
            prefs.revision = prefs.revision.saturating_add(1);
            save_preferences(&state.home, &prefs)?;
            Ok(serde_json::to_value(prefs).map_err(|e| e.to_string())?)
        }

        "hermes:translucency:support" => Ok(json!({ "glass": false, "translucency": false })),

        // The Rust shell attaches to an already-running gateway, so there is
        // no managed install to report: boot is complete and no bootstrap is
        // in flight. Shapes mirror `DesktopBootProgress`/`DesktopBootstrapState`.
        "hermes:boot-progress:get" => Ok(json!({
            "error": null,
            "fakeMode": false,
            "message": "Connected",
            "phase": "ready",
            "progress": 100
        })),

        "hermes:bootstrap:get" => Ok(json!({
            "active": false,
            "manifest": null,
            "stages": {},
            "error": null,
            "log": [],
            "startedAt": null,
            "completedAt": null,
            "setupChoice": null,
            "unsupportedPlatform": null,
            "bundled": false
        })),

        "hermes:hud:windowing" => Ok(Value::Null),

        "hermes:connection" | "hermes:connection:for" => {
            let conn = state.connection()?;
            Ok(serde_json::to_value(conn).map_err(|e| e.to_string())?)
        }

        "hermes:connection:revalidate" => {
            state.forget();
            let rebuilt = state.connection().is_ok();
            Ok(json!({ "ok": rebuilt, "rebuilt": rebuilt }))
        }

        "hermes:gateway:ws-url" | "hermes:gateway:ws-url-for" => {
            let (conn, instance) = state.attach()?;
            // The renderer turns `?native_dial=1&ticket=…` into the
            // `hermes-gateway-ticket.*` subprotocol itself (canonical-protocol.ts).
            let ticket = state.mint(&instance, "interactive")?;
            let sep = if conn.ws_url.contains('?') { '&' } else { '?' };
            Ok(json!({
                "ok": true,
                "wsUrl": format!("{}{}native_dial=1&ticket={}", conn.ws_url, sep, ticket)
            }))
        }

        "hermes:embed-host:origin" => Ok(Value::String(String::new())),

        // Resolution only: this shell never installs a runtime, it attaches to
        // one that is already running.
        "hermes:local-backend:probe" => Ok(json!({ "bootstrapNeeded": false })),

        "hermes:version" => Ok(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "shell": "rust"
        })),

        "hermes:window-control" => {
            match arg(&args, 0).as_str().unwrap_or_default() {
                "minimize" => window.minimize().map_err(|e| e.to_string())?,
                "toggle-maximize" => {
                    if window.is_maximized().map_err(|e| e.to_string())? {
                        window.unmaximize().map_err(|e| e.to_string())?;
                    } else {
                        window.maximize().map_err(|e| e.to_string())?;
                    }
                }
                "close" => window.close().map_err(|e| e.to_string())?,
                other => return Err(format!("bad window-control: {other}")),
            }
            Ok(Value::Null)
        }

        "hermes:api" => {
            let (conn, instance) = state.attach()?;
            let request: HermesApiRequest = serde_json::from_value(arg(&args, 0))
                .map_err(|e| format!("bad HermesApiRequest: {e}"))?;
            let ticket = state.rest_ticket(&instance).ok();
            tauri::async_runtime::spawn_blocking(move || {
                api_call(&conn, ticket.as_deref(), &request)
            })
            .await
            .map_err(|e| e.to_string())?
        }

        // --- filesystem, dialogs, clipboard ---------------------------------
        "hermes:readFileText" => Ok(fs_ops::read_file_text(
            arg(&args, 0).as_str().unwrap_or_default(),
        )),

        "hermes:readPluginSource" => Ok(fs_ops::read_plugin_source(
            arg(&args, 0).as_str().unwrap_or_default(),
        )),

        "hermes:readFileDataUrl" | "hermes:readFileDataUrlForAttach" => {
            let path = arg(&args, 0).as_str().unwrap_or_default().to_string();
            let cap = fs_ops::data_url_read_max_mb(&state.home) * 1024 * 1024;
            tauri::async_runtime::spawn_blocking(move || fs_ops::read_file_data_url(&path, cap))
                .await
                .map_err(|e| e.to_string())
        }

        "hermes:data-url-read-max:get" => Ok(fs_ops::data_url_read_max_state(&state.home)),

        "hermes:data-url-read-max:set" => {
            let max_mb = arg(&args, 0).as_u64().unwrap_or(fs_ops::DATA_URL_DEFAULT_MAX_MB);
            fs_ops::set_data_url_read_max_mb(&state.home, max_mb)
        }

        "hermes:fs:readDir" => Ok(fs_ops::read_dir(
            arg(&args, 0).as_str().unwrap_or_default(),
        )),

        "hermes:fs:gitRoot" => Ok(fs_ops::git_root(
            arg(&args, 0).as_str().unwrap_or_default(),
        )),

        "hermes:fs:reveal" => Ok(Value::Bool(fs_ops::reveal(
            arg(&args, 0).as_str().unwrap_or_default(),
        ))),

        "hermes:fs:openDir" => Ok(fs_ops::open_dir(
            arg(&args, 0).as_str().unwrap_or_default(),
        )),

        "hermes:fs:desktopPluginsRoot" => Ok(Value::String(fs_ops::desktop_plugins_root(
            &state.home,
        ))),

        "hermes:fs:logsRoot" => Ok(Value::String(fs_ops::logs_root(
            &state.home,
            arg(&args, 0).as_str(),
        ))),

        "hermes:fs:rename" => fs_ops::rename(
            arg(&args, 0).as_str().unwrap_or_default(),
            arg(&args, 1).as_str().unwrap_or_default(),
        ),

        "hermes:fs:writeText" => fs_ops::write_text(
            arg(&args, 0).as_str().unwrap_or_default(),
            arg(&args, 1).as_str().unwrap_or_default(),
        ),

        "hermes:fs:trash" => Ok(Value::Bool(fs_ops::trash(
            arg(&args, 0).as_str().unwrap_or_default(),
        )?)),

        // Reconcile is an installer concern; this shell publishes nothing and
        // refuses nothing, which is exactly the state of an empty app half.
        "hermes:fs:reconcileDesktopPlugins" => Ok(json!({ "published": [], "refused": [] })),

        "hermes:selectPaths" => {
            let options = arg(&args, 0);
            tauri::async_runtime::spawn_blocking(move || fs_ops::select_paths(&options))
                .await
                .map_err(|e| e.to_string())?
        }

        "hermes:selectSavePath" => {
            let options = arg(&args, 0);
            tauri::async_runtime::spawn_blocking(move || fs_ops::select_save_path(&options))
                .await
                .map_err(|e| e.to_string())?
        }

        "hermes:saveTextFileAs" => {
            let request = arg(&args, 0);
            tauri::async_runtime::spawn_blocking(move || fs_ops::save_text_file_as(&request))
                .await
                .map_err(|e| e.to_string())?
        }

        "hermes:readClipboard" => fs_ops::read_clipboard(),

        "hermes:writeClipboard" => Ok(Value::Bool(fs_ops::write_clipboard(
            arg(&args, 0).as_str().unwrap_or_default(),
        )?)),

        "hermes:saveImageBuffer" => fs_ops::save_image_buffer(&arg(&args, 0)),

        "hermes:savePastedText" => fs_ops::save_pasted_text(&state.home, &arg(&args, 0)),

        // --- embedded terminal ----------------------------------------------
        "hermes:terminal:start" => {
            let terminals = state.terminals.clone();
            let payload = arg(&args, 0);
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                crate::terminal::start(app, &terminals, &payload)
            })
            .await
            .map_err(|e| e.to_string())?
        }

        "hermes:terminal:attach" => Ok(Value::Bool(crate::terminal::attach(
            &state.terminals,
            arg(&args, 0).as_str().unwrap_or_default(),
        ))),

        "hermes:terminal:write" => Ok(Value::Bool(crate::terminal::write(
            &state.terminals,
            arg(&args, 0).as_str().unwrap_or_default(),
            arg(&args, 1).as_str().unwrap_or_default(),
        ))),

        "hermes:terminal:resize" => {
            let size = arg(&args, 1);
            Ok(Value::Bool(crate::terminal::resize(
                &state.terminals,
                arg(&args, 0).as_str().unwrap_or_default(),
                size.get("cols").and_then(|v| v.as_u64()).unwrap_or(80),
                size.get("rows").and_then(|v| v.as_u64()).unwrap_or(24),
            )))
        }

        "hermes:terminal:cwd" => Ok(crate::terminal::cwd(
            &state.terminals,
            arg(&args, 0).as_str().unwrap_or_default(),
        )
        .map(Value::String)
        .unwrap_or(Value::Null)),

        "hermes:terminal:dispose" => Ok(Value::Bool(crate::terminal::dispose(
            &state.terminals,
            arg(&args, 0).as_str().unwrap_or_default(),
        ))),

        // --- git -------------------------------------------------------------
        "hermes:git:repoStatus" => {
            let repo = arg(&args, 0);
            tauri::async_runtime::spawn_blocking(move || {
                git_ops::repo_status(repo.as_str().unwrap_or_default())
            })
            .await
            .map_err(|e| e.to_string())
        }

        "hermes:git:branchList" | "hermes:git:baseBranchList" => {
            let repo = arg(&args, 0);
            tauri::async_runtime::spawn_blocking(move || {
                git_ops::branch_list(repo.as_str().unwrap_or_default())
            })
            .await
            .map_err(|e| e.to_string())
        }

        "hermes:git:branchSwitch" => {
            let repo = arg(&args, 0);
            let branch = arg(&args, 1);
            tauri::async_runtime::spawn_blocking(move || {
                git_ops::branch_switch(
                    repo.as_str().unwrap_or_default(),
                    branch.as_str().unwrap_or_default(),
                )
            })
            .await
            .map_err(|e| e.to_string())?
        }

        "hermes:git:worktreeList" => {
            let repo = arg(&args, 0);
            tauri::async_runtime::spawn_blocking(move || {
                git_ops::worktree_list(repo.as_str().unwrap_or_default())
            })
            .await
            .map_err(|e| e.to_string())
        }

        "hermes:git:worktreeAdd" => {
            let repo = arg(&args, 0);
            let options = arg(&args, 1);
            tauri::async_runtime::spawn_blocking(move || {
                git_ops::worktree_add(repo.as_str().unwrap_or_default(), &options)
            })
            .await
            .map_err(|e| e.to_string())?
        }

        "hermes:git:worktreeRemove" => {
            let repo = arg(&args, 0);
            let tree = arg(&args, 1);
            let options = arg(&args, 2);
            tauri::async_runtime::spawn_blocking(move || {
                git_ops::worktree_remove(
                    repo.as_str().unwrap_or_default(),
                    tree.as_str().unwrap_or_default(),
                    &options,
                )
            })
            .await
            .map_err(|e| e.to_string())?
        }

        "hermes:git:review:list" => {
            let repo = arg(&args, 0);
            let scope = arg(&args, 1);
            let base = arg(&args, 2);
            tauri::async_runtime::spawn_blocking(move || {
                git_ops::review_list(
                    repo.as_str().unwrap_or_default(),
                    scope.as_str().unwrap_or_default(),
                    base.as_str().unwrap_or_default(),
                )
            })
            .await
            .map_err(|e| e.to_string())
        }

        "hermes:git:review:diff" => {
            let repo = arg(&args, 0);
            let file = arg(&args, 1);
            let scope = arg(&args, 2);
            let base = arg(&args, 3);
            let staged = arg(&args, 4).as_bool().unwrap_or(false);
            tauri::async_runtime::spawn_blocking(move || {
                Value::String(git_ops::review_diff(
                    repo.as_str().unwrap_or_default(),
                    file.as_str().unwrap_or_default(),
                    scope.as_str().unwrap_or_default(),
                    base.as_str().unwrap_or_default(),
                    staged,
                ))
            })
            .await
            .map_err(|e| e.to_string())
        }

        "hermes:git:fileDiff" => {
            let repo = arg(&args, 0);
            let file = arg(&args, 1);
            tauri::async_runtime::spawn_blocking(move || {
                Value::String(git_ops::review_diff(
                    repo.as_str().unwrap_or_default(),
                    file.as_str().unwrap_or_default(),
                    "uncommitted",
                    "",
                    false,
                ))
            })
            .await
            .map_err(|e| e.to_string())
        }

        "hermes:git:review:stage" | "hermes:git:review:unstage" | "hermes:git:review:revert" => {
            let channel = channel.clone();
            let repo = arg(&args, 0);
            let file = arg(&args, 1).as_str().map(str::to_string);
            tauri::async_runtime::spawn_blocking(move || match channel.as_str() {
                "hermes:git:review:stage" => git_ops::stage(repo.as_str().unwrap_or_default(), file.as_deref()),
                "hermes:git:review:unstage" => {
                    git_ops::unstage(repo.as_str().unwrap_or_default(), file.as_deref())
                }
                _ => git_ops::revert(repo.as_str().unwrap_or_default(), file.as_deref()),
            })
            .await
            .map_err(|e| e.to_string())?
        }

        "hermes:git:review:revParse" => {
            let repo = arg(&args, 0);
            let reference = arg(&args, 1);
            tauri::async_runtime::spawn_blocking(move || {
                git_ops::rev_parse(
                    repo.as_str().unwrap_or_default(),
                    reference.as_str().unwrap_or_default(),
                )
            })
            .await
            .map_err(|e| e.to_string())
        }

        "hermes:git:review:commit" => {
            let repo = arg(&args, 0);
            let message = arg(&args, 1);
            let push = arg(&args, 2).as_bool().unwrap_or(false);
            tauri::async_runtime::spawn_blocking(move || {
                git_ops::commit(
                    repo.as_str().unwrap_or_default(),
                    message.as_str().unwrap_or_default(),
                    push,
                )
            })
            .await
            .map_err(|e| e.to_string())?
        }

        "hermes:git:review:push" => {
            let repo = arg(&args, 0);
            tauri::async_runtime::spawn_blocking(move || {
                git_ops::push(repo.as_str().unwrap_or_default())
            })
            .await
            .map_err(|e| e.to_string())?
        }

        "hermes:git:review:commitContext" => {
            let repo = arg(&args, 0);
            tauri::async_runtime::spawn_blocking(move || {
                git_ops::commit_context(repo.as_str().unwrap_or_default())
            })
            .await
            .map_err(|e| e.to_string())
        }

        "hermes:git:review:shipInfo" => {
            let repo = arg(&args, 0);
            tauri::async_runtime::spawn_blocking(move || {
                git_ops::ship_info(repo.as_str().unwrap_or_default())
            })
            .await
            .map_err(|e| e.to_string())
        }

        "hermes:git:review:prList" => {
            let repo = arg(&args, 0);
            tauri::async_runtime::spawn_blocking(move || {
                git_ops::pr_list(repo.as_str().unwrap_or_default())
            })
            .await
            .map_err(|e| e.to_string())
        }

        "hermes:git:review:createPr" => {
            let repo = arg(&args, 0);
            tauri::async_runtime::spawn_blocking(move || {
                git_ops::create_pr(repo.as_str().unwrap_or_default())
            })
            .await
            .map_err(|e| e.to_string())?
        }

        "hermes:git:scanRepos" => {
            let roots: Vec<String> = arg(&args, 0)
                .as_array()
                .map(|items| items.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let max_depth = arg(&args, 1)
                .get("maxDepth")
                .and_then(|v| v.as_u64())
                .unwrap_or(2) as usize;
            tauri::async_runtime::spawn_blocking(move || git_ops::scan_repos(&roots, max_depth))
                .await
                .map_err(|e| e.to_string())
        }

        // --- logs, zoom, keep-awake, notifications ---------------------------
        "hermes:logs:recent" => Ok(system_ops::recent_logs(&state.home, 200)),
        "hermes:logs:reveal" => Ok(Value::Bool(system_ops::reveal_logs(&state.home))),

        "hermes:logs:renderer-error" => {
            let line = format!("[renderer-error] {}", arg(&args, 0));
            system_ops::append_log(&state.home, &line);
            Ok(Value::Null)
        }

        "hermes:logs:renderer-line" => {
            let line = arg(&args, 0).as_str().unwrap_or_default().to_string();
            system_ops::append_log(&state.home, &line);
            Ok(Value::Null)
        }

        "hermes:zoom:get" => {
            let state = system_ops::zoom_get(&state.zoom);
            let percent = state["percent"].as_f64().unwrap_or(100.0);
            Ok(json!({ "level": percent / 100.0, "percent": percent }))
        }

        "hermes:zoom:set-percent" => {
            let percent = arg(&args, 0).as_f64().unwrap_or(100.0);
            let _ = window.set_zoom((percent / 100.0).clamp(0.5, 2.0));
            Ok(system_ops::zoom_state(&state.zoom, percent))
        }

        "hermes:keep-awake" => {
            let mode = arg(&args, 0).as_str().unwrap_or_default().to_string();
            Ok(Value::Bool(state.keep_awake.set(&mode)))
        }

        "hermes:notify" => Ok(Value::Bool(system_ops::notify(&arg(&args, 0)))),

        other => Err(format!("unimplemented:{other}")),
    }
}


/// Receives the renderer's UI audit report and appends it to a per-width file,
/// so a sweep over several window sizes can be diffed afterwards.
#[tauri::command]
pub fn task_audit(payload: String) -> Result<String, String> {
    let path = std::env::temp_dir().join("hermes-rust-audit.json");
    let width = serde_json::from_str::<serde_json::Value>(&payload)
        .ok()
        .and_then(|value| value.get("width").and_then(|w| w.as_u64()))
        .unwrap_or(0);
    let mut reports: Vec<serde_json::Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    reports.retain(|report| report.get("width").and_then(|w| w.as_u64()) != Some(width));
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&payload) {
        reports.push(value);
    }
    reports.sort_by_key(|report| report.get("width").and_then(|w| w.as_u64()).unwrap_or(0));
    std::fs::write(&path, serde_json::to_vec_pretty(&reports).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}
