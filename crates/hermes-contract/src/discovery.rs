//! Local backend discovery.
//!
//! Electron's local path attaches to the per-host gateway over a private
//! control channel and mints per-call REST grants. The Rust shell starts with
//! the same discovery primitive the CLI already exposes — `hermes gateway
//! attach` — which returns the owner's API origin plus a one-use WS ticket.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::types::{AttachReceipt, HermesConnection};

/// Where to find the `hermes` launcher inside a normal install.
pub fn default_launcher(home: &Path) -> PathBuf {
    home.join(".hermes/hermes-agent/.hermes/bin/hermes")
}

/// Resolve the launcher: `HERMES_BIN` wins, then the install-local shim, then
/// whatever `hermes` is on PATH.
pub fn launcher(home: &Path) -> PathBuf {
    if let Ok(explicit) = std::env::var("HERMES_BIN") {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
    let local = default_launcher(home);
    if local.is_file() {
        return local;
    }
    PathBuf::from("hermes")
}

/// Run `hermes gateway attach` and parse its single-line JSON verdict.
pub fn attach(home: &Path, timeout_s: u64) -> Result<AttachReceipt, String> {
    let exe = launcher(home);
    let out = Command::new(&exe)
        .arg("gateway")
        .arg("attach")
        .arg("--timeout")
        .arg(timeout_s.to_string())
        .env("HERMES_HOME", home)
        .output()
        .map_err(|e| format!("spawn {}: {e}", exe.display()))?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .ok_or_else(|| {
            format!(
                "gateway attach produced no JSON (exit {:?}): {}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr).trim()
            )
        })?;

    let receipt: AttachReceipt =
        serde_json::from_str(line).map_err(|e| format!("gateway attach JSON: {e}"))?;
    if receipt.state.as_deref().is_some_and(|s| s != "ready") {
        return Err(format!(
            "gateway not ready: {}",
            receipt.state.unwrap_or_default()
        ));
    }
    Ok(receipt)
}

/// Map a receipt onto the descriptor shape the renderer expects.
pub fn connection_from_receipt(receipt: &AttachReceipt) -> HermesConnection {
    HermesConnection {
        base_url: receipt.endpoint.api_origin.clone(),
        is_fullscreen: false,
        native_overlay_width: 0.0,
        token: receipt.ticket.clone().unwrap_or_default(),
        ws_url: receipt.url.clone(),
        logs: Vec::new(),
        custom_window_controls: Some(false),
        darwin_major: None,
        is_maximized: Some(false),
        mode: Some("local".into()),
        auth_mode: Some("native".into()),
        remote_host: None,
        remote_identity: None,
        remote_kind: None,
        remote_hermes_version: None,
        source: Some("local".into()),
        profile: None,
        connection_id: None,
        registry_scoped: false,
        shared_primary: false,
        shared_remote: false,
        window_button_position: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"client_code_sha":"3b42","detail":null,"endpoint":{"api_origin":"http://127.0.0.1:51024","authority_epoch":7,"capabilities":["durable-admission-v1"],"code_sha":"3b42","control_home":null,"instance_id":"2601672d","profile_id":"/home/u/.hermes","runtime_protocol":1,"supervisor":"launchd"},"protocol":"hermes-gateway-v1","protocols":["hermes-gateway-v1","hermes-gateway-ticket.ABC"],"reason_code":null,"state":"ready","ticket":"ABC","url":"ws://127.0.0.1:51024/api/ws"}"#;

    #[test]
    fn parses_attach_verdict() {
        let receipt: AttachReceipt = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(receipt.endpoint.api_origin, "http://127.0.0.1:51024");
        assert_eq!(receipt.state.as_deref(), Some("ready"));
        assert_eq!(receipt.ticket.as_deref(), Some("ABC"));
    }

    #[test]
    fn descriptor_matches_renderer_contract() {
        let receipt: AttachReceipt = serde_json::from_str(SAMPLE).unwrap();
        let conn = connection_from_receipt(&receipt);
        assert_eq!(conn.base_url, "http://127.0.0.1:51024");
        assert_eq!(conn.ws_url, "ws://127.0.0.1:51024/api/ws");
        assert_eq!(conn.auth_mode.as_deref(), Some("native"));
        assert_eq!(conn.mode.as_deref(), Some("local"));

        let wire = serde_json::to_value(&conn).unwrap();
        assert!(wire.get("baseUrl").is_some());
        assert!(wire.get("nativeOverlayWidth").is_some());
        assert!(wire.get("base_url").is_none());
        // false flags are omitted, matching the renderer's `=== true` tests.
        assert!(wire.get("registryScoped").is_none());
    }
}

/// Mint one gateway grant over the local control socket.
///
/// Wire protocol (ported from `apps/desktop/electron/local-gateway.ts`): one
/// newline-terminated JSON request, one newline-terminated JSON reply. Only
/// `interactive` tickets carry `scope: "host"`; REST grants are `native-http`.
pub fn mint_ticket(
    home: &Path,
    instance_id: &str,
    profile_id: &str,
    purpose: &str,
) -> Result<String, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let socket_path = home.join("gateway.sock");
    let mut stream = UnixStream::connect(&socket_path)
        .map_err(|e| format!("control socket {}: {e}", socket_path.display()))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;

    let mut params = serde_json::json!({
        "profile_id": profile_id,
        "instance_id": instance_id,
        "purpose": purpose,
    });
    if purpose == "interactive" {
        params["scope"] = serde_json::json!("host");
    }
    let request = serde_json::json!({
        "protocol": 1,
        "id": 1,
        "verb": "session-ticket",
        "params": params,
    });

    let mut line = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    line.push(b'\n');
    stream.write_all(&line).map_err(|e| e.to_string())?;

    let mut reply = String::new();
    BufReader::new(stream)
        .read_line(&mut reply)
        .map_err(|e| format!("control socket read: {e}"))?;

    let parsed: serde_json::Value =
        serde_json::from_str(reply.trim()).map_err(|e| format!("control reply: {e}"))?;
    if parsed.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        return Err(format!("gateway refused ticket: {}", parsed));
    }
    parsed
        .pointer("/result/ticket")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| "control reply carried no ticket".to_string())
}

#[cfg(test)]
mod ticket_tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;

    #[test]
    fn mints_over_the_control_socket() {
        let dir = std::env::temp_dir().join(format!("hermes-ticket-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("gateway.sock");
        let listener = UnixListener::bind(&sock).unwrap();

        let server = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut request = String::new();
            BufReader::new(conn.try_clone().unwrap())
                .read_line(&mut request)
                .unwrap();
            let value: serde_json::Value = serde_json::from_str(request.trim()).unwrap();
            assert_eq!(value["verb"], "session-ticket");
            assert_eq!(value["params"]["purpose"], "native-http");
            assert!(value["params"].get("scope").is_none());
            conn.write_all(b"{\"ok\":true,\"protocol\":1,\"id\":1,\"result\":{\"ticket\":\"T-1\"}}\n")
                .unwrap();
            request
        });

        let ticket = mint_ticket(&dir, "inst-1", "/home/u/.hermes", "native-http").unwrap();
        assert_eq!(ticket, "T-1");
        server.join().unwrap();
    }
}
