//! Private local gateway discovery and grant authority for one owned profile.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::{
    fs::{FileTypeExt, MetadataExt},
    net::UnixStream,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::types::{AttachEndpoint, AttachReceipt, HermesConnection};
use serde_json::{json, Value};

const MAX_CONTROL_REPLY: u64 = 512 * 1024;

#[derive(Debug, Clone)]
pub struct NativeGateway {
    pub connection: HermesConnection,
    pub instance_id: String,
    pub profile_id: String,
    pub control_home: PathBuf,
}

/// A bounded exchange on the daemon's private, same-user control socket.
pub fn control_request(home: &Path, verb: &str, params: Value) -> Result<Value, String> {
    let socket = home.join("gateway.sock");
    let uid = unsafe { libc::geteuid() };
    for (path, is_home) in [(home, true), (socket.as_path(), false)] {
        let meta = std::fs::symlink_metadata(path)
            .map_err(|e| format!("Gateway control path {}: {e}", path.display()))?;
        if meta.uid() != uid
            || meta.file_type().is_symlink()
            || (is_home && (!meta.is_dir() || meta.mode() & 0o022 != 0))
            || (!is_home && (!meta.file_type().is_socket() || meta.mode() & 0o077 != 0))
        {
            return Err(format!("Unsafe gateway control path: {}", path.display()));
        }
    }
    let mut stream =
        UnixStream::connect(&socket).map_err(|e| format!("Gateway control connection: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let mut request = serde_json::to_vec(&json!({"protocol":1,"id":1,"verb":verb,"params":params}))
        .map_err(|e| e.to_string())?;
    if request.len() >= 64 * 1024 {
        return Err("Gateway control request exceeded limit".into());
    }
    request.push(b'\n');
    stream
        .write_all(&request)
        .map_err(|e| format!("Gateway control write: {e}"))?;
    let mut reply = Vec::new();
    BufReader::new(stream.take(MAX_CONTROL_REPLY + 1))
        .read_until(b'\n', &mut reply)
        .map_err(|e| format!("Gateway control read: {e}"))?;
    if reply.len() as u64 > MAX_CONTROL_REPLY || reply.last() != Some(&b'\n') {
        return Err("Gateway control reply incomplete or exceeded limit".into());
    }
    let reply: Value =
        serde_json::from_slice(&reply).map_err(|_| "Invalid gateway control reply")?;
    if reply["protocol"] != 1 || reply["id"] != 1 {
        return Err("Gateway control protocol or response identity mismatch".into());
    }
    if reply["ok"] != true {
        return Err("Gateway refused the control request".into());
    }
    reply
        .get("result")
        .cloned()
        .ok_or_else(|| "Gateway control reply has no result".into())
}

pub fn resolve(home: &Path, profile: Option<&str>) -> Result<NativeGateway, String> {
    // Profile names are a single identifier, never a caller-supplied filesystem path.
    if let Some(name) = profile {
        if name.is_empty()
            || !name.as_bytes()[0].is_ascii_alphanumeric()
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        {
            return Err("Invalid profile route".into());
        }
    }
    let home = home
        .canonicalize()
        .map_err(|e| format!("Gateway home: {e}"))?;
    let requested = match profile {
        None | Some("default") => home.clone(),
        Some(name) => home.join("profiles").join(name),
    };
    let result = control_request(&home, "resolve-profile", json!({"home":requested}))?;
    if result["resolve_protocol"] != 1 {
        return Err("Unsupported gateway resolution protocol".into());
    }
    if result["state"] != "ready" || result["attachable"] != true || result["update_fenced"] == true
    {
        return Err("The requested profile's gateway is not ready or is being updated".into());
    }
    let endpoint = &result["endpoint"];
    if endpoint["runtime_protocol"] != 1 {
        return Err("Unsupported gateway runtime protocol".into());
    }
    let read = |key: &str| {
        endpoint[key]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| format!("Gateway resolution missing {key}"))
    };
    let api = read("api_origin")?;
    let ws = read("ws_url")?;
    let origin = url::Url::parse(&api).map_err(|_| "Invalid native gateway origin")?;
    let socket = url::Url::parse(&ws).map_err(|_| "Invalid native gateway socket URL")?;
    let loopback = match origin.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain("localhost")) => true,
        _ => false,
    };
    if !loopback
        || origin.scheme() != "http"
        || socket.scheme() != "ws"
        || origin.host() != socket.host()
        || origin.port_or_known_default() != socket.port_or_known_default()
        || origin.path() != "/"
        || origin.query().is_some()
        || origin.fragment().is_some()
        || !origin.username().is_empty()
        || origin.password().is_some()
        || !socket.username().is_empty()
        || socket.password().is_some()
    {
        return Err("Native gateway origin mismatch".into());
    }
    let instance_id = read("instance_id")?;
    let profile_id = read("profile_id")?;
    let control_home = PathBuf::from(endpoint["control_home"].as_str().unwrap_or(&profile_id));
    if !Path::new(&profile_id).is_absolute() || !control_home.is_absolute() {
        return Err("Gateway resolution has a nonabsolute home".into());
    }
    let receipt = AttachReceipt {
        endpoint: AttachEndpoint {
            api_origin: api,
            instance_id: Some(instance_id.clone()),
            profile_id: Some(profile_id.clone()),
            supervisor: None,
        },
        url: ws,
        ticket: None,
        protocol: Some("hermes-gateway-v1".into()),
        state: Some("ready".into()),
    };
    let mut connection = crate::discovery::connection_from_receipt(&receipt);
    connection.profile = Some(profile.unwrap_or("default").to_owned());
    connection.connection_id = Some("local".into());
    Ok(NativeGateway {
        connection,
        instance_id,
        profile_id,
        control_home,
    })
}

impl NativeGateway {
    pub fn mint(&self, purpose: &str) -> Result<String, String> {
        let mut params =
            json!({"profile_id":self.profile_id,"instance_id":self.instance_id,"purpose":purpose});
        if purpose == "interactive" {
            params["scope"] = json!("host");
        }
        control_request(&self.control_home, "session-ticket", params)?["ticket"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| "Gateway returned no ticket".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn fixture(
        reply: serde_json::Value,
    ) -> (
        std::path::PathBuf,
        std::thread::JoinHandle<serde_json::Value>,
    ) {
        let home = std::env::temp_dir().join(format!(
            "hermes-resolve-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&home).unwrap();
        let socket = home.join("gateway.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).unwrap();
        let thread = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut raw = String::new();
            BufReader::new(socket.try_clone().unwrap())
                .read_line(&mut raw)
                .unwrap();
            socket.write_all(format!("{reply}\n").as_bytes()).unwrap();
            serde_json::from_str(&raw).unwrap()
        });
        (home, thread)
    }

    #[test]
    fn resolves_the_named_profile_and_keeps_authoritative_control_identity() {
        let (home, server) = fixture(json!({"protocol":1,"id":1,"ok":true,"result":{
            "resolve_protocol":1,"state":"ready","attachable":true,"update_fenced":false,
            "endpoint":{"runtime_protocol":1,"api_origin":"http://127.0.0.1:9123","ws_url":"ws://127.0.0.1:9123/api/ws", "profile_id":"/canonical/profiles/work","instance_id":"owner-a","control_home":"/canonical"}
        }}));
        let owner = resolve(&home, Some("work")).unwrap();
        let request = server.join().unwrap();
        assert_eq!(request["verb"], "resolve-profile");
        assert_eq!(
            request["params"]["home"],
            home.canonicalize()
                .unwrap()
                .join("profiles/work")
                .to_str()
                .unwrap()
        );
        assert_eq!(owner.profile_id, "/canonical/profiles/work");
        assert_eq!(owner.instance_id, "owner-a");
        assert_eq!(owner.control_home, std::path::PathBuf::from("/canonical"));
        assert!(owner.connection.token.is_empty());
        assert_eq!(owner.connection.profile.as_deref(), Some("work"));
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn refuses_version_skew_and_fenced_or_unready_resolution() {
        for result in [
            json!({"resolve_protocol":2,"state":"ready","attachable":true}),
            json!({"resolve_protocol":1,"state":"stopped","attachable":false}),
            json!({"resolve_protocol":1,"state":"ready","attachable":true,"update_fenced":true}),
        ] {
            let (home, server) = fixture(json!({"protocol":1,"id":1,"ok":true,"result":result}));
            assert!(resolve(&home, None).is_err());
            server.join().unwrap();
            std::fs::remove_dir_all(home).unwrap();
        }
    }

    #[test]
    fn rejects_a_mismatched_control_reply() {
        let (home, server) = fixture(json!({"protocol":1,"id":99,"ok":true,"result":{}}));
        assert!(control_request(&home, "identify", json!({})).is_err());
        server.join().unwrap();
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn rejects_traversal_before_opening_a_socket() {
        assert!(resolve(std::path::Path::new("/missing"), Some("../work"))
            .unwrap_err()
            .contains("profile"));
    }
}
