//! Native peer/session windows use the renderer's query-before-hash contract.
use hermes_contract::scope;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

#[derive(Default)]
pub struct WindowRoutes(Mutex<HashMap<String, Value>>);
impl WindowRoutes {
    pub fn remember(&self, label: &str, profile: &str) {
        self.0
            .lock()
            .unwrap()
            .entry(label.into())
            .or_insert_with(|| json!({"profile": profile, "connectionId": null}));
    }
    pub fn set(&self, label: &str, route: Value) -> Result<(), String> {
        let valid = scope::requested_profile(&route);
        // Retain an unsupported owner too: opening a peer must refuse that
        // route rather than accidentally resurrect a previously local owner.
        self.0.lock().unwrap().insert(label.into(), route);
        valid.map(|_| ())
    }
    pub fn forget(&self, label: &str) {
        self.0.lock().unwrap().remove(label);
    }
    pub fn inherited(&self, label: &str) -> Value {
        self.0
            .lock()
            .unwrap()
            .get(label)
            .cloned()
            .unwrap_or(Value::Null)
    }
}
static NEXT_WINDOW: AtomicU64 = AtomicU64::new(1);

fn window_url(session: Option<&str>, options: &Value, inherited: &Value) -> Result<String, String> {
    let explicit = !options.is_null();
    let route = if explicit { options } else { inherited };
    if !route.is_null() && !route.is_object() {
        return Err("Invalid window options".into());
    }
    scope::requested_profile(route)?;
    let profile = route
        .get("profile")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty());
    let mut url = if session.is_some() {
        "index.html?win=secondary"
    } else {
        "index.html?peer=1"
    }
    .to_owned();
    if let Some(profile) = profile {
        url.push_str(&format!(
            "&profile={}&connectionId=",
            urlencoding::encode(profile.trim())
        ));
        if session.is_none() && explicit {
            url.push_str("&profileWindow=1");
        }
    }
    if let Some(session) = session {
        if session.trim().is_empty() || session.len() > 200 || session.chars().any(char::is_control)
        {
            return Err("Invalid session id".into());
        }
        if let Some(watch) = options.get("watch") {
            if !watch.is_boolean() {
                return Err("Invalid watch flag".into());
            }
            if watch == true {
                url.push_str("&watch=1");
            }
        }
        url.push_str(&format!("#/{}", urlencoding::encode(session)));
    }
    Ok(url)
}

pub fn open(
    app: &tauri::AppHandle,
    home: &PathBuf,
    session: Option<&str>,
    options: &Value,
    inherited: &Value,
) -> Result<Value, String> {
    let url = window_url(session, options, inherited)?;
    let kind = if session.is_some() { "session" } else { "peer" };
    let label = format!("{kind}-{}", NEXT_WINDOW.fetch_add(1, Ordering::Relaxed));
    tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App(url.into()))
        .title("Hermes")
        .inner_size(1180.0, 820.0)
        .min_inner_size(720.0, 480.0)
        .initialization_script(crate::bootstrap_script(home))
        .build()
        .map_err(|e| e.to_string())?;
    Ok(json!({"ok": true}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_active_route_wins_over_later_background_profile_reads() {
        let routes = WindowRoutes::default();
        routes.remember("main", "default");
        routes
            .set("main", json!({"profile":"work","connectionId":null}))
            .unwrap();
        routes.remember("main", "other");
        assert_eq!(routes.inherited("main")["profile"], "work");
        assert!(routes
            .set(
                "main",
                json!({"profile":"work","connectionId":"unsupported-remote"})
            )
            .is_err());
        assert!(window_url(None, &Value::Null, &routes.inherited("main")).is_err());
    }
    #[test]
    fn session_uses_source_query_contract_and_encodes_route() {
        assert_eq!(
            window_url(
                Some("a b/c?#"),
                &json!({"profile":"work", "connectionId":null,"watch":true}),
                &Value::Null
            )
            .unwrap(),
            "index.html?win=secondary&profile=work&connectionId=&watch=1#/a%20b%2Fc%3F%23"
        );
    }
    #[test]
    fn peer_inherits_current_owner_without_permanent_profile_pin() {
        let owner = json!({"profile":"work","connectionId":null});
        assert_eq!(
            window_url(None, &Value::Null, &owner).unwrap(),
            "index.html?peer=1&profile=work&connectionId="
        );
        assert!(window_url(None, &owner, &Value::Null)
            .unwrap()
            .ends_with("&profileWindow=1"));
    }
    #[test]
    fn omitted_session_options_still_route_to_session() {
        assert_eq!(
            window_url(Some("task"), &Value::Null, &Value::Null).unwrap(),
            "index.html?win=secondary#/task"
        );
    }
    #[test]
    fn invalid_owners_or_window_payloads_fail_before_creation() {
        for options in [
            json!({"connectionId":"remote","profile":"work"}),
            json!({"profile":"../private"}),
            json!("work"),
            json!({"profile":5}),
            json!({"watch":"yes"}),
        ] {
            assert!(window_url(Some("task"), &options, &Value::Null).is_err());
        }
        for id in ["", "\n", "task\0other"] {
            assert!(window_url(Some(id), &Value::Null, &Value::Null).is_err());
        }
    }
    #[test]
    fn window_routes_are_independent_and_retired_on_close() {
        let routes = WindowRoutes::default();
        routes.remember("main", "default");
        routes.remember("peer-1", "work");
        assert_eq!(routes.inherited("main")["profile"], "default");
        assert_eq!(routes.inherited("peer-1")["profile"], "work");
        routes.forget("peer-1");
        assert_eq!(routes.inherited("peer-1"), Value::Null);
    }
}
