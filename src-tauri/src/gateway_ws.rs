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
use serde_json::json;
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
) -> Result<u64, String> {
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|e| format!("bad ws url: {e}"))?;
    if !protocols.is_empty() {
        let joined = protocols.join(", ");
        request
            .headers_mut()
            .insert("sec-websocket-protocol", joined.parse().map_err(|e| format!("protocols: {e}"))?);
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
                Ok(Message::Text(text)) => {
                    let _ = app_in.emit_to(
                        "main",
                        "hermes-ws-message",
                        json!({ "id": id, "data": text.to_string() }),
                    );
                }
                Ok(Message::Binary(bytes)) => {
                    let _ = app_in.emit_to(
                        "main",
                        "hermes-ws-message",
                        json!({ "id": id, "data": String::from_utf8_lossy(&bytes) }),
                    );
                }
                Ok(Message::Ping(payload)) => {
                    let _ = registry_clone.with(id, |sender| {
                        sender.send(Message::Pong(payload.clone())).is_ok()
                    });
                }
                Ok(Message::Close(frame)) => {
                    let code = frame.as_ref().map(|f| u16::from(f.code)).unwrap_or(1005);
                    let reason = frame.as_ref().map(|f| f.reason.to_string()).unwrap_or_default();
                    let _ = app_in.emit_to(
                        "main",
                        "hermes-ws-closed",
                        json!({ "id": id, "code": code, "reason": reason }),
                    );
                    break;
                }
                Ok(_) => {}
                Err(error) => {
                    let _ = app_in.emit_to(
                        "main",
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

pub fn send(registry: &SocketRegistry, id: u64, data: &str) -> bool {
    registry.with(id, |sender| sender.send(Message::Text(data.to_string())).is_ok())
}

pub fn close(registry: &SocketRegistry, id: u64) -> bool {
    let closed = registry.with(id, |sender| sender.send(Message::Close(None)).is_ok());
    registry.remove(id);
    closed
}
