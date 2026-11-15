//! Desktop-facing operations the renderer calls on `window.hermesDesktop` but
//! the shell had no Rust answer for: opening links, OS windows, the gateway
//! file bridge, machine facts, SSH config, theme marketplace and settings.
//!
//! Each function mirrors the Electron main-process behaviour it replaces. Where
//! Electron did something the Rust shell cannot honestly do, the function
//! returns the shape the renderer already handles for "unavailable" rather than
//! inventing success.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::fs_ops;

/// Only these schemes may be handed to the OS. Anything else — notably
/// `file:` and `javascript:` — is refused: an external open comes from model
/// output and page content, so it must never reach a local file or a script
/// URL.
const WEB_SCHEMES: &[&str] = &["http", "https", "mailto", "tel"];

fn validate_external_url(raw_url: &str) -> Result<&str, String> {
    let url = raw_url.trim();
    if url.is_empty() {
        return Err("openExternal: empty url".into());
    }
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase())
        .ok_or_else(|| "openExternal: not an absolute url".to_string())?;
    if !WEB_SCHEMES.contains(&scheme.as_str()) {
        return Err(format!("openExternal: refused scheme {scheme}"));
    }
    // A control character could smuggle a second argument past the URL.
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("openExternal: url contains whitespace or control characters".into());
    }

    Ok(url)
}

pub fn open_external(raw_url: &str) -> Result<Value, String> {
    let url = validate_external_url(raw_url)?;
    #[cfg(target_os = "macos")]
    let opened = std::process::Command::new("open")
        .arg(url)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    #[cfg(not(target_os = "macos"))]
    let opened = std::process::Command::new("xdg-open")
        .arg(url)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if opened {
        Ok(Value::Null)
    } else {
        Err("openExternal: the OS refused to open the url".into())
    }
}

/// Machine facts the i18n bootstrap reads to pick a first-run locale.
pub fn machine_profile() -> Value {
    let age_days = std::fs::metadata(std::env::var_os("HOME").unwrap_or_default())
        .and_then(|meta| meta.created())
        .ok()
        .and_then(|created| created.elapsed().ok())
        .map(|age| age.as_secs() / 86_400);

    let username = std::env::var("USER").unwrap_or_default();
    let locale = std::env::var("LANG")
        .ok()
        .and_then(|value| value.split(['.', '@']).next().map(str::to_string))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "en_US".to_string())
        .replace('_', "-");

    json!({
        "ageDays": age_days,
        "arch": std::env::consts::ARCH,
        "locale": locale,
        "platform": std::env::consts::OS,
        "release": uname_release(),
        "username": username
    })
}

fn uname_release() -> String {
    std::process::Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_default()
}

/// `ssh -G <host>` is the only correct way to resolve an alias: it applies
/// `Include`s, `Match` blocks and the system config in one pass. Parsing the
/// file by hand would answer a different question than ssh itself.
pub fn ssh_resolve_host(host: &str) -> Value {
    let host = host.trim();
    if host.is_empty() || host.starts_with('-') {
        return json!({ "hostname": null, "identityFile": null, "port": null, "user": null });
    }
    let output = std::process::Command::new("ssh")
        .args(["-G", "--", host])
        .output();

    let Ok(output) = output else {
        return json!({ "hostname": null, "identityFile": null, "port": null, "user": null });
    };
    if !output.status.success() {
        return json!({ "hostname": null, "identityFile": null, "port": null, "user": null });
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let field = |key: &str| -> Option<String> {
        text.lines()
            .filter_map(|line| line.trim().split_once(' '))
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.trim().to_string())
    };

    json!({
        "hostname": field("hostname"),
        "identityFile": field("identityfile"),
        "port": field("port").and_then(|value| value.parse::<u16>().ok()),
        "user": field("user")
    })
}

/// The default project directory lives in the same preferences file the
/// renderer's own UI preferences use, so one file describes one user's shell.
pub fn get_default_project_dir(home: &Path) -> Value {
    let prefs = crate::commands::load_preferences(home);
    Value::String(
        prefs
            .values
            .get("defaultProjectDir")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
    )
}

pub fn set_default_project_dir(home: &Path, dir: &str) -> Result<Value, String> {
    let mut prefs = crate::commands::load_preferences(home);
    prefs
        .values
        .insert("defaultProjectDir".into(), Value::String(dir.to_string()));
    prefs.revision = prefs.revision.saturating_add(1);
    crate::commands::save_preferences(home, &prefs)?;
    Ok(Value::String(dir.to_string()))
}

pub fn pick_default_project_dir(home: &Path) -> Result<Value, String> {
    let picked = fs_ops::select_paths(&json!({ "directories": true }))?;
    let Some(path) = picked
        .get("paths")
        .and_then(|paths| paths.as_array())
        .and_then(|paths| paths.first())
        .and_then(|first| first.as_str())
        .map(str::to_string)
    else {
        return Ok(Value::Null);
    };
    Ok(set_default_project_dir(home, &path)?)
}

/// Ask for a destination, then stream the gateway's bytes straight into a temp
/// file beside it and rename on success — a failed download never truncates an
/// existing file at the destination.
pub fn save_gateway_file(home: &Path, payload: &Value) -> Result<Value, String> {
    let path = payload
        .get("path")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "saveGatewayFile: missing path".to_string())?;
    let suggested = payload
        .get("suggestedName")
        .and_then(|value| value.as_str())
        .unwrap_or("download");

    let picked = fs_ops::select_save_path(&json!({ "defaultPath": suggested }))?;
    let Some(destination) = picked.as_str() else {
        return Ok(json!({ "canceled": true, "saved": false }));
    };

    let owner = crate::commands::resolve_owner(home, payload)?;
    let ticket = owner.mint("native-http")?;
    let mut url = format!(
        "{}/api/files/download?path={}&profile={}",
        owner.connection.base_url.trim_end_matches('/'),
        urlencoding::encode(path),
        urlencoding::encode(owner.connection.profile.as_deref().unwrap_or("default"))
    );
    if let Some(session) = payload.get("sessionId").and_then(Value::as_str) {
        url.push_str(&format!("&session_id={}", urlencoding::encode(session)));
    }
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let mut request = agent
        .request("GET", &url)
        .set("X-Hermes-Gateway-Ticket", &ticket);
    if !owner.connection.token.is_empty() {
        request = request.set(
            "Authorization",
            &format!("Bearer {}", owner.connection.token),
        );
    }

    let response = request.call().map_err(|e| {
        if let ureq::Error::Status(code, _) = &e {
            format!("saveGatewayFile: gateway answered {code}")
        } else {
            format!("saveGatewayFile: {e}")
        }
    })?;
    let mut reader = response.into_reader();
    let destination = PathBuf::from(destination);
    let partial = destination.with_extension(format!(
        "{}.part",
        destination
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("download")
    ));
    let mut file = std::fs::File::create(&partial).map_err(|e| format!("saveGatewayFile: {e}"))?;
    if let Err(error) = std::io::copy(&mut reader, &mut file) {
        drop(file);
        let _ = std::fs::remove_file(&partial);
        return Err(format!("saveGatewayFile: {error}"));
    }
    drop(file);
    std::fs::rename(&partial, &destination).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        format!("saveGatewayFile: {e}")
    })?;

    Ok(json!({
        "canceled": false,
        "path": destination.to_string_lossy(),
        "saved": true
    }))
}

/// Save a remote image (an assistant illustration, say) by fetching it and
/// reusing the local image-save path.
pub fn save_image_from_url(url: &str) -> Result<Value, String> {
    let url = url.trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("saveImageFromUrl: only http(s) urls".into());
    }
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("saveImageFromUrl: {e}"))?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut response.into_reader(), &mut bytes)
        .map_err(|e| format!("saveImageFromUrl: {e}"))?;

    // The extension follows the URL's own path, so the dialog suggests the
    // right filter before the user sees the bytes.
    let ext = url
        .split('?')
        .next()
        .and_then(|clean| clean.rsplit_once('.'))
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .filter(|ext| ["png", "jpg", "jpeg", "gif", "webp", "svg"].contains(&ext.as_str()))
        .unwrap_or_else(|| "png".to_string());
    let name = url
        .split('?')
        .next()
        .and_then(|clean| clean.rsplit('/').next())
        .filter(|name| !name.is_empty())
        .unwrap_or("image");

    let saved = fs_ops::save_image_buffer(&json!({
        "data": bytes,
        "ext": format!(".{ext}"),
        "name": name
    }))?;
    Ok(Value::Bool(
        saved
            .get("saved")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    ))
}

/// One HTML-free summary of a preview target: what kind of thing the right rail
/// is about to show, and whether it exists. The renderer decides how to render;
/// the shell only answers "what is at this path".
pub fn normalize_preview_target(target: &str, base_dir: Option<&str>) -> Value {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Value::Null;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return json!({
            "kind": "url",
            "label": trimmed,
            "renderMode": "preview"
        });
    }
    let path = if Path::new(trimmed).is_absolute() {
        PathBuf::from(trimmed)
    } else if let Some(base) = base_dir {
        Path::new(base).join(trimmed)
    } else {
        PathBuf::from(trimmed)
    };
    let path_text = path.to_string_lossy().to_string();
    let meta = std::fs::symlink_metadata(&path).ok();

    let (preview_kind, binary, byte_size) = match &meta {
        None => ("missing", false, None),
        Some(meta) if meta.is_dir() => ("directory", false, None),
        Some(meta) => {
            let kind = match path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_ascii_lowercase())
                .as_deref()
            {
                Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "svg") => "image",
                Some("pdf") => "pdf",
                Some("html" | "htm") => "html",
                _ => "text",
            };
            (kind, kind == "image" || kind == "pdf", Some(meta.len()))
        }
    };

    json!({
        "binary": binary,
        "byteSize": byte_size,
        "kind": "file",
        "label": path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path_text.clone()),
        "large": byte_size.map(|size| size > 2 * 1024 * 1024).unwrap_or(false),
        "path": path_text,
        "previewKind": preview_kind,
        "renderMode": if preview_kind == "html" || preview_kind == "image" || preview_kind == "pdf" {
            "preview"
        } else {
            "source"
        }
    })
}

/// Poll-based file watch: the renderer gets a stable id and a change event, so a
/// hot-reloaded preview repaints without the shell owning a native watcher.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_url_validation_has_no_os_or_browser_side_effects() {
        for url in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "",
            "not a url",
            "https://example.com/\nrm -rf /",
        ] {
            assert!(validate_external_url(url).is_err());
        }
        assert_eq!(
            validate_external_url("https://example.com/").unwrap(),
            "https://example.com/"
        );
        assert_eq!(
            validate_external_url(" https://example.com/ ").unwrap(),
            "https://example.com/"
        );
    }

    #[test]
    fn machine_profile_answers_every_field_the_renderer_reads() {
        let profile = machine_profile();
        for key in [
            "ageDays", "arch", "locale", "platform", "release", "username",
        ] {
            assert!(profile.get(key).is_some(), "missing {key}");
        }
        assert!(!profile["locale"].as_str().unwrap().contains('_'));
    }

    #[test]
    fn preview_targets_classify_without_reading_bytes() {
        let missing = normalize_preview_target("/definitely/not/here.txt", None);
        assert_eq!(missing["previewKind"], "missing");
        assert_eq!(missing["kind"], "file");

        let url = normalize_preview_target("https://example.com/x", None);
        assert_eq!(url["kind"], "url");

        let dir = std::env::temp_dir().join(format!("hermes-preview-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let html = dir.join("index.html");
        std::fs::write(&html, "<p>hi</p>").unwrap();
        let resolved = normalize_preview_target("index.html", Some(&dir.to_string_lossy()));
        assert_eq!(resolved["previewKind"], "html");
        assert_eq!(resolved["renderMode"], "preview");
        assert_eq!(resolved["byteSize"], 9);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_relative_preview_resolves_against_its_base() {
        let resolved = normalize_preview_target("note.md", Some("/tmp/workspace"));
        assert_eq!(resolved["path"], "/tmp/workspace/note.md");
        assert_eq!(resolved["label"], "note.md");
    }

    #[test]
    fn ssh_resolution_refuses_an_option_lookalike() {
        let result = ssh_resolve_host("-oProxyCommand=evil");
        assert!(result["hostname"].is_null());
        assert!(result["user"].is_null());
    }

    #[test]
    fn the_default_project_dir_round_trips_through_preferences() {
        let home = std::env::temp_dir().join(format!("hermes-prefs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(
            home.join("desktop-ui.json"),
            "{\"revision\":0,\"values\":{}}",
        )
        .unwrap();

        assert_eq!(get_default_project_dir(&home), Value::String(String::new()));
        set_default_project_dir(&home, "/tmp/projects").unwrap();
        assert_eq!(
            get_default_project_dir(&home),
            Value::String("/tmp/projects".into())
        );
        let _ = std::fs::remove_dir_all(&home);
    }
}
