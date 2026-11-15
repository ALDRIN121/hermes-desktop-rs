//! The IPC surface: one Tauri command, `hermes_ipc`, dispatches on the same
//! channel names the Electron main process registers. Every channel the shell
//! does not implement fails with `unimplemented:<channel>` instead of silently
//! pretending to work.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::fs_ops;
use crate::git_ops;
use crate::system_ops;
use hermes_contract::types::{HermesApiRequest, HermesConnection, UiPreferences};
use hermes_contract::{native_gateway, scope};
use serde_json::{json, Value};

pub struct ShellState {
    pub home: PathBuf,
    pub terminals: std::sync::Arc<crate::terminal::TerminalState>,
    keep_awake: crate::system_ops::KeepAwake,
    zoom: crate::system_ops::Zoom,
}

impl ShellState {
    pub fn new(home: PathBuf) -> Self {
        Self {
            home,
            terminals: std::sync::Arc::new(crate::terminal::TerminalState::default()),
            keep_awake: crate::system_ops::KeepAwake::default(),
            zoom: crate::system_ops::Zoom::default(),
        }
    }
}

fn resolve_native(home: &Path, payload: &Value) -> Result<native_gateway::NativeGateway, String> {
    let profile = scope::requested_profile(payload)?;
    native_gateway::resolve(home, profile.as_deref())
}

async fn resolve_native_async(
    home: PathBuf,
    payload: Value,
) -> Result<native_gateway::NativeGateway, String> {
    tauri::async_runtime::spawn_blocking(move || resolve_native(&home, &payload))
        .await
        .map_err(|e| e.to_string())?
}

/// Scope-checked owner resolution for the desktop file bridge: a payload names
/// a connection and profile the same way a REST request does, so the same
/// local-only rule applies.
pub fn resolve_owner(
    home: &Path,
    payload: &Value,
) -> Result<native_gateway::NativeGateway, String> {
    scope::require_local_connection(payload.get("connectionId"))?;
    let profile = scope::profile_name(
        payload
            .get("profile")
            .and_then(|value| value.as_str())
            .unwrap_or_default(),
    )?;
    native_gateway::resolve(home, profile.as_deref())
}

fn owned_api_call(home: &Path, mut request: HermesApiRequest) -> Result<Value, String> {
    scope::require_local_connection(request.connection_id.as_ref())?;
    let route = scope::route_http(
        &request.path,
        request.method.as_deref().unwrap_or("GET"),
        request.body.as_ref(),
        request.profile.as_ref(),
    )?;
    let owner = native_gateway::resolve(home, route.profile.as_deref())?;
    let ticket = owner.mint(route.purpose)?;
    request.path = route.path;
    api_call(&owner.connection, Some(&ticket), &request)
}

#[cfg(test)]
mod owner_tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};

    #[test]
    fn foreign_connection_and_conflicting_profile_never_touch_the_default_home() {
        let absent = Path::new("/not-a-hermes-fixture-home");
        let foreign: HermesApiRequest = serde_json::from_value(
            json!({"path":"/api/config","connectionId":"remote","method":"PATCH","body":{"x":1}}),
        )
        .unwrap();
        assert!(owned_api_call(absent, foreign)
            .unwrap_err()
            .contains("not yet supported"));
        let conflict: HermesApiRequest =
            serde_json::from_value(json!({"path":"/api/config?profile=a","profile":"b"})).unwrap();
        assert!(owned_api_call(absent, conflict)
            .unwrap_err()
            .contains("Conflicting"));
    }

    #[test]
    fn actual_rest_request_uses_named_profile_and_its_private_grant() {
        let home = std::env::temp_dir().join(format!("hermes-owned-api-{}", std::process::id()));
        std::fs::create_dir(&home).unwrap();
        let home = home.canonicalize().unwrap();
        let control_path = home.join("gateway.sock");
        let control = UnixListener::bind(&control_path).unwrap();
        std::fs::set_permissions(&control_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let http = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", http.local_addr().unwrap());
        let profile_home = home.join("profiles/work");
        let expected_home = profile_home.clone();
        let control_home = home.clone();
        let issuer = std::thread::spawn(move || {
            for verb in ["resolve-profile", "session-ticket"] {
                let (mut client, _) = control.accept().unwrap();
                client
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut line = String::new();
                BufReader::new(client.try_clone().unwrap())
                    .read_line(&mut line)
                    .unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                assert_eq!(request["verb"], verb);
                let result = if verb == "resolve-profile" {
                    assert_eq!(request["params"]["home"], expected_home.to_str().unwrap());
                    json!({"resolve_protocol":1,"state":"ready","attachable":true,"update_fenced":false,"endpoint":{
                        "runtime_protocol":1,"api_origin":origin,"ws_url":origin.replacen("http:","ws:",1)+"/api/ws",
                        "profile_id":expected_home,"control_home":control_home,"instance_id":"work-owner"
                    }})
                } else {
                    assert_eq!(
                        request["params"]["profile_id"],
                        expected_home.to_str().unwrap()
                    );
                    assert_eq!(request["params"]["instance_id"], "work-owner");
                    assert_eq!(request["params"]["purpose"], "native-http");
                    assert!(request["params"].get("scope").is_none());
                    json!({"ticket":"fixture-work-grant"})
                };
                writeln!(
                    client,
                    "{}",
                    json!({"protocol":1,"id":1,"ok":true,"result":result})
                )
                .unwrap();
            }
        });
        let backend = std::thread::spawn(move || {
            let (mut client, _) = http.accept().unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(client.try_clone().unwrap());
            let mut request = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                request.push_str(&line);
            }
            assert!(
                request.starts_with("GET /api/config?profile=work HTTP/1.1"),
                "{request}"
            );
            assert!(request
                .to_ascii_lowercase()
                .contains("x-hermes-gateway-ticket: fixture-work-grant"));
            assert!(!request.to_ascii_lowercase().contains("authorization:"));
            let body = "{\"owner\":\"work\"}";
            write!(client, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        });
        let request = serde_json::from_value(
            json!({"path":"/api/config","profile":"work","connectionId":"local"}),
        )
        .unwrap();
        let response = owned_api_call(&home, request);
        issuer.join().unwrap();
        backend.join().unwrap();
        std::fs::remove_dir_all(home).unwrap();
        assert_eq!(response.unwrap()["owner"], "work");
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

pub fn save_preferences(home: &Path, prefs: &UiPreferences) -> Result<(), String> {
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
/// The owner and method-specific native grant have already been resolved.
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

    let builder = ureq::AgentBuilder::new().redirects(0);
    let agent = if timeout.is_zero() {
        builder
    } else {
        builder.timeout(timeout)
    }
    .build();
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
///
/// Byte ranges matter here: WebKit's `<audio>`/`<video>` seek only when the
/// server answers `Range` with a 206 and an `accept-ranges` header, and a
/// player that cannot seek cannot play a voice clip past its first chunk. The
/// range is read from disk directly, so a 200 MB video never lands in memory
/// whole just to play ten seconds of it.
pub fn media_response(uri: &str, range_header: Option<&str>) -> tauri::http::Response<Vec<u8>> {
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
    let Ok(meta) = std::fs::metadata(file) else {
        return not_found();
    };
    if !meta.is_file() {
        return not_found();
    }
    let total = meta.len();
    let mime = mime_for(file);

    let Some((start, end)) = parse_range(range_header, total) else {
        // An unparseable or unsatisfiable range must be refused, not silently
        // answered with the whole file: the player would mis-seat its cursor.
        return tauri::http::Response::builder()
            .status(416)
            .header("content-range", format!("bytes */{total}"))
            .header("content-type", "text/plain")
            .body(b"range not satisfiable".to_vec())
            .unwrap();
    };

    use std::io::{Read, Seek, SeekFrom};
    let length = end - start + 1;
    let mut buffer = vec![0u8; length as usize];
    let read = std::fs::File::open(file).and_then(|mut handle| {
        handle.seek(SeekFrom::Start(start))?;
        handle.read_exact(&mut buffer)
    });
    if read.is_err() {
        return not_found();
    }

    let builder = tauri::http::Response::builder()
        .header("content-type", mime)
        .header("access-control-allow-origin", "*")
        .header("accept-ranges", "bytes");

    if start == 0 && length == total {
        builder.status(200).body(buffer).unwrap()
    } else {
        builder
            .status(206)
            .header("content-range", format!("bytes {start}-{end}/{total}"))
            .header("content-length", length.to_string())
            .body(buffer)
            .unwrap()
    }
}

/// `bytes=start-end`, `bytes=start-` and `bytes=-suffix`; `None` when the
/// header is absent (serve everything) and `None` when it cannot be honoured.
enum RangeParse {
    Whole,
    Partial(u64, u64),
    Unsatisfiable,
}

fn parse_range_typed(header: Option<&str>, total: u64) -> RangeParse {
    let Some(raw) = header else {
        return RangeParse::Whole;
    };
    let raw = raw.trim();
    let Some(spec) = raw.strip_prefix("bytes=") else {
        // Some other unit: serve the whole representation, which is allowed.
        return RangeParse::Whole;
    };
    // Multi-range answers would need multipart/byteranges; take the first.
    let spec = spec.split(',').next().unwrap_or(spec).trim();
    let Some((from, to)) = spec.split_once('-') else {
        return RangeParse::Unsatisfiable;
    };
    if total == 0 {
        return RangeParse::Unsatisfiable;
    }

    if from.is_empty() {
        let Ok(suffix) = to.trim().parse::<u64>() else {
            return RangeParse::Unsatisfiable;
        };
        if suffix == 0 {
            return RangeParse::Unsatisfiable;
        }
        let start = total.saturating_sub(suffix);
        return RangeParse::Partial(start, total - 1);
    }

    let Ok(start) = from.trim().parse::<u64>() else {
        return RangeParse::Unsatisfiable;
    };
    if start >= total {
        return RangeParse::Unsatisfiable;
    }
    let end = if to.trim().is_empty() {
        total - 1
    } else {
        let Ok(end) = to.trim().parse::<u64>() else {
            return RangeParse::Unsatisfiable;
        };
        if end < start {
            return RangeParse::Unsatisfiable;
        }
        end.min(total - 1)
    };
    RangeParse::Partial(start, end)
}

fn parse_range(header: Option<&str>, total: u64) -> Option<(u64, u64)> {
    match parse_range_typed(header, total) {
        RangeParse::Whole => Some((0, total.saturating_sub(1))),
        RangeParse::Partial(start, end) => Some((start, end)),
        RangeParse::Unsatisfiable => None,
    }
}

#[tauri::command]
pub async fn hermes_ws_open(
    window: tauri::WebviewWindow,
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
        window.label().to_owned(),
    )
    .await
}

#[tauri::command]
pub fn hermes_ws_send(
    registry: tauri::State<'_, std::sync::Arc<crate::gateway_ws::SocketRegistry>>,
    id: u64,
    data: Value,
    binary: Option<bool>,
) -> Result<(), String> {
    crate::gateway_ws::send(registry.inner(), id, data, binary.unwrap_or(false))
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
    let owner =
        resolve_native_async(state.home.clone(), json!({"profile":params.get("profile")})).await?;
    let ticket = owner.mint("interactive")?;
    let budget = std::time::Duration::from_millis(timeout_ms.unwrap_or(20_000));
    crate::task_ops::call(&owner.connection, &ticket, &method, params, budget).await
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
    let owner = resolve_native_async(state.home.clone(), json!({"profile":profile})).await?;
    let ticket = owner.mint("interactive")?;
    let budget = std::time::Duration::from_millis(timeout_ms.unwrap_or(30_000));
    crate::task_ops::submit(
        &owner.connection,
        &ticket,
        &session_id,
        &profile,
        &text,
        budget,
    )
    .await
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

/// Milliseconds from process start to the moment WebKit reports the main
/// document finished, so a performance run measures the shell's own startup
/// rather than guessing from outside. Only written for a measure launch.
pub fn stamp_first_paint() {
    if std::env::var("HERMES_MEASURE_STARTUP").is_err() {
        return;
    }
    let elapsed = STARTED.get().map(|at| at.elapsed().as_millis()).unwrap_or(0);
    let path = std::env::temp_dir().join("hermes-rust-first-paint.json");
    let _ = std::fs::write(&path, serde_json::to_vec(&serde_json::json!({
        "firstPaintMs": elapsed,
        "pid": std::process::id()
    })).unwrap_or_default());
}

/// Process start reference for `stamp_first_paint`. Set once at the top of
/// `main` — a lazy lock would start its clock at the first *read*, which is the
/// very moment being measured.
pub static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

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
    let _ = std::fs::write(
        &path,
        serde_json::to_vec_pretty(&payload).unwrap_or_default(),
    );
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
            let owner = resolve_native_async(state.home.clone(), arg(&args, 0)).await?;
            Ok(serde_json::to_value(owner.connection).map_err(|e| e.to_string())?)
        }

        "hermes:connection:revalidate" => {
            let rebuilt = resolve_native_async(state.home.clone(), arg(&args, 0))
                .await
                .is_ok();
            Ok(json!({ "ok": rebuilt, "rebuilt": rebuilt }))
        }

        "hermes:gateway:ws-url" | "hermes:gateway:ws-url-for" => {
            let owner = resolve_native_async(state.home.clone(), arg(&args, 0)).await?;
            // The renderer turns `?native_dial=1&ticket=…` into the
            // `hermes-gateway-ticket.*` subprotocol itself (canonical-protocol.ts).
            let ticket = owner.mint("interactive")?;
            let sep = if owner.connection.ws_url.contains('?') {
                '&'
            } else {
                '?'
            };
            Ok(json!({
                "ok": true,
                "wsUrl": format!("{}{}native_dial=1&ticket={}", owner.connection.ws_url, sep, ticket)
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
            let request: HermesApiRequest = serde_json::from_value(arg(&args, 0))
                .map_err(|e| format!("bad HermesApiRequest: {e}"))?;
            let home = state.home.clone();
            tauri::async_runtime::spawn_blocking(move || owned_api_call(&home, request))
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
            let max_mb = arg(&args, 0)
                .as_u64()
                .unwrap_or(fs_ops::DATA_URL_DEFAULT_MAX_MB);
            fs_ops::set_data_url_read_max_mb(&state.home, max_mb)
        }

        "hermes:fs:readDir" => Ok(fs_ops::read_dir(arg(&args, 0).as_str().unwrap_or_default())),

        "hermes:fs:gitRoot" => Ok(fs_ops::git_root(arg(&args, 0).as_str().unwrap_or_default())),

        "hermes:fs:reveal" => Ok(Value::Bool(fs_ops::reveal(
            arg(&args, 0).as_str().unwrap_or_default(),
        ))),

        "hermes:fs:openDir" => Ok(fs_ops::open_dir(arg(&args, 0).as_str().unwrap_or_default())),

        "hermes:fs:desktopPluginsRoot" => {
            Ok(Value::String(fs_ops::desktop_plugins_root(&state.home)))
        }

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
                "hermes:git:review:stage" => {
                    git_ops::stage(repo.as_str().unwrap_or_default(), file.as_deref())
                }
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
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
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
            Ok(json!({ "level": (percent / 100.0).ln() / 1.2_f64.ln(), "percent": percent }))
        }

        "hermes:zoom:set-percent" => {
            let percent = arg(&args, 0).as_f64().unwrap_or(100.0);
            window.set_zoom((percent / 100.0).clamp(0.5, 2.0)).map_err(|e| e.to_string())?;
            let result = system_ops::zoom_state(&state.zoom, percent);
            let percent = result["percent"].as_f64().unwrap_or(100.0);
            let payload = json!({ "level": (percent / 100.0).ln() / 1.2_f64.ln(), "percent": percent });
            use tauri::Emitter;
            window.emit("hermes:zoom:changed", &payload).map_err(|e| e.to_string())?;
            Ok(payload)
        }

        "hermes:keep-awake" => {
            let mode = arg(&args, 0).as_str().unwrap_or_default().to_string();
            Ok(Value::Bool(state.keep_awake.set(&mode)))
        }

        "hermes:notify" => Ok(Value::Bool(system_ops::notify(&arg(&args, 0)))),

        // --- desktop surface the renderer calls directly --------------------
        "hermes:openExternal" => {
            let url = arg(&args, 0).as_str().unwrap_or_default().to_string();
            tauri::async_runtime::spawn_blocking(move || crate::desktop_ops::open_external(&url))
                .await
                .map_err(|e| e.to_string())?
        }

        "hermes:machine:profile" => Ok(crate::desktop_ops::machine_profile()),

        "hermes:ssh-config:resolve" => {
            let host = arg(&args, 0).as_str().unwrap_or_default().to_string();
            tauri::async_runtime::spawn_blocking(move || {
                crate::desktop_ops::ssh_resolve_host(&host)
            })
            .await
            .map_err(|e| e.to_string())
        }

        "hermes:setting:defaultProjectDir:get" => {
            Ok(crate::desktop_ops::get_default_project_dir(&state.home))
        }

        "hermes:setting:defaultProjectDir:set" => crate::desktop_ops::set_default_project_dir(
            &state.home,
            arg(&args, 0).as_str().unwrap_or_default(),
        ),

        "hermes:setting:defaultProjectDir:pick" => {
            crate::desktop_ops::pick_default_project_dir(&state.home)
        }

        "hermes:saveGatewayFile" => {
            let home = state.home.clone();
            let payload = arg(&args, 0);
            tauri::async_runtime::spawn_blocking(move || {
                crate::desktop_ops::save_gateway_file(&home, &payload)
            })
            .await
            .map_err(|e| e.to_string())?
        }

        "hermes:saveImageFromUrl" => {
            let url = arg(&args, 0).as_str().unwrap_or_default().to_string();
            tauri::async_runtime::spawn_blocking(move || {
                crate::desktop_ops::save_image_from_url(&url)
            })
            .await
            .map_err(|e| e.to_string())?
        }

        "hermes:normalizePreviewTarget" => {
            let target = arg(&args, 0).as_str().unwrap_or_default().to_string();
            let base = arg(&args, 1).as_str().map(str::to_string);
            tauri::async_runtime::spawn_blocking(move || {
                crate::desktop_ops::normalize_preview_target(&target, base.as_deref())
            })
            .await
            .map_err(|e| e.to_string())
        }

        "hermes:watchPreviewFile" => {
            let url = arg(&args, 0).as_str().unwrap_or_default().to_string();
            tauri::async_runtime::spawn_blocking(move || {
                crate::desktop_ops::watch_preview_file(&url)
            })
            .await
            .map_err(|e| e.to_string())
        }

        // Bootstrap repair is an installer concern; this shell attaches to a
        // gateway it did not install, so there is nothing to repair.
        "hermes:bootstrap:repair" => Ok(json!({ "ok": true, "repaired": false })),

        "hermes:prepared-submissions:read" => Ok(json!({ "submissions": [] })),

        "hermes:vscode-theme:fetch" | "hermes:vscode-theme:search" => {
            Ok(json!({ "ok": false, "error": "theme marketplace is not available in the Rust shell" }))
        }

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
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&reports).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_media(name: &str, bytes: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hermes-media-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn uri_for(path: &Path) -> String {
        format!(
            "hermes-media://stream/{}",
            urlencoding::encode(&path.to_string_lossy())
        )
    }

    #[test]
    fn a_whole_file_is_served_without_a_range_and_offers_ranges() {
        let path = write_media("whole.txt", b"0123456789");
        let response = media_response(&uri_for(&path), None);

        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["accept-ranges"], "bytes");
        assert_eq!(response.body().as_slice(), b"0123456789");
    }

    #[test]
    fn a_bounded_range_answers_206_with_the_exact_bytes() {
        let path = write_media("bounded.txt", b"0123456789");
        let response = media_response(&uri_for(&path), Some("bytes=2-5"));

        assert_eq!(response.status(), 206);
        assert_eq!(response.headers()["content-range"], "bytes 2-5/10");
        assert_eq!(response.headers()["content-length"], "4");
        assert_eq!(response.body().as_slice(), b"2345");
    }

    #[test]
    fn an_open_ended_range_runs_to_the_end() {
        let path = write_media("open.txt", b"0123456789");
        let response = media_response(&uri_for(&path), Some("bytes=7-"));

        assert_eq!(response.status(), 206);
        assert_eq!(response.body().as_slice(), b"789");
    }

    #[test]
    fn a_suffix_range_serves_the_tail() {
        let path = write_media("suffix.txt", b"0123456789");
        let response = media_response(&uri_for(&path), Some("bytes=-3"));

        assert_eq!(response.status(), 206);
        assert_eq!(response.headers()["content-range"], "bytes 7-9/10");
        assert_eq!(response.body().as_slice(), b"789");
    }

    #[test]
    fn a_range_past_the_end_is_416_not_the_whole_file() {
        let path = write_media("past.txt", b"0123456789");
        let response = media_response(&uri_for(&path), Some("bytes=99-"));

        assert_eq!(response.status(), 416);
        assert_eq!(response.headers()["content-range"], "bytes */10");
        assert!(response.body().is_empty() || response.body().as_slice() == b"range not satisfiable");
    }

    #[test]
    fn a_reversed_or_malformed_range_is_refused() {
        let path = write_media("malformed.txt", b"0123456789");
        let uri = uri_for(&path);

        assert_eq!(media_response(&uri, Some("bytes=5-2")).status(), 416);
        assert_eq!(media_response(&uri, Some("bytes=abc")).status(), 416);
        assert_eq!(media_response(&uri, Some("bytes=-0")).status(), 416);
        assert_eq!(media_response(&uri, Some("bytes=")).status(), 416);
    }

    #[test]
    fn an_end_past_the_file_is_clamped_not_widened() {
        let path = write_media("clamp.txt", b"0123456789");
        let response = media_response(&uri_for(&path), Some("bytes=8-9999"));

        assert_eq!(response.status(), 206);
        assert_eq!(response.headers()["content-range"], "bytes 8-9/10");
        assert_eq!(response.body().as_slice(), b"89");
    }

    #[test]
    fn a_missing_file_is_404_and_a_bad_host_is_501() {
        assert_eq!(
            media_response("hermes-media://stream/%2Fdefinitely%2Fnot%2Fhere", None).status(),
            404
        );
        assert_eq!(media_response("hermes-media://remote/x", None).status(), 501);
    }
}
