//! One-shot, window-owned loopback callbacks. Tokens are redeemed by Hermes,
//! which verifies state; this listener only returns the redirect parameters.
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{oneshot, watch},
};
struct Entry {
    owner: String,
    created: Instant,
    result: watch::Receiver<Option<Value>>,
    stop: oneshot::Sender<()>,
}
#[derive(Default)]
pub struct CallbackRegistry {
    entries: Mutex<HashMap<String, Entry>>,
    next: AtomicU64,
}
impl CallbackRegistry {
    pub async fn listen(self: Arc<Self>, owner: &str) -> Result<Value, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let id = self.next.fetch_add(1, Ordering::Relaxed).to_string();
        let (tx, rx) = watch::channel(None);
        let (stop, cancel) = oneshot::channel();
        {
            let mut entries = self.entries.lock().unwrap();
            entries.retain(|_, entry| entry.created.elapsed() < Duration::from_secs(900));
            if entries.len() >= 8 {
                return Err("Too many pending OAuth listeners".into());
            }
            entries.insert(
                id.clone(),
                Entry {
                    owner: owner.into(),
                    created: Instant::now(),
                    result: rx,
                    stop,
                },
            );
        }
        tokio::spawn(async move {
            let result = tokio::select! {
                _ = cancel => json!({"code":null,"state":null,"iss":null,"error":"cancelled"}),
                result = tokio::time::timeout(Duration::from_secs(900), receive_callback(listener)) => {
                    result.unwrap_or_else(|_| json!({"code":null,"state":null,"iss":null,"error":"timeout waiting for OAuth callback"}))
                }
            };
            let _ = tx.send(Some(result));
        });
        Ok(json!({"id":id,"redirectUri":format!("http://127.0.0.1:{port}/callback")}))
    }
    pub fn cancel(&self, owner: &str, id: &str) -> bool {
        let mut entries = self.entries.lock().unwrap();
        if !entries.get(id).is_some_and(|entry| entry.owner == owner) {
            return false;
        }
        if let Some(entry) = entries.remove(id) {
            let _ = entry.stop.send(());
        }
        true
    }
    pub fn close_window(&self, owner: &str) {
        self.entries
            .lock()
            .unwrap()
            .retain(|_, entry| entry.owner != owner);
    }
    pub async fn wait(&self, owner: &str, id: &str, timeout_ms: u64) -> Result<Value, String> {
        let mut rx = {
            let entries = self.entries.lock().unwrap();
            let entry = entries
                .get(id)
                .filter(|entry| entry.owner == owner)
                .ok_or("OAuth listener not found for this window")?;
            entry.result.clone()
        };
        let read = async {
            loop {
                if let Some(result) = rx.borrow_and_update().clone() {
                    return result;
                }
                if rx.changed().await.is_err() {
                    return json!({"code":null,"state":null,"iss":null,"error":"cancelled"});
                }
            }
        };
        let result = tokio::time::timeout(Duration::from_millis(timeout_ms.clamp(1000, 900_000)), read).await
            .unwrap_or_else(|_| json!({"code":null,"state":null,"iss":null,"error":"timeout waiting for OAuth callback"}));
        self.cancel(owner, id);
        Ok(result)
    }
}
async fn receive_callback(listener: TcpListener) -> Value {
    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            return json!({"code":null,"state":null,"iss":null,"error":"callback listener failed"});
        };
        let request = tokio::time::timeout(Duration::from_secs(2), async {
            let mut bytes = Vec::new();
            while bytes.len() < 8192 {
                let mut chunk = [0; 512];
                let size = stream.read(&mut chunk).await.ok()?;
                if size == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..size]);
                if bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                    break;
                }
            }
            String::from_utf8(bytes).ok()
        })
        .await
        .ok()
        .flatten();
        let url = request.as_deref().and_then(|request| {
            let mut parts = request.lines().next()?.split_whitespace();
            if parts.next()? != "GET" {
                return None;
            }
            let path = parts.next()?;
            if !path.starts_with('/') || path.starts_with("//") {
                return None;
            }
            tauri::Url::parse(&format!("http://127.0.0.1{path}")).ok()
        });
        let body = "<!doctype html><meta charset=utf-8><title>Hermes authorization</title><p>You can return to Hermes.</p>";
        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            stream.write_all(response.as_bytes()),
        )
        .await;
        let _ = stream.shutdown().await;
        if let Some(url) = url.filter(|url| url.path() == "/callback") {
            let params: HashMap<_, _> = url.query_pairs().into_owned().collect();
            if params.contains_key("code") || params.contains_key("error") {
                // Release the listening socket before publishing completion;
                // a completed async future may otherwise retain its captures.
                drop(listener);
                return json!({"code":params.get("code"),"state":params.get("state"),"iss":params.get("iss"),"error":params.get("error")});
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[tokio::test]
    async fn callback_survives_browser_noise_and_preserves_issuer_and_owner() {
        let registry = std::sync::Arc::new(CallbackRegistry::default());
        let listen = registry.clone().listen("main").await.unwrap();
        let id = listen["id"].as_str().unwrap();
        let uri = tauri::Url::parse(listen["redirectUri"].as_str().unwrap()).unwrap();
        assert!(registry.wait("peer-1", id, 1000).await.is_err());
        for path in [
            "/favicon.ico",
            "/callback?code=abc%2B123&state=expected&iss=https%3A%2F%2Fissuer.test",
        ] {
            let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", uri.port().unwrap()))
                .await
                .unwrap();
            stream
                .write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes())
                .await
                .unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).await.unwrap();
            assert!(String::from_utf8_lossy(&bytes).contains("200 OK"));
        }
        assert_eq!(
            registry.wait("main", id, 1000).await.unwrap(),
            serde_json::json!({"code":"abc+123","state":"expected","iss":"https://issuer.test","error":null})
        );
        // Result delivery and the OS's observable TCP teardown need not land
        // in the same scheduler turn. Require bounded release, not a sleep.
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if tokio::net::TcpStream::connect(("127.0.0.1", uri.port().unwrap()))
                    .await
                    .is_err()
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(registry.entries.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn cancellation_and_window_close_release_listeners() {
        let registry = std::sync::Arc::new(CallbackRegistry::default());
        let a = registry.clone().listen("main").await.unwrap();
        let id = a["id"].as_str().unwrap();
        assert!(!registry.cancel("peer-1", id));
        assert!(registry.cancel("main", id));
        assert!(registry.wait("main", id, 1000).await.is_err());
        registry.clone().listen("peer-1").await.unwrap();
        registry.close_window("peer-1");
        assert!(registry.entries.lock().unwrap().is_empty());
    }
}
