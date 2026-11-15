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

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

#[derive(Default)]
pub struct SocketRegistry {
    sockets: Mutex<HashMap<u64, (String, mpsc::UnboundedSender<Message>)>>,
    next_id: Mutex<u64>,
    closed_windows: Mutex<HashSet<String>>,
    shutdown: Mutex<HashMap<String, watch::Sender<bool>>>,
}

impl SocketRegistry {
    fn insert(
        &self,
        sender: mpsc::UnboundedSender<Message>,
        target: String,
    ) -> Result<u64, String> {
        let closed = self.closed_windows.lock().unwrap();
        if closed.contains(&target) {
            return Err("Socket owner window closed".into());
        }
        let mut next = self.next_id.lock().unwrap();
        *next += 1;
        let id = *next;
        self.sockets.lock().unwrap().insert(id, (target, sender));
        Ok(id)
    }

    fn subscribe(&self, target: &str) -> Result<watch::Receiver<bool>, String> {
        let closed = self.closed_windows.lock().unwrap();
        if closed.contains(target) {
            return Err("Socket owner window closed".into());
        }
        Ok(self
            .shutdown
            .lock()
            .unwrap()
            .entry(target.into())
            .or_insert_with(|| watch::channel(false).0)
            .subscribe())
    }

    fn with<F: FnOnce(&mpsc::UnboundedSender<Message>) -> bool>(&self, id: u64, f: F) -> bool {
        match self.sockets.lock().unwrap().get(&id) {
            Some((_, sender)) => f(sender),
            None => false,
        }
    }

    fn owns(&self, target: &str, id: u64) -> bool {
        self.sockets
            .lock()
            .unwrap()
            .get(&id)
            .is_some_and(|(owner, _)| owner == target)
    }

    pub fn close_window(&self, target: &str) {
        let mut closed = self.closed_windows.lock().unwrap();
        closed.insert(target.into());
        if let Some(shutdown) = self.shutdown.lock().unwrap().remove(target) {
            let _ = shutdown.send(true);
        }
        self.sockets.lock().unwrap().retain(|_, (owner, sender)| {
            if owner == target {
                let _ = sender.send(Message::Close(None));
                false
            } else {
                true
            }
        });
    }

    fn remove(&self, id: u64) {
        self.sockets.lock().unwrap().remove(&id);
    }
}

async fn dial(
    registry: std::sync::Arc<SocketRegistry>,
    url: String,
    protocols: Vec<String>,
    ticket: Option<String>,
    target: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    String,
> {
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|e| format!("bad ws url: {e}"))?;
    if let Some(ticket) = ticket {
        request.headers_mut().insert(
            "X-Hermes-Gateway-Ticket",
            ticket.parse().map_err(|_| "Invalid native grant")?,
        );
    }
    if !protocols.is_empty() {
        let joined = protocols.join(", ");
        request.headers_mut().insert(
            "sec-websocket-protocol",
            joined.parse().map_err(|e| format!("protocols: {e}"))?,
        );
    }

    let mut cancelled = registry.subscribe(target)?;
    tokio::select! {
        _ = cancelled.changed() => Err("Socket owner window closed".into()),
        result = tokio::time::timeout(std::time::Duration::from_secs(15), tokio_tungstenite::connect_async(request)) => {
            result.map_err(|_| "Gateway socket handshake timed out".to_string())?.map(|(stream, _)| stream).map_err(|e| format!("Gateway socket handshake failed: {e}"))
        }
    }
}

/// Dial the gateway and pump it into the window as `hermes-ws-*` events.
pub async fn open(
    app: AppHandle,
    registry: std::sync::Arc<SocketRegistry>,
    url: String,
    protocols: Vec<String>,
    ticket: Option<String>,
    target: String,
) -> Result<u64, String> {
    let stream = dial(registry.clone(), url, protocols, ticket, &target).await?;
    let (mut sink, mut source) = stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let id = registry.insert(tx, target.clone())?;
    let mut writer_cancelled = registry.subscribe(&target)?;
    let mut reader_cancelled = writer_cancelled.clone();

    // Outbound: IPC → gateway, cancellable even while the network is stalled.
    tokio::spawn(async move {
        loop {
            let message = tokio::select! {
                _ = writer_cancelled.changed() => break,
                message = rx.recv() => match message { Some(message) => message, None => break }
            };
            tokio::select! {
                _ = writer_cancelled.changed() => break,
                result = sink.send(message) => if result.is_err() { break; }
            }
        }
    });

    // Inbound: gateway → IPC events.
    let registry_clone = registry.clone();
    tokio::spawn(async move {
        let app_in = app.clone();
        loop {
            let next = tokio::select! {
                _ = reader_cancelled.changed() => break,
                next = source.next() => match next { Some(next) => next, None => break }
            };
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

pub fn send(
    registry: &SocketRegistry,
    target: &str,
    id: u64,
    data: Value,
    binary: bool,
) -> Result<(), String> {
    if !registry.owns(target, id) {
        return Err("socket is not owned by this window".into());
    }
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

pub fn close(registry: &SocketRegistry, target: &str, id: u64) -> bool {
    if !registry.owns(target, id) {
        return false;
    }
    let closed = registry.with(id, |sender| sender.send(Message::Close(None)).is_ok());
    registry.remove(id);
    closed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn destroying_window_cancels_a_pending_handshake() {
        use tokio::io::AsyncReadExt;
        let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/api/ws", server.local_addr().unwrap());
        let registry = std::sync::Arc::new(SocketRegistry::default());
        let client_registry = registry.clone();
        let client =
            tokio::spawn(
                async move { dial(client_registry, url, Vec::new(), None, "peer-1").await },
            );
        let (mut socket, _) = server.accept().await.unwrap();
        let mut bytes = [0; 8192];
        assert!(socket.read(&mut bytes).await.unwrap() > 0);
        registry.close_window("peer-1");
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(2), client)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(2), socket.read(&mut bytes))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        assert!(registry.sockets.lock().unwrap().is_empty());
        let (tx, _) = mpsc::unbounded_channel();
        assert!(registry.insert(tx, "peer-1".into()).is_err());
    }

    #[test]
    fn another_window_cannot_send_or_close_an_owned_socket() {
        let registry = SocketRegistry::default();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let id = registry.insert(tx, "main".into()).unwrap();
        assert!(send(&registry, "peer-1", id, json!("wrong"), false).is_err());
        assert!(!close(&registry, "peer-1", id));
        assert!(rx.try_recv().is_err());
        send(&registry, "main", id, json!("right"), false).unwrap();
        assert_eq!(rx.try_recv().unwrap(), Message::Text("right".into()));
        registry.close_window("main");
        assert_eq!(rx.try_recv().unwrap(), Message::Close(None));
        assert!(send(&registry, "main", id, json!("late"), false).is_err());
    }

    #[test]
    fn binary_audio_survives_both_ipc_directions() {
        let registry = SocketRegistry::default();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let id = registry.insert(tx, "main".into()).unwrap();
        send(&registry, "main", id, json!([0, 128, 255]), true).unwrap();
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
        let id = registry.insert(tx, "main".into()).unwrap();
        send(&registry, "main", id, json!("hello"), false).unwrap();
        assert_eq!(
            message_payload(id, rx.try_recv().unwrap()),
            Some(json!({"id": id, "data": "hello", "binary": false}))
        );
        assert!(send(&registry, "main", id, json!([256]), true).is_err());
        assert!(rx.try_recv().is_err());
        assert!(send(&registry, "main", id + 1, json!("hello"), false).is_err());
    }
}
