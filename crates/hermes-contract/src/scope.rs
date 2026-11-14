//! Owner and native-HTTP grant routing. A missing port must never mean "local".

use serde_json::Value;

pub fn require_local_connection(value: Option<&Value>) -> Result<(), String> {
    match value {
        None | Some(Value::Null) => Ok(()),
        Some(Value::String(id)) if id.is_empty() || id == "local" => Ok(()),
        Some(Value::String(_)) => Err("This gateway connection is not yet supported by the Rust host; the request was not sent.".into()),
        _ => Err("Invalid connection identity".into()),
    }
}

fn profile(value: Option<&Value>) -> Result<Option<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(name)) => profile_name(name),
        _ => Err("Invalid profile identity".into()),
    }
}

/// Preload's getConnection(profile) and registry getConnectionFor({owner})
/// are distinct public shapes; both must retain the selected profile.
pub fn requested_profile(payload: &Value) -> Result<Option<String>, String> {
    match payload {
        Value::Null => Ok(None),
        Value::String(_) => profile(Some(payload)),
        Value::Object(owner) => {
            require_local_connection(owner.get("connectionId"))?;
            profile(owner.get("profile"))
        }
        _ => Err("Invalid gateway owner".into()),
    }
}

pub fn profile_name(name: &str) -> Result<Option<String>, String> {
    let name = name.trim();
    if name.is_empty() || name == "current" {
        return Ok(None);
    }
    literal_profile_name(name).map(Some)
}

fn literal_profile_name(name: &str) -> Result<String, String> {
    if name.is_empty() {
        return Err("Invalid profile route".into());
    }
    if !name.as_bytes()[0].is_ascii_alphanumeric()
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err("Invalid profile route".into());
    }
    Ok(name.to_owned())
}

#[derive(Debug)]
pub struct NativeHttpRoute {
    pub path: String,
    pub profile: Option<String>,
    pub purpose: &'static str,
}

fn collection(path: &str, method: &str) -> Option<&'static str> {
    match (path, method) {
        ("/api/profiles/active", "GET" | "POST") => Some("active"),
        ("/api/profiles/import", "POST") => Some("import"),
        ("/api/profiles/projects/tree", "GET") => Some("projects/tree"),
        ("/api/profiles/sessions", "GET") => Some("sessions"),
        ("/api/profiles/sessions/pull-requests", "POST") => Some("sessions/pull-requests"),
        ("/api/profiles/sessions/sidebar", "GET") => Some("sessions/sidebar"),
        _ => None,
    }
}

/// Mirrors the method-aware native grant policy in Electron's local-gateway.ts.
/// All explicit selectors must agree before any network I/O or ticket mint.
pub fn route_http(
    path: &str,
    method: &str,
    body: Option<&Value>,
    metadata: Option<&Value>,
) -> Result<NativeHttpRoute, String> {
    if !path.starts_with('/') || path.starts_with("//") || path.contains('#') || path.contains('\\')
    {
        return Err("API path must be a relative path on this gateway".into());
    }
    let mut url =
        url::Url::parse(&format!("http://hermes.invalid{path}")).map_err(|_| "Invalid API path")?;
    let method = method.to_ascii_uppercase();
    let pathname = url.path().trim_end_matches('/');
    let group = collection(pathname, &method);
    let suffix = pathname.strip_prefix("/api/profiles/").unwrap_or("");
    let purpose = if method == "POST" && pathname == "/api/profiles" {
        "native-profile-create"
    } else if matches!(method.as_str(), "PATCH" | "DELETE")
        && !suffix.is_empty()
        && !suffix.contains('/')
        && group.is_none()
    {
        "native-profile-registry"
    } else {
        "native-http"
    };

    let mut selectors = Vec::new();
    if !suffix.is_empty() && group.is_none() {
        let encoded = format!("profile={}", suffix.split('/').next().unwrap());
        let decoded = url::form_urlencoded::parse(encoded.as_bytes())
            .next()
            .unwrap()
            .1
            .into_owned();
        selectors.push(Some(literal_profile_name(&decoded)?));
    }
    for (key, value) in url.query_pairs() {
        if key == "profile" || (group == Some("sessions/sidebar") && key == "recents_profile") {
            selectors.push(profile_name(&value)?);
        }
    }
    selectors.push(profile(
        body.and_then(|body| body.as_object())
            .and_then(|map| map.get("profile")),
    )?);
    let metadata = profile(metadata)?;
    selectors.push(metadata.clone());
    let mut owner: Option<String> = None;
    for name in selectors.into_iter().flatten() {
        if owner.as_ref().is_some_and(|old| old != &name) {
            return Err("Conflicting profile selectors; the request was not sent".into());
        }
        owner = Some(name);
    }
    if let Some(metadata) = metadata {
        let has_selector = url.query_pairs().any(|(key, _)| {
            key == "profile" || (group == Some("sessions/sidebar") && key == "recents_profile")
        });
        if !has_selector && (suffix.is_empty() || group.is_some()) {
            url.query_pairs_mut().append_pair(
                if group == Some("sessions/sidebar") {
                    "recents_profile"
                } else {
                    "profile"
                },
                &metadata,
            );
        }
    }
    Ok(NativeHttpRoute {
        path: format!(
            "{}{}",
            url.path(),
            url.query().map(|q| format!("?{q}")).unwrap_or_default()
        ),
        profile: owner,
        purpose,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_foreign_or_malformed_connection_instead_of_using_local() {
        assert!(require_local_connection(Some(&json!("remote-a"))).is_err());
        assert!(require_local_connection(Some(&json!({"id":"local"}))).is_err());
        assert!(require_local_connection(Some(&json!("local"))).is_ok());
        assert!(require_local_connection(None).is_ok());
    }

    #[test]
    fn accepts_both_preload_profile_argument_and_registry_owner_payload() {
        assert_eq!(
            requested_profile(&json!("work")).unwrap().as_deref(),
            Some("work")
        );
        assert_eq!(
            requested_profile(&json!({"connectionId":"local","profile":"work"}))
                .unwrap()
                .as_deref(),
            Some("work")
        );
        assert_eq!(requested_profile(&Value::Null).unwrap(), None);
        assert!(requested_profile(&json!(42)).is_err());
        assert!(requested_profile(&json!({"connectionId":"remote","profile":"work"})).is_err());
    }

    #[test]
    fn routes_metadata_query_path_and_body_with_native_grant_purpose() {
        let r = route_http(
            "/api/config",
            "PATCH",
            Some(&json!({"x":true})),
            Some(&json!("work")),
        )
        .unwrap();
        assert_eq!(r.profile.as_deref(), Some("work"));
        assert_eq!(r.path, "/api/config?profile=work");
        assert_eq!(r.purpose, "native-http");
        let r = route_http("/api/profiles/active", "PATCH", None, None).unwrap();
        assert_eq!(r.profile.as_deref(), Some("active"));
        assert_eq!(r.purpose, "native-profile-registry");
        assert_eq!(
            route_http("/api/profiles/current", "PATCH", None, None)
                .unwrap()
                .profile
                .as_deref(),
            Some("current")
        );
        assert_eq!(
            route_http(
                "/api/profiles/sessions/sidebar",
                "GET",
                None,
                Some(&json!("work"))
            )
            .unwrap()
            .path,
            "/api/profiles/sessions/sidebar?recents_profile=work"
        );
        assert_eq!(
            route_http("/api/profiles/active", "GET", None, None)
                .unwrap()
                .profile,
            None
        );
        assert_eq!(
            route_http("/api/profiles", "POST", None, None)
                .unwrap()
                .purpose,
            "native-profile-create"
        );
        assert_eq!(
            route_http(
                "/api/profiles/sessions/sidebar?recents_profile=work",
                "GET",
                None,
                None
            )
            .unwrap()
            .profile
            .as_deref(),
            Some("work")
        );
        assert_eq!(
            route_http(
                "/api/sessions/id",
                "PATCH",
                Some(&json!({"profile":"work"})),
                None
            )
            .unwrap()
            .profile
            .as_deref(),
            Some("work")
        );
    }

    #[test]
    fn refuses_conflicting_owners_paths_and_invalid_profile_selectors() {
        for path in [
            "https://other.invalid/api/config",
            "//other.invalid/api/config",
            "/api/config#hidden",
        ] {
            assert!(route_http(path, "GET", None, None).is_err(), "{path}");
        }
        assert!(route_http(
            "/api/config?profile=a",
            "PATCH",
            Some(&json!({"profile":"b"})),
            None
        )
        .is_err());
        assert!(route_http("/api/config?profile=a&profile=b", "GET", None, None).is_err());
        assert!(route_http("/api/config?profile=a", "GET", None, Some(&json!("b"))).is_err());
        assert!(route_http("/api/config", "GET", None, Some(&json!("../work"))).is_err());
        assert!(route_http("/api/config", "GET", None, Some(&json!([]))).is_err());
    }
}
