//! Durable shared submission journal. Mutations and comparisons serialize in
//! Rust, so a second window cannot acknowledge the same retained send twice.
use serde_json::{json, Map, Value};
use std::{
    io::{Read, Write},
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};
const MAX_JOURNAL: u64 = 64 * 1024 * 1024;
#[derive(Default)]
pub struct PreparedJournal {
    lock: Mutex<()>,
    next: AtomicU64,
}
fn read_record(home: &Path) -> Result<Map<String, Value>, String> {
    let file = home.join("desktop-prepared-submissions.json");
    let meta = match std::fs::symlink_metadata(&file) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(error) => return Err(error.to_string()),
    };
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_JOURNAL {
        return Err("Unsafe or oversized prepared submission journal".into());
    }
    let mut raw = Vec::new();
    std::fs::File::open(&file)
        .map_err(|e| e.to_string())?
        .take(MAX_JOURNAL + 1)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    if raw.len() as u64 > MAX_JOURNAL {
        return Err("Prepared submission journal exceeded limit".into());
    }
    if let Ok(Value::Object(record)) = serde_json::from_slice(&raw) {
        return Ok(record);
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    std::fs::rename(
        &file,
        home.join(format!("desktop-prepared-submissions.json.corrupt-{stamp}")),
    )
    .map_err(|e| e.to_string())?;
    Ok(Map::new())
}
fn parse(raw: Option<&str>) -> Result<Value, String> {
    raw.map(serde_json::from_str)
        .transpose()
        .map(|v| v.unwrap_or(Value::Null))
        .map_err(|_| "Invalid prepared submission JSON".into())
}
impl PreparedJournal {
    pub fn read(&self, home: &Path) -> Result<String, String> {
        let _guard = self.lock.lock().unwrap();
        serde_json::to_string(&read_record(home)?).map_err(|e| e.to_string())
    }
    fn write(
        &self,
        home: &Path,
        mut record: Map<String, Value>,
        key: &str,
        entry: Value,
    ) -> Result<(), String> {
        use std::os::unix::fs::OpenOptionsExt;
        if key.len() > 64 * 1024 {
            return Err("Prepared submission key exceeded limit".into());
        }
        if entry.is_null() {
            record.remove(key);
        } else {
            record.insert(key.into(), entry);
        }
        let bytes = serde_json::to_vec(&record).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_JOURNAL {
            return Err("Prepared submission journal exceeded limit".into());
        }
        let temporary = home.join(format!(
            ".desktop-prepared-submissions-{}-{}.tmp",
            std::process::id(),
            self.next.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            file.write_all(&bytes)?;
            file.sync_all()?;
            std::fs::rename(&temporary, home.join("desktop-prepared-submissions.json"))
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result.map_err(|e: std::io::Error| e.to_string())
    }
    pub fn update(&self, home: &Path, key: &str, entry: Option<&str>) -> Result<(), String> {
        let entry = parse(entry)?;
        let _guard = self.lock.lock().unwrap();
        self.write(home, read_record(home)?, key, entry)
    }
    pub fn compare_and_set(
        &self,
        home: &Path,
        key: &str,
        expected: Option<&str>,
        entry: Option<&str>,
    ) -> Result<Value, String> {
        let expected = parse(expected)?;
        let entry = parse(entry)?;
        let _guard = self.lock.lock().unwrap();
        let record = read_record(home)?;
        let current = record.get(key).cloned().unwrap_or(Value::Null);
        if current != expected {
            return Ok(
                json!({"applied":false,"current":if current.is_null() { None } else { Some(current.to_string()) }}),
            );
        }
        self.write(home, record, key, entry.clone())?;
        Ok(
            json!({"applied":true,"current":if entry.is_null() { None } else { Some(entry.to_string()) }}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn home(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("hermes-journal-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
    #[test]
    fn invalid_utf8_is_quarantined_too() {
        let home = home("binary-corrupt");
        std::fs::write(
            home.join("desktop-prepared-submissions.json"),
            [255, 0, 128],
        )
        .unwrap();
        assert_eq!(PreparedJournal::default().read(&home).unwrap(), "{}");
        std::fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn read_is_json_text_and_updates_survive_a_new_store() {
        let home = home("persist");
        let store = PreparedJournal::default();
        assert_eq!(store.read(&home).unwrap(), "{}");
        store
            .update(&home, "owner/session", Some("{\"id\":\"draft-a\"}"))
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &PreparedJournal::default().read(&home).unwrap()
            )
            .unwrap()["owner/session"]["id"],
            "draft-a"
        );
        store.update(&home, "owner/session", None).unwrap();
        assert_eq!(store.read(&home).unwrap(), "{}");
        std::fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn two_windows_cannot_both_claim_or_delete_the_same_submission() {
        let home = home("cas");
        let store = PreparedJournal::default();
        assert_eq!(
            store
                .compare_and_set(&home, "key", None, Some("{\"id\":1}"))
                .unwrap()["applied"],
            true
        );
        assert_eq!(
            store
                .compare_and_set(&home, "key", None, Some("{\"id\":2}"))
                .unwrap()["applied"],
            false
        );
        assert_eq!(
            store
                .compare_and_set(&home, "key", Some("{\"id\":2}"), None)
                .unwrap()["applied"],
            false
        );
        assert_eq!(
            store
                .compare_and_set(&home, "key", Some("{\"id\":1}"), None)
                .unwrap()["applied"],
            true
        );
        std::fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn corrupt_bytes_are_quarantined_instead_of_blocking_all_future_sends() {
        let home = home("corrupt");
        std::fs::write(home.join("desktop-prepared-submissions.json"), "broken").unwrap();
        let store = PreparedJournal::default();
        assert_eq!(store.read(&home).unwrap(), "{}");
        assert_eq!(
            std::fs::read_to_string(
                std::fs::read_dir(&home)
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path()
            )
            .unwrap(),
            "broken"
        );
        store.update(&home, "key", Some("{}")).unwrap();
        std::fs::remove_dir_all(home).unwrap();
    }
}
