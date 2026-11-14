//! Gateway WebSocket, owned by Rust.
//!
//! Why this exists: on macOS Tauri serves the renderer from `tauri://localhost`,
//! and WebKit's App Transport Security refuses every loopback `ws://`/`http://`
//! dial from that origin (verified: `fetch http://127.0.0.1:<port>` → "Load
//! failed", WS → close 1006, while the same handshake succeeds from Node).
//! Electron never hit this because its pages are `file://` with the dial done in
//! the main process.
//!
//! So the shell dials the gateway itself and gives the renderer a WebSocket
//! lookalike over IPC. Tickets stay in Rust: the renderer asks for a URL, the
//! shell mints and dials.

use std::collections::HashMap;
use std::sync::Mutex;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

#[derive(Default)]
pub struct SocketRegistry {
    sockets: Mutex<HashMap<u64, mpsc::UnboundedSender<Message>>>,
    next_id: Mutex<u64>,
}

impl SocketRegistry {
    fn insert(&self, sender: mpsc::UnboundedSender<Message>) -> u64 {
        let mut next = self.next_id.lock().unwrap();
        *next += 1;
        let id = *next;
        self.sockets.lock().unwrap().insert(id, sender);
        id
    }

    fn with<F: FnOnce(&mpsc::UnboundedSender<Message>) -> bool>(&self, id: u64, f: F) -> bool {
        match self.sockets.lock().unwrap().get(&id) {
            Some(sender) => f(sender),
            None => false,
        }
    }

    fn remove(&self, id: u64) {
        self.sockets.lock().unwrap().remove(&id);
    }
}

/// Dial the gateway and pump it into the window as `hermes-ws-*` events.
pub async fn open(
    app: AppHandle,
    registry: std::sync::Arc<SocketRegistry>,
    url: String,
    protocols: Vec<String>,
    target: String,
) -> Result<u64, String> {
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|e| format!("bad ws url: {e}"))?;
    if !protocols.is_empty() {
        let joined = protocols.join(", ");
        request.headers_mut().insert(
            "sec-websocket-protocol",
            joined.parse().map_err(|e| format!("protocols: {e}"))?,
        );
    }

    let (stream, _response) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| format!("dial {url}: {e}"))?;

    let (mut sink, mut source) = stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let id = registry.insert(tx);

    // Outbound: IPC → gateway.
    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sink.send(message).await.is_err() {
                break;
            }
        }
    });

    // Inbound: gateway → IPC events.
    let registry_clone = registry.clone();
    tokio::spawn(async move {
        let app_in = app.clone();
        while let Some(next) = source.next().await {
            match next {
                Ok(frame @ (Message::Text(_) | Message::Binary(_))) => {
                    if let Some(payload) = message_payload(id, frame) {
                        let _ = app_in.emit_to(target.as_str(), "hermes-ws-message", payload);
                    }
                }
                Ok(Message::Ping(payload)) => {
                    let _ = registry_clone.with(id, |sender| {
                        sender.send(Message::Pong(payload.clone())).is_ok()
                    });
                }
                Ok(Message::Close(frame)) => {
                    let code = frame.as_ref().map(|f| u16::from(f.code)).unwrap_or(1005);
                    let reason = frame
                        .as_ref()
                        .map(|f| f.reason.to_string())
                        .unwrap_or_default();
                    let _ = app_in.emit_to(
                        target.as_str(),
                        "hermes-ws-closed",
                        json!({ "id": id, "code": code, "reason": reason }),
                    );
                    break;
                }
                Ok(_) => {}
                Err(error) => {
                    let _ = app_in.emit_to(
                        target.as_str(),
                        "hermes-ws-closed",
                        json!({ "id": id, "code": 1006, "reason": error.to_string() }),
                    );
                    break;
                }
            }
        }
        registry_clone.remove(id);
    });

    Ok(id)
}

fn message_payload(id: u64, frame: Message) -> Option<Value> {
    match frame {
        Message::Text(text) => Some(json!({"id": id, "data": text, "binary": false})),
        Message::Binary(bytes) => Some(json!({"id": id, "data": bytes, "binary": true})),
        _ => None,
    }
}

pub fn send(registry: &SocketRegistry, id: u64, data: Value, binary: bool) -> Result<(), String> {
    let frame = if binary {
        let bytes: Vec<u8> = serde_json::from_value(data).map_err(|_| "invalid binary frame")?;
        Message::Binary(bytes)
    } else {
        Message::Text(data.as_str().ok_or("invalid text frame")?.to_owned())
    };
    if registry.with(id, |sender| sender.send(frame).is_ok()) {
        Ok(())
    } else {
        Err("socket is not open".into())
    }
}

pub fn close(registry: &SocketRegistry, id: u64) -> bool {
    let closed = registry.with(id, |sender| sender.send(Message::Close(None)).is_ok());
    registry.remove(id);
    closed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_audio_survives_both_ipc_directions() {
        let registry = SocketRegistry::default();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let id = registry.insert(tx);
        send(&registry, id, json!([0, 128, 255]), true).unwrap();
        let frame = rx.try_recv().unwrap();
        assert_eq!(frame, Message::Binary(vec![0, 128, 255]));
        assert_eq!(
            message_payload(id, frame),
            Some(json!({
                "id": id, "data": [0, 128, 255], "binary": true
            }))
        );
    }

    #[test]
    fn text_remains_text_and_invalid_binary_is_rejected() {
        let registry = SocketRegistry::default();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let id = registry.insert(tx);
        send(&registry, id, json!("hello"), false).unwrap();
        assert_eq!(
            message_payload(id, rx.try_recv().unwrap()),
            Some(json!({"id": id, "data": "hello", "binary": false}))
        );
        assert!(send(&registry, id, json!([256]), true).is_err());
        assert!(rx.try_recv().is_err());
        assert!(send(&registry, id + 1, json!("hello"), false).is_err());
    }
}
