//! Window-owned file watches with bounded stat polling, including atomic replacement.
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::SystemTime,
};
use tokio::sync::oneshot;
struct Entry {
    owner: String,
    _stop: oneshot::Sender<()>,
}
#[derive(Default)]
pub struct PreviewWatches {
    entries: Mutex<HashMap<String, Entry>>,
    next: AtomicU64,
}
fn preview_path(raw: &str) -> Result<PathBuf, String> {
    if raw.starts_with("file:") {
        return tauri::Url::parse(raw)
            .map_err(|e| e.to_string())?
            .to_file_path()
            .map_err(|_| "Invalid local file URL".into());
    }
    let path = crate::fs_ops::expand(raw);
    if !path.is_absolute() {
        return Err("Preview watch needs an absolute local file path".into());
    }
    Ok(path)
}
fn snapshot(path: &std::path::Path) -> Option<(u64, SystemTime, u64)> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path)
        .ok()
        .filter(|meta| meta.is_file())
        .map(|meta| {
            (
                meta.len(),
                meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                meta.ino(),
            )
        })
}
impl PreviewWatches {
    pub fn start(
        &self,
        owner: &str,
        raw: &str,
        emit: impl Fn(Value) + Send + 'static,
    ) -> Result<Value, String> {
        let path = preview_path(raw)?;
        let mut previous = snapshot(&path).ok_or("Preview file is missing or is not a file")?;
        let id = format!("preview-{}", self.next.fetch_add(1, Ordering::Relaxed));
        let url = tauri::Url::from_file_path(&path)
            .map_err(|_| "Invalid preview path")?
            .to_string();
        let (stop, mut cancelled) = oneshot::channel();
        {
            let mut entries = self.entries.lock().unwrap();
            if entries
                .values()
                .filter(|entry| entry.owner == owner)
                .count()
                >= 64
            {
                return Err("Too many preview watches in this window".into());
            }
            entries.insert(
                id.clone(),
                Entry {
                    owner: owner.into(),
                    _stop: stop,
                },
            );
        }
        let event = json!({"id":id,"path":path,"url":url});
        let reply = json!({"id":id,"path":path});
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(std::time::Duration::from_millis(500));
            loop {
                tokio::select! {
                    _ = &mut cancelled => break,
                    _ = timer.tick() => {
                        if let Some(current) = snapshot(&path) {
                            if current != previous { previous = current; emit(event.clone()); }
                        }
                    }
                }
            }
        });
        Ok(reply)
    }
    pub fn stop(&self, owner: &str, id: &str) -> bool {
        let mut entries = self.entries.lock().unwrap();
        if !entries.get(id).is_some_and(|entry| entry.owner == owner) {
            return false;
        }
        entries.remove(id);
        true
    }
    pub fn close_window(&self, owner: &str) {
        self.entries
            .lock()
            .unwrap()
            .retain(|_, entry| entry.owner != owner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn watches_emit_changes_and_only_the_owner_can_stop_them() {
        let file = std::env::temp_dir().join(format!("hermes-watch-{}.txt", std::process::id()));
        std::fs::write(&file, "before").unwrap();
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = events.clone();
        let registry = PreviewWatches::default();
        let watch = registry
            .start("main", file.to_str().unwrap(), move |event| {
                captured.lock().unwrap().push(event)
            })
            .unwrap();
        let id = watch["id"].as_str().unwrap();
        assert!(!registry.stop("peer-1", id));
        std::fs::write(&file, "after with a different length").unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                if !events.lock().unwrap().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(events.lock().unwrap()[0]["id"], id);
        assert_eq!(events.lock().unwrap()[0]["path"], file.to_str().unwrap());
        registry.close_window("main");
        assert!(registry.entries.lock().unwrap().is_empty());
        std::fs::remove_file(file).unwrap();
    }
    #[test]
    fn file_urls_are_decoded_as_urls_and_foreign_origins_refused() {
        assert_eq!(
            preview_path("file:///tmp/a%20b.txt").unwrap(),
            std::path::PathBuf::from("/tmp/a b.txt")
        );
        assert!(preview_path("https://example.com/file.txt").is_err());
        assert!(preview_path("file://foreign/tmp/a.txt").is_err());
    }
}
