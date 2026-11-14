//! Canonical JSON-RPC over the gateway WebSocket, for the shell's own windows.
//!
//! The renderer speaks this protocol through the injected `WebSocket` shim; the
//! workspace/task windows are shell-owned UI, so they get a direct Rust client
//! instead — one request, one socket, closed as soon as the answer arrives.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use hermes_contract::types::HermesConnection;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::client::Request as WsRequest;
use tokio_tungstenite::tungstenite::Message;

fn dial_request(conn: &HermesConnection, ticket: &str) -> Result<WsRequest, String> {
    let mut request = conn
        .ws_url
        .as_str()
        .into_client_request()
        .map_err(|e| format!("bad ws url: {e}"))?;
    // Both subprotocols go out together: the version tag and the ticket.
    let protocols = format!("hermes-gateway-v1, hermes-gateway-ticket.{ticket}");
    request.headers_mut().insert(
        "sec-websocket-protocol",
        protocols.parse().map_err(|_| "bad ticket".to_string())?,
    );
    Ok(request)
}

/// One request/response round-trip. Frames pushed by the gateway in between
/// (`{"method":"event"}`) are ignored: this client never subscribes.
pub async fn call(
    conn: &HermesConnection,
    ticket: &str,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, String> {
    let request = dial_request(conn, ticket)?;
    let (mut socket, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| format!("gateway ws: {e}"))?;

    let payload = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    socket
        .send(Message::Text(payload.to_string().into()))
        .await
        .map_err(|e| format!("send: {e}"))?;

    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let next = tokio::time::timeout_at(deadline, socket.next()).await;
        match next {
            Err(_) => {
                let _ = socket.close(None).await;
                return Err(format!("gateway timeout: {method}"));
            }
            Ok(None) => return Err(format!("gateway closed before answering {method}")),
            Ok(Some(Err(error))) => return Err(format!("gateway ws: {error}")),
            Ok(Some(Ok(Message::Text(text)))) => {
                let Ok(value) = serde_json::from_str::<Value>(&text) else { continue };
                if value.get("id") != Some(&json!(1)) {
                    continue;
                }
                let _ = socket.close(None).await;
                if let Some(error) = value.get("error") {
                    return Err(error.to_string());
                }
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
            Ok(Some(Ok(_))) => continue,
        }
    }
}

/// `prompt.submit` is an ATTACH-REQUIRED RPC: the gateway drops a submit that
/// arrives on a connection which never attached the session. So one socket does
/// `session.resume` first and then submits, instead of reusing a short-lived
/// per-call connection.
pub async fn submit(
    conn: &HermesConnection,
    ticket: &str,
    session_id: &str,
    profile: &str,
    text: &str,
    timeout: Duration,
) -> Result<Value, String> {
    let request = dial_request(conn, ticket)?;
    let (mut socket, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| format!("gateway ws: {e}"))?;

    let resume = json!({
        "jsonrpc": "2.0", "id": 1, "method": "session.resume",
        "params": { "session_id": session_id, "profile": profile, "source": "desktop" }
    });
    socket
        .send(Message::Text(resume.to_string().into()))
        .await
        .map_err(|e| format!("send resume: {e}"))?;

    let submit = json!({
        "jsonrpc": "2.0", "id": 2, "method": "prompt.submit",
        "params": {
            "session_id": session_id,
            "profile": profile,
            "text": text,
            "submission_id": uuid_like()
        }
    });

    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let next = tokio::time::timeout_at(deadline, socket.next()).await;
        match next {
            Err(_) => {
                let _ = socket.close(None).await;
                return Err("gateway timeout: prompt.submit".into());
            }
            Ok(None) => return Err("gateway closed before acknowledging prompt.submit".into()),
            Ok(Some(Err(error))) => return Err(format!("gateway ws: {error}")),
            Ok(Some(Ok(Message::Text(raw)))) => {
                let Ok(value) = serde_json::from_str::<Value>(&raw) else { continue };
                match value.get("id").and_then(|id| id.as_u64()) {
                    Some(1) => {
                        if let Some(error) = value.get("error") {
                            let _ = socket.close(None).await;
                            return Err(format!("session.resume failed: {error}"));
                        }
                        socket
                            .send(Message::Text(submit.to_string().into()))
                            .await
                            .map_err(|e| format!("send submit: {e}"))?;
                    }
                    Some(2) => {
                        let _ = socket.close(None).await;
                        if let Some(error) = value.get("error") {
                            return Err(format!("prompt.submit failed: {error}"));
                        }
                        return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                    }
                    _ => continue,
                }
            }
            Ok(Some(Ok(_))) => continue,
        }
    }
}

/// A v4-shaped identifier: the gateway only needs uniqueness, not a crypto RNG.
fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id() as u128;
    let mixed = nanos ^ (pid << 64) ^ (nanos >> 7);
    let hex = format!("{mixed:032x}");
    format!(
        "{}-{}-4{}-8{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[13..16],
        &hex[17..20],
        &hex[20..32]
    )
}
