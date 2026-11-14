//! Embedded terminal: the `hermes:terminal:*` surface, backed by a real PTY
//! (`portable-pty`, the Rust equivalent of node-pty).
//!
//! Event shapes match the preload exactly: `hermes:terminal:<id>:data` carries a
//! string, `hermes:terminal:<id>:exit` carries `{ code, signal }`.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

struct Session {
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    child: Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
    pid: Option<u32>,
}

#[derive(Default)]
pub struct TerminalState {
    sessions: Mutex<HashMap<String, Arc<Session>>>,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

fn is_executable(path: &str) -> bool {
    std::fs::metadata(path)
        .map(|meta| meta.is_file())
        .unwrap_or(false)
        && {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::metadata(path)
                    .map(|m| m.permissions().mode() & 0o111 != 0)
                    .unwrap_or(false)
            }
            #[cfg(not(unix))]
            {
                true
            }
        }
}

/// Shell resolution, ported: explicit override, then `$SHELL`, then the best
/// POSIX shell that exists.
fn shell_spec() -> (String, Vec<String>, String) {
    let override_path = std::env::var("HERMES_DESKTOP_SHELL").unwrap_or_default();
    let candidate = if !override_path.trim().is_empty() {
        override_path.trim().to_string()
    } else {
        std::env::var("SHELL").unwrap_or_default()
    };

    let shell = if is_executable(&candidate) {
        candidate
    } else {
        ["/bin/zsh", "/bin/bash", "/bin/sh"]
            .into_iter()
            .find(|path| is_executable(path))
            .unwrap_or("/bin/sh")
            .to_string()
    };

    let name = shell.rsplit('/').next().unwrap_or("sh").to_string();
    let args = if name.contains("zsh") || name.contains("bash") {
        vec!["-il".to_string()]
    } else {
        vec!["-i".to_string()]
    };
    (shell, args, name)
}

/// Env scrubbing: package-manager vars out, truecolor + a real locale in.
fn terminal_env() -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();
    env.retain(|key, _| {
        !(key.starts_with("npm_config_")
            || key.starts_with("npm_package_")
            || key.starts_with("pnpm_config_"))
    });
    for noisy in ["NO_COLOR", "FORCE_COLOR", "COLORFGBG"] {
        env.remove(noisy);
    }
    env.insert("COLORTERM".into(), "truecolor".into());
    env.insert(
        "LC_CTYPE".into(),
        std::env::var("LC_CTYPE").unwrap_or_else(|_| "UTF-8".into()),
    );
    env.insert("TERM".into(), "xterm-256color".into());
    env.insert("TERM_PROGRAM".into(), "Hermes".into());
    env.insert(
        "TERM_PROGRAM_VERSION".into(),
        env!("CARGO_PKG_VERSION").to_string(),
    );
    env.insert("HERMES_DESKTOP_TERMINAL".into(), "1".into());
    env
}

fn safe_cwd(requested: &str) -> PathBuf {
    let candidate = if requested.trim().is_empty() {
        home()
    } else {
        PathBuf::from(requested.trim())
    };
    match std::fs::metadata(&candidate) {
        Ok(meta) if meta.is_dir() => candidate,
        Ok(_) => candidate.parent().map(PathBuf::from).unwrap_or_else(home),
        Err(_) => home(),
    }
}

fn emit(app: &AppHandle, id: &str, suffix: &str, payload: Value) {
    let channel = format!("hermes:terminal:{id}:{suffix}");
    let _ = app.emit_to("main", channel.as_str(), payload);
}

/// `hermes:terminal:start` → `{ connectionId, cwd, id, shell }`.
pub fn start(
    app: AppHandle,
    state: &TerminalState,
    payload: &Value,
) -> Result<Value, String> {
    let (shell, args, shell_name) = shell_spec();
    let cwd = safe_cwd(payload.get("cwd").and_then(|v| v.as_str()).unwrap_or(""));
    let cols = payload
        .get("cols")
        .and_then(|v| v.as_u64())
        .unwrap_or(80)
        .max(2) as u16;
    let rows = payload
        .get("rows")
        .and_then(|v| v.as_u64())
        .unwrap_or(24)
        .max(2) as u16;

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("openpty: {e}"))?;

    let mut command = CommandBuilder::new(&shell);
    for arg in &args {
        command.arg(arg);
    }
    command.cwd(&cwd);
    for (key, value) in terminal_env() {
        command.env(key, value);
    }

    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|e| format!("spawn {shell}: {e}"))?;
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("pty reader: {e}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("pty writer: {e}"))?;
    let pid = child.process_id();

    let id = uuid_like();
    let session = Arc::new(Session {
        writer: Mutex::new(writer),
        master: Mutex::new(pair.master),
        child: Mutex::new(child),
        pid,
    });
    state
        .sessions
        .lock()
        .unwrap()
        .insert(id.clone(), session.clone());

    // Reader thread: PTY bytes → renderer events, exit event on EOF.
    let read_id = id.clone();
    let read_app = app.clone();
    let read_session = session.clone();
    std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buffer[..n]).to_string();
                    emit(&read_app, &read_id, "data", Value::String(chunk));
                }
                Err(_) => break,
            }
        }
        let code = read_session
            .child
            .lock()
            .unwrap()
            .try_wait()
            .ok()
            .flatten()
            .and_then(|status| status.exit_code().into());
        emit(
            &read_app,
            &read_id,
            "exit",
            json!({ "code": code, "signal": Value::Null }),
        );
    });

    Ok(json!({
        "connectionId": "local",
        "cwd": cwd.to_string_lossy(),
        "id": id,
        "shell": shell_name
    }))
}

pub fn attach(state: &TerminalState, id: &str) -> bool {
    state.sessions.lock().unwrap().contains_key(id)
}

pub fn write(state: &TerminalState, id: &str, data: &str) -> bool {
    let session = state.sessions.lock().unwrap().get(id).cloned();
    match session {
        Some(session) => session
            .writer
            .lock()
            .unwrap()
            .write_all(data.as_bytes())
            .is_ok(),
        None => false,
    }
}

pub fn resize(state: &TerminalState, id: &str, cols: u64, rows: u64) -> bool {
    let session = state.sessions.lock().unwrap().get(id).cloned();
    match session {
        Some(session) => session
            .master
            .lock()
            .unwrap()
            .resize(PtySize {
                rows: rows.max(2) as u16,
                cols: cols.max(2) as u16,
                pixel_width: 0,
                pixel_height: 0,
            })
            .is_ok(),
        None => false,
    }
}

/// `lsof -a -p <pid> -d cwd -Fn` — the same probe the Electron host uses on
/// macOS to learn where a reopened tab should start.
pub fn cwd(state: &TerminalState, id: &str) -> Option<String> {
    let session = state.sessions.lock().unwrap().get(id).cloned()?;
    let pid = session.pid?;
    let output = std::process::Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .find(|line| line.starts_with('n'))
        .map(|line| line[1..].to_string())
}

pub fn dispose(state: &TerminalState, id: &str) -> bool {
    let session = state.sessions.lock().unwrap().remove(id);
    match session {
        Some(session) => {
            let _ = session.child.lock().unwrap().kill();
            true
        }
        None => false,
    }
}

fn uuid_like() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("t{now:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_spec_prefers_explicit_shells() {
        let (shell, args, name) = shell_spec();
        assert!(shell.starts_with('/'), "shell must be absolute: {shell}");
        assert!(!args.is_empty());
        assert!(!name.is_empty());
    }

    #[test]
    fn env_scrubbing_keeps_colour_and_drops_package_manager_state() {
        std::env::set_var("npm_config_registry", "https://example.invalid");
        std::env::set_var("NO_COLOR", "1");
        let env = terminal_env();
        assert!(!env.contains_key("npm_config_registry"));
        assert!(!env.contains_key("NO_COLOR"));
        assert_eq!(env.get("COLORTERM").map(String::as_str), Some("truecolor"));
        assert_eq!(env.get("TERM").map(String::as_str), Some("xterm-256color"));
        assert_eq!(env.get("HERMES_DESKTOP_TERMINAL").map(String::as_str), Some("1"));
    }

    #[test]
    fn pty_runs_a_command_and_returns_its_output() {
        use portable_pty::{native_pty_system, CommandBuilder, PtySize};

        let pair = native_pty_system()
            .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
            .unwrap();
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "printf 'pty-ok\\n'"]);
        let mut child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut output = String::new();
        let mut buffer = [0u8; 256];
        while let Ok(n) = reader.read(&mut buffer) {
            if n == 0 {
                break;
            }
            output.push_str(&String::from_utf8_lossy(&buffer[..n]));
            if output.contains("pty-ok") {
                break;
            }
        }
        let _ = child.kill();
        assert!(output.contains("pty-ok"), "pty produced: {output:?}");
    }
}
