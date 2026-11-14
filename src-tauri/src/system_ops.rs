//! Small system surfaces: `hermes:logs:*`, keep-awake, native notifications and
//! window zoom. Each one replaces a small piece of Electron main.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Mutex;

use serde_json::{json, Value};

pub fn logs_dir(home: &Path) -> PathBuf {
    home.join("logs")
}

pub fn desktop_log_path(home: &Path) -> PathBuf {
    logs_dir(home).join("desktop.log")
}

/// Append one renderer line (error-boundary catches and `logLine` chatter).
pub fn append_log(home: &Path, line: &str) {
    let path = desktop_log_path(home);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{line}");
    }
}

/// `hermes:logs:recent` — the tail the Settings → Logs pane renders.
pub fn recent_logs(home: &Path, limit: usize) -> Value {
    let raw = std::fs::read_to_string(desktop_log_path(home)).unwrap_or_default();
    let lines: Vec<&str> = raw.lines().collect();
    let start = lines.len().saturating_sub(limit);
    Value::Array(
        lines[start..]
            .iter()
            .map(|line| Value::String((*line).to_string()))
            .collect(),
    )
}

/// `hermes:logs:reveal`.
pub fn reveal_logs(home: &Path) -> bool {
    let dir = logs_dir(home);
    let _ = std::fs::create_dir_all(&dir);
    Command::new("open")
        .arg(&dir)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Keep-awake: `caffeinate` while a mode is set, killed when it goes off.
#[derive(Default)]
pub struct KeepAwake {
    child: Mutex<Option<Child>>,
}

impl KeepAwake {
    pub fn set(&self, mode: &str) -> bool {
        let mut current = self.child.lock().unwrap();
        if let Some(mut child) = current.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if mode.is_empty() || mode == "off" || mode == "false" {
            return false;
        }
        // -d display, -i idle, -m disk, -s system (on AC), -u user assertion.
        match Command::new("caffeinate").args(["-dimsu"]).spawn() {
            Ok(child) => {
                *current = Some(child);
                true
            }
            Err(_) => false,
        }
    }
}

/// `hermes:notify` — native notification through `osascript` so no extra
/// permission plugin is needed for the first cut.
pub fn notify(payload: &Value) -> bool {
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Hermes");
    let body = payload
        .get("body")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("message").and_then(|v| v.as_str()))
        .unwrap_or("");
    if body.is_empty() {
        return false;
    }
    let script = format!(
        "display notification {} with title {}",
        applescript_string(body),
        applescript_string(title)
    );
    Command::new("osascript")
        .args(["-e", &script])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn applescript_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Zoom state shared by `hermes:zoom:get` / `set-percent`.
#[derive(Default)]
pub struct Zoom(pub Mutex<Option<f64>>);

pub fn zoom_state(zoom: &Zoom, percent: f64) -> Value {
    let clamped = percent.clamp(50.0, 200.0);
    *zoom.0.lock().unwrap() = Some(clamped);
    json!({ "percent": clamped.round() as i64 })
}

pub fn zoom_get(zoom: &Zoom) -> Value {
    let percent = zoom.0.lock().unwrap().unwrap_or(100.0);
    json!({ "percent": percent.round() as i64 })
}
