//! Additional app windows: a peer full-chrome window (`openWindow`) and a
//! single-session window (`openSessionWindow`).
//!
//! Electron could open both because each `BrowserWindow` carried its own
//! startup route. The Rust shell had one window, so "open in new window" and
//! ⌘⇧N silently did nothing — `canOpenNewWindow()` reads the bridge member, so
//! its absence removed the affordance rather than failing loudly. Each window
//! created here gets its own bootstrap script with its own route baked in, so
//! two windows never fight over one global route.

use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

/// Monotonic label source. Window labels must be unique for the app's lifetime,
/// and reusing "main" would make `get_webview_window` return the wrong one.
static NEXT_WINDOW: AtomicU64 = AtomicU64::new(1);

/// Only in-app hash routes may be opened this way: a peer window reviews a
/// screen, it does not browse an arbitrary URL with the shell's privileges.
fn route_from_payload(payload: &Value) -> Option<String> {
    let profile = payload
        .get("profile")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    let session = payload
        .get("sessionId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());

    if let Some(session) = session {
        // A session window is addressed by the session it shows, and keeps the
        // profile that owns it so two same-named sessions never collide.
        let mut route = format!("/?session={}", urlencoding::encode(session));
        if let Some(profile) = profile {
            route.push_str(&format!("&profile={}", urlencoding::encode(profile)));
        }
        if payload.get("watch").and_then(Value::as_bool) == Some(true) {
            route.push_str("&watch=1");
        }

        return Some(route);
    }

    profile.map(|profile| format!("/?profile={}", urlencoding::encode(profile)))
}

/// Open (or focus) a window for one session. `watch` opens a spectator window
/// that does not build an agent, which is what the subagent viewer uses.
pub fn open_session_window(
    app: &tauri::AppHandle,
    session_id: &str,
    opts: &Value,
) -> Result<Value, String> {
    let session_id = session_id.trim();
    if session_id.is_empty() || session_id.len() > 200 {
        return Ok(json!({ "ok": false, "error": "invalid session id" }));
    }
    // Session ids address a row, not a path: refuse separators and control
    // characters so a crafted id cannot rewrite the route it is embedded in.
    if session_id
        .chars()
        .any(|c| c.is_control() || c == '/' || c == '\\' || c == '\0')
    {
        return Ok(json!({ "ok": false, "error": "invalid session id" }));
    }

    let mut payload = opts.clone();
    if let Some(object) = payload.as_object_mut() {
        object.insert("sessionId".into(), Value::String(session_id.to_string()));
    }
    Ok(spawn_window(app, "session", payload))
}

/// Open a peer full-chrome window, optionally pinned to a connection/profile.
pub fn open_window(app: &tauri::AppHandle, options: &Value) -> Result<Value, String> {
    Ok(spawn_window(app, "peer", options.clone()))
}

fn spawn_window(app: &tauri::AppHandle, kind: &str, payload: Value) -> Value {
    let route = route_from_payload(&payload);
    let label = format!("{kind}-{}", NEXT_WINDOW.fetch_add(1, Ordering::Relaxed));
    let script = match crate::bootstrap_script_for(app) {
        Some(script) => script,
        None => return json!({ "ok": false, "error": "shell bootstrap unavailable" }),
    };

    let built = tauri::WebviewWindowBuilder::new(
        app,
        &label,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("Hermes")
    .inner_size(1180.0, 820.0)
    .min_inner_size(720.0, 480.0)
    .initialization_script(&script)
    .build();

    match built {
        Ok(window) => {
            if let Some(route) = route {
                let literal = serde_json::to_string(&route).unwrap_or_else(|_| "\"\"".into());
                let _ = window.eval(&format!("location.hash = {literal};"));
            }
            json!({ "ok": true })
        }
        Err(error) => json!({ "ok": false, "error": error.to_string() }),
    }
}
