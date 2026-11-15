//! Native plugin dial grants never enter renderer-visible URLs or diagnostics.
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tauri::Url;
struct Dial {
    owner: String,
    prepared: String,
    target: String,
    ticket: String,
    created: Instant,
}
#[derive(Default)]
pub struct PreparedDials {
    entries: Mutex<HashMap<String, Dial>>,
    next: AtomicU64,
}
pub fn plugin_path(id: &str, path: &str) -> Result<String, String> {
    if id.is_empty()
        || id.len() > 128
        || !id.as_bytes()[0].is_ascii_alphanumeric()
        || id.contains("..")
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    {
        return Err("Invalid plugin id".into());
    }
    if !path.starts_with('/')
        || path.starts_with("//")
        || path.len() > 2048
        || path.contains(['#', '\\'])
        || path.chars().any(char::is_control)
    {
        return Err("Invalid plugin socket path".into());
    }
    for part in path.split('?').next().unwrap_or_default().split('/') {
        let decoded = urlencoding::decode(part).map_err(|_| "Invalid plugin socket encoding")?;
        if decoded == "." || decoded == ".." || decoded.contains(['/', '\\', '%']) {
            return Err("Invalid plugin socket path".into());
        }
    }
    let url = Url::parse(&format!("ws://plugin.invalid/api/plugins/{id}{path}"))
        .map_err(|e| e.to_string())?;
    if url
        .query_pairs()
        .any(|(key, _)| key == "native_dial" || key == "ticket")
    {
        return Err("Reserved plugin socket query".into());
    }
    Ok(format!(
        "{}{}",
        url.path(),
        url.query().map(|q| format!("?{q}")).unwrap_or_default()
    ))
}
impl PreparedDials {
    pub fn prepare(
        &self,
        owner: &str,
        origin: &str,
        path: &str,
        ticket: String,
    ) -> Result<String, String> {
        let mut target = Url::parse(origin).map_err(|e| e.to_string())?;
        target
            .set_scheme("ws")
            .map_err(|_| "Invalid gateway socket scheme")?;
        let target = target.join(path).map_err(|e| e.to_string())?.to_string();
        let id = format!("rust-plugin-{}", self.next.fetch_add(1, Ordering::Relaxed));
        let mut prepared = Url::parse(&target).map_err(|e| e.to_string())?;
        prepared.query_pairs_mut().append_pair("native_dial", &id);
        let prepared = prepared.to_string();
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|_, dial| dial.created.elapsed() < Duration::from_secs(30));
        if entries.len() >= 128 {
            return Err("Too many prepared plugin sockets".into());
        }
        entries.insert(
            id,
            Dial {
                owner: owner.into(),
                prepared: prepared.clone(),
                target,
                ticket,
                created: Instant::now(),
            },
        );
        Ok(prepared)
    }
    pub fn take(&self, owner: &str, raw: &str) -> Result<Option<(String, String)>, String> {
        let url = Url::parse(raw).map_err(|e| e.to_string())?;
        let Some(id) = url
            .query_pairs()
            .find(|(key, value)| key == "native_dial" && value.starts_with("rust-plugin-"))
            .map(|(_, value)| value.into_owned())
        else {
            return Ok(None);
        };
        let mut entries = self.entries.lock().unwrap();
        let dial = entries
            .get(&id)
            .filter(|dial| {
                dial.owner == owner
                    && dial.prepared == raw
                    && dial.created.elapsed() < Duration::from_secs(30)
            })
            .ok_or("Plugin socket grant is missing, expired or belongs to another window")?;
        let result = (dial.target.clone(), dial.ticket.clone());
        entries.remove(&id);
        Ok(Some(result))
    }
    pub fn close_window(&self, owner: &str) {
        self.entries
            .lock()
            .unwrap()
            .retain(|_, dial| dial.owner != owner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plugin_namespace_cannot_escape_through_encoding_or_queries() {
        assert_eq!(
            plugin_path("kanban", "/events?board=a").unwrap(),
            "/api/plugins/kanban/events?board=a"
        );
        for id in ["../other", "bad..id", "", "a/b"] {
            assert!(plugin_path(id, "/events").is_err());
        }
        for path in [
            "/../config",
            "/%2e%2e/config",
            "/a%2fb",
            "/a%255cb",
            "/a\\b",
            "/events#hash",
            "//other",
            "/events?native_dial=other",
            "/events?ticket=bad",
        ] {
            assert!(plugin_path("kanban", path).is_err(), "{path}");
        }
    }
    #[test]
    fn grant_stays_native_and_is_one_use_window_bound_and_url_bound() {
        let dials = PreparedDials::default();
        let url = dials
            .prepare(
                "main",
                "http://127.0.0.1:1234",
                "/api/plugins/kanban/events",
                "private-grant".into(),
            )
            .unwrap();
        assert!(!url.contains("private-grant"));
        assert!(dials.take("peer-1", &url).is_err());
        assert!(dials
            .take("main", &url.replace("/events", "/config"))
            .is_err());
        let (target, grant) = dials.take("main", &url).unwrap().unwrap();
        assert_eq!(target, "ws://127.0.0.1:1234/api/plugins/kanban/events");
        assert_eq!(grant, "private-grant");
        assert!(dials.take("main", &url).is_err());
        assert!(dials
            .take("main", "ws://127.0.0.1:1234/api/ws?native_dial=1")
            .unwrap()
            .is_none());
    }
}
