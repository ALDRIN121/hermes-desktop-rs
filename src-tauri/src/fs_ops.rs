//! Filesystem, dialog and clipboard channels: the `hermes:fs:*`,
//! `hermes:readFile*`, `hermes:select*` and `hermes:save*` surface.
//!
//! Shapes mirror the Electron handlers (`global.d.ts` is the contract the
//! renderer type-checks against), including the structured error object a
//! preview read returns instead of throwing.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

pub const TEXT_PREVIEW_MAX_BYTES: u64 = 512 * 1024;
pub const DATA_URL_DEFAULT_MAX_MB: u64 = 16;
pub const DATA_URL_MIN_MB: u64 = 1;
pub const DATA_URL_MAX_MB: u64 = 256;

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

pub fn expand(path: &str) -> PathBuf {
    let trimmed = path.trim();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return home().join(rest);
    }
    if trimmed == "~" {
        return home();
    }
    PathBuf::from(trimmed)
}

fn error_result(path: &str, code: &str, message: &str) -> Value {
    json!({ "ok": false, "error": code, "message": message, "path": path })
}

fn language_for(ext: &str) -> &'static str {
    match ext {
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "rs" => "rust",
        "py" => "python",
        "go" => "go",
        "json" => "json",
        "md" | "markdown" => "markdown",
        "css" => "css",
        "html" | "htm" => "html",
        "yml" | "yaml" => "yaml",
        "toml" => "toml",
        "sh" | "bash" | "zsh" => "shell",
        "sql" => "sql",
        "swift" => "swift",
        _ => "text",
    }
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        Some("mp4") => "video/mp4",
        Some("mp3") => "audio/mpeg",
        Some("json") => "application/json",
        Some("txt") | Some("md") | Some("log") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn looks_binary(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(4096)];
    sample.contains(&0u8)
}

/// `hermes:readFileText` — same keys the renderer's `HermesReadFileTextResult`
/// declares, including `truncated` when the file is larger than the preview cap.
pub fn read_file_text(raw_path: &str) -> Value {
    let path = expand(raw_path);
    let meta = match std::fs::metadata(&path) {
        Ok(meta) => meta,
        Err(error) => {
            return error_result(
                &path.to_string_lossy(),
                if error.kind() == std::io::ErrorKind::NotFound {
                    "ENOENT"
                } else {
                    "EACCES"
                },
                &error.to_string(),
            )
        }
    };
    if meta.is_dir() {
        return error_result(&path.to_string_lossy(), "EISDIR", "Path is a directory");
    }

    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => return error_result(&path.to_string_lossy(), "EACCES", &error.to_string()),
    };
    let cap = TEXT_PREVIEW_MAX_BYTES as usize;
    let truncated = bytes.len() > cap;
    let slice = &bytes[..bytes.len().min(cap)];

    json!({
        "binary": looks_binary(slice),
        "byteSize": meta.len(),
        "language": language_for(path.extension().and_then(|e| e.to_str()).unwrap_or("")),
        "mimeType": mime_for(&path),
        "path": path.to_string_lossy(),
        "text": String::from_utf8_lossy(slice),
        "truncated": truncated
    })
}

/// `hermes:readPluginSource` — full source, no preview truncation.
pub fn read_plugin_source(raw_path: &str) -> Value {
    let path = expand(raw_path);
    match std::fs::read_to_string(&path) {
        Ok(text) => json!({
            "byteSize": text.len(),
            "language": language_for(path.extension().and_then(|e| e.to_str()).unwrap_or("")),
            "mimeType": mime_for(&path),
            "path": path.to_string_lossy(),
            "text": text,
            "truncated": false
        }),
        Err(error) => error_result(&path.to_string_lossy(), "EACCES", &error.to_string()),
    }
}

fn data_url_for(path: &Path, bytes: &[u8]) -> String {
    format!(
        "data:{};base64,{}",
        mime_for(path),
        base64_encode(bytes)
    )
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

/// `hermes:readFileDataUrl` / `…ForAttach` — a data URL, or the structured
/// error the renderer's `HermesReadFileErrorResult` expects.
pub fn read_file_data_url(raw_path: &str, max_bytes: u64) -> Value {
    let path = expand(raw_path);
    let meta = match std::fs::metadata(&path) {
        Ok(meta) => meta,
        Err(error) => {
            return error_result(
                &path.to_string_lossy(),
                if error.kind() == std::io::ErrorKind::NotFound {
                    "ENOENT"
                } else {
                    "EACCES"
                },
                &error.to_string(),
            )
        }
    };
    if meta.len() > max_bytes {
        return error_result(
            &path.to_string_lossy(),
            "ETOOLARGE",
            &format!("File is {} bytes, limit is {max_bytes}", meta.len()),
        );
    }
    match std::fs::read(&path) {
        Ok(bytes) => Value::String(data_url_for(&path, &bytes)),
        Err(error) => error_result(&path.to_string_lossy(), "EACCES", &error.to_string()),
    }
}

/// `hermes:fs:readDir` — `{ entries: [{ name, path, isDirectory }] }`, sorted
/// directories-first then by name, exactly as the Electron reader sorts.
pub fn read_dir(raw_path: &str) -> Value {
    let path = expand(raw_path);
    let entries = match std::fs::read_dir(&path) {
        Ok(entries) => entries,
        Err(error) => {
            return json!({
                "entries": [],
                "error": if error.kind() == std::io::ErrorKind::NotFound { "ENOENT" } else { "read-error" }
            })
        }
    };

    let mut rows: Vec<Value> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name() != ".DS_Store")
        .map(|entry| {
            let child = entry.path();
            let is_directory = std::fs::metadata(&child).map(|m| m.is_dir()).unwrap_or(false);
            json!({
                "name": entry.file_name().to_string_lossy(),
                "path": child.to_string_lossy(),
                "isDirectory": is_directory
            })
        })
        .collect();

    rows.sort_by(|a, b| {
        let ad = a["isDirectory"].as_bool().unwrap_or(false);
        let bd = b["isDirectory"].as_bool().unwrap_or(false);
        bd.cmp(&ad).then_with(|| {
            a["name"]
                .as_str()
                .unwrap_or("")
                .to_lowercase()
                .cmp(&b["name"].as_str().unwrap_or("").to_lowercase())
        })
    });

    json!({ "entries": rows })
}

pub fn write_text(raw_path: &str, content: &str) -> Result<Value, String> {
    let path = expand(raw_path);
    if content.len() > 1_000_000 {
        return Err("Content too large".into());
    }
    match path.parent() {
        Some(parent) if parent.is_dir() => {}
        _ => return Err("Parent directory does not exist".into()),
    }
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(json!({ "path": path.to_string_lossy() }))
}

pub fn rename(raw_path: &str, new_name: &str) -> Result<Value, String> {
    let source = expand(raw_path);
    let name = new_name.trim();
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err("Invalid rename".into());
    }
    let target = source
        .parent()
        .ok_or_else(|| "Invalid rename".to_string())?
        .join(name);
    if target.exists() {
        return Err(format!("\"{name}\" already exists"));
    }
    std::fs::rename(&source, &target).map_err(|e| e.to_string())?;
    Ok(json!({ "path": target.to_string_lossy() }))
}

/// Move to the OS trash. Uses the platform trash directory on macOS so the
/// delete stays recoverable, like Electron's `shell.trashItem`.
pub fn trash(raw_path: &str) -> Result<bool, String> {
    let source = expand(raw_path);
    if !source.exists() {
        return Err("Path does not exist".into());
    }
    let file_name = source
        .file_name()
        .ok_or_else(|| "Invalid delete".to_string())?
        .to_string_lossy()
        .to_string();

    #[cfg(target_os = "macos")]
    {
        let trash_dir = home().join(".Trash");
        let mut target = trash_dir.join(&file_name);
        let mut counter = 1;
        while target.exists() {
            target = trash_dir.join(format!("{file_name} {counter}"));
            counter += 1;
        }
        std::fs::rename(&source, &target).map_err(|e| e.to_string())?;
        return Ok(true);
    }

    #[cfg(not(target_os = "macos"))]
    {
        std::fs::remove_file(&source)
            .or_else(|_| std::fs::remove_dir_all(&source))
            .map_err(|e| e.to_string())?;
        Ok(true)
    }
}

fn open_with(program: &str, args: &[&str]) -> bool {
    std::process::Command::new(program)
        .args(args)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn reveal(raw_path: &str) -> bool {
    let path = expand(raw_path);
    if !path.exists() {
        return false;
    }
    #[cfg(target_os = "macos")]
    {
        return open_with("open", &["-R", &path.to_string_lossy()]);
    }
    #[cfg(not(target_os = "macos"))]
    {
        open_with("xdg-open", &[&path.parent().unwrap_or(&path).to_string_lossy()])
    }
}

pub fn open_dir(raw_path: &str) -> Value {
    let path = expand(raw_path);
    if path.as_os_str().is_empty() {
        return json!({ "ok": false, "error": "no path" });
    }
    let _ = std::fs::create_dir_all(&path);
    if !path.is_dir() {
        return json!({ "ok": false, "error": "not a directory" });
    }
    #[cfg(target_os = "macos")]
    let opened = open_with("open", &[&path.to_string_lossy()]);
    #[cfg(not(target_os = "macos"))]
    let opened = open_with("xdg-open", &[&path.to_string_lossy()]);

    json!({ "ok": opened, "path": path.to_string_lossy() })
}

pub fn git_root(raw_path: &str) -> Value {
    let mut cursor = expand(raw_path);
    if cursor.is_file() {
        if let Some(parent) = cursor.parent() {
            cursor = parent.to_path_buf();
        }
    }
    loop {
        if cursor.join(".git").exists() {
            return Value::String(cursor.to_string_lossy().to_string());
        }
        match cursor.parent() {
            Some(parent) => cursor = parent.to_path_buf(),
            None => return Value::Null,
        }
    }
}

pub fn desktop_plugins_root(home: &Path) -> String {
    home.join("desktop-plugins").to_string_lossy().to_string()
}

pub fn logs_root(home: &Path, profile: Option<&str>) -> String {
    match profile {
        Some(name) if !name.is_empty() && name != "default" => home
            .join("profiles")
            .join(name)
            .join("logs")
            .to_string_lossy()
            .to_string(),
        _ => home.join("logs").to_string_lossy().to_string(),
    }
}

// --- persisted data-URL cap -------------------------------------------------

fn data_url_cap_path(home: &Path) -> PathBuf {
    home.join("desktop").join("data-url-read-max.json")
}

pub fn data_url_read_max_mb(home: &Path) -> u64 {
    std::fs::read_to_string(data_url_cap_path(home))
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|value| value.get("maxMb").and_then(|v| v.as_u64()))
        .map(|mb| mb.clamp(DATA_URL_MIN_MB, DATA_URL_MAX_MB))
        .unwrap_or(DATA_URL_DEFAULT_MAX_MB)
}

pub fn data_url_read_max_state(home: &Path) -> Value {
    let max_mb = data_url_read_max_mb(home);
    json!({
        "defaultMaxMb": DATA_URL_DEFAULT_MAX_MB,
        "maxMb": max_mb,
        "maxBytes": max_mb * 1024 * 1024
    })
}

pub fn set_data_url_read_max_mb(home: &Path, max_mb: u64) -> Result<Value, String> {
    let clamped = max_mb.clamp(DATA_URL_MIN_MB, DATA_URL_MAX_MB);
    let path = data_url_cap_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, json!({ "maxMb": clamped }).to_string()).map_err(|e| e.to_string())?;
    Ok(data_url_read_max_state(home))
}

// --- composer attachments ---------------------------------------------------

fn composer_images_dir() -> PathBuf {
    home()
        .join("Library/Application Support/Hermes/composer-images")
}

pub fn save_image_buffer(payload: &Value) -> Result<Value, String> {
    let data = payload
        .get("data")
        .ok_or_else(|| "saveImageBuffer: missing data".to_string())?;
    let bytes: Vec<u8> = match data {
        Value::String(text) => base64_decode(text).ok_or_else(|| "invalid base64".to_string())?,
        Value::Array(items) => items
            .iter()
            .filter_map(|v| v.as_u64())
            .map(|n| n as u8)
            .collect(),
        _ => return Err("saveImageBuffer: unsupported data".into()),
    };

    let ext = payload
        .get("ext")
        .and_then(|v| v.as_str())
        .unwrap_or(".png")
        .to_ascii_lowercase();
    let ext = if ext.starts_with('.') && ext.len() <= 6 {
        ext
    } else {
        ".png".to_string()
    };
    let stem = payload
        .get("name")
        .and_then(|v| v.as_str())
        .map(|n| {
            n.rsplit(['/', '\\'])
                .next()
                .unwrap_or(n)
                .rsplit_once('.')
                .map(|(base, _)| base.to_string())
                .unwrap_or_else(|| n.to_string())
        })
        .unwrap_or_default();

    let dir = composer_images_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let safe: String = stem
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
        .collect();
    let safe = safe.trim_matches(|c| c == '.' || c == '_' || c == '-');
    let file_name = if safe.is_empty() {
        format!("composer_{stamp}{ext}")
    } else {
        format!("{safe}_{stamp}{ext}")
    };
    let path = dir.join(file_name);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(Value::String(path.to_string_lossy().to_string()))
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let cleaned: Vec<u8> = text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let cleaned = match cleaned.iter().position(|b| *b == b',') {
        // Tolerate a full data URL as well as bare base64.
        Some(index) if cleaned[..index].contains(&b';') => cleaned[index + 1..].to_vec(),
        _ => cleaned,
    };
    let mut out = Vec::with_capacity(cleaned.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for byte in cleaned {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return None,
        } as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

pub fn save_pasted_text(home: &Path, payload: &Value) -> Result<Value, String> {
    let text = payload
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "savePastedText: missing text".to_string())?;
    if text.is_empty() {
        return Err("savePastedText: missing text".into());
    }
    let dir = home.join("pastes");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = dir.join(format!("pasted_content_{stamp}.txt"));
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(Value::String(path.to_string_lossy().to_string()))
}

// --- clipboard --------------------------------------------------------------

pub fn read_clipboard() -> Result<Value, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    Ok(Value::String(
        clipboard.get_text().unwrap_or_default(),
    ))
}

pub fn write_clipboard(text: &str) -> Result<bool, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text.to_string()).map_err(|e| e.to_string())?;
    Ok(true)
}

// --- dialogs ----------------------------------------------------------------

fn filters_from(options: &Value) -> Vec<(String, Vec<String>)> {
    options
        .get("filters")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item.get("name")?.as_str()?.to_string();
                    let extensions = item
                        .get("extensions")?
                        .as_array()?
                        .iter()
                        .filter_map(|e| e.as_str().map(str::to_string))
                        .collect::<Vec<_>>();
                    (!extensions.is_empty()).then_some((name, extensions))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn select_paths(options: &Value) -> Result<Value, String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(default_path) = options.get("defaultPath").and_then(|v| v.as_str()) {
        let dir = expand(default_path);
        if dir.is_dir() {
            dialog = dialog.set_directory(dir);
        }
    }
    for (name, extensions) in filters_from(options) {
        let refs: Vec<&str> = extensions.iter().map(String::as_str).collect();
        dialog = dialog.add_filter(&name, &refs);
    }
    let multiple = options
        .get("multiple")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        || options
            .get("mode")
            .and_then(|v| v.as_str())
            .map(|mode| mode == "files" || mode == "multi")
            .unwrap_or(false);

    let paths: Vec<String> = if multiple {
        dialog
            .pick_files()
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect()
    } else {
        dialog
            .pick_file()
            .map(|p| vec![p.to_string_lossy().to_string()])
            .unwrap_or_default()
    };
    Ok(json!(paths))
}

pub fn select_save_path(options: &Value) -> Result<Value, String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(title) = options.get("title").and_then(|v| v.as_str()) {
        dialog = dialog.set_title(title);
    }
    if let Some(default_path) = options.get("defaultPath").and_then(|v| v.as_str()) {
        let path = expand(default_path);
        if let Some(parent) = path.parent() {
            if parent.is_dir() {
                dialog = dialog.set_directory(parent);
            }
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            dialog = dialog.set_file_name(name);
        }
    }
    for (name, extensions) in filters_from(options) {
        let refs: Vec<&str> = extensions.iter().map(String::as_str).collect();
        dialog = dialog.add_filter(&name, &refs);
    }
    Ok(match dialog.save_file() {
        Some(path) => Value::String(path.to_string_lossy().to_string()),
        None => Value::Null,
    })
}

pub fn save_text_file_as(request: &Value) -> Result<Value, String> {
    let content = request
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid content".to_string())?;
    if content.len() > 64 * 1024 * 1024 {
        return Err("Content too large".into());
    }
    let chosen = select_save_path(request)?;
    match chosen {
        Value::String(path) => {
            std::fs::write(&path, content).map_err(|e| e.to_string())?;
            Ok(json!({ "saved": true, "path": path }))
        }
        _ => Ok(json!({ "saved": false, "canceled": true })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hermes-fs-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn read_file_text_matches_the_renderer_contract() {
        let dir = scratch("text");
        let file = dir.join("note.md");
        std::fs::write(&file, "# hello\nworld\n").unwrap();

        let value = read_file_text(&file.to_string_lossy());
        assert_eq!(value["text"], "# hello\nworld\n");
        assert_eq!(value["language"], "markdown");
        assert_eq!(value["truncated"], false);
        assert_eq!(value["binary"], false);
        assert_eq!(value["byteSize"], 14);
    }

    #[test]
    fn read_file_text_truncates_and_reports_it() {
        let dir = scratch("big");
        let file = dir.join("big.txt");
        std::fs::write(&file, vec![b'a'; TEXT_PREVIEW_MAX_BYTES as usize + 10]).unwrap();

        let value = read_file_text(&file.to_string_lossy());
        assert_eq!(value["truncated"], true);
        assert_eq!(value["text"].as_str().unwrap().len(), TEXT_PREVIEW_MAX_BYTES as usize);
    }

    #[test]
    fn missing_file_returns_a_structured_error_not_a_panic() {
        let value = read_file_text("/definitely/not/here.txt");
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"], "ENOENT");
        assert!(value["message"].is_string());
    }

    #[test]
    fn data_url_round_trips_and_respects_the_cap() {
        let dir = scratch("dataurl");
        let file = dir.join("tiny.png");
        std::fs::write(&file, [0x89, b'P', b'N', b'G']).unwrap();

        let value = read_file_data_url(&file.to_string_lossy(), 1024);
        let url = value.as_str().expect("data url");
        assert!(url.starts_with("data:image/png;base64,"));
        assert_eq!(base64_decode(url).unwrap(), vec![0x89, b'P', b'N', b'G']);

        let too_small = read_file_data_url(&file.to_string_lossy(), 2);
        assert_eq!(too_small["error"], "ETOOLARGE");
    }

    #[test]
    fn read_dir_sorts_directories_first() {
        let dir = scratch("dir");
        std::fs::create_dir(dir.join("zsub")).unwrap();
        std::fs::write(dir.join("afile.txt"), "x").unwrap();

        let value = read_dir(&dir.to_string_lossy());
        let entries = value["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["name"], "zsub");
        assert_eq!(entries[0]["isDirectory"], true);
        assert_eq!(entries[1]["name"], "afile.txt");
    }

    #[test]
    fn rename_refuses_traversal_and_existing_targets() {
        let dir = scratch("rename");
        let file = dir.join("a.txt");
        std::fs::write(&file, "a").unwrap();

        assert!(rename(&file.to_string_lossy(), "../escape").is_err());
        std::fs::write(dir.join("b.txt"), "b").unwrap();
        assert!(rename(&file.to_string_lossy(), "b.txt").is_err());

        let renamed = rename(&file.to_string_lossy(), "c.txt").unwrap();
        assert!(renamed["path"].as_str().unwrap().ends_with("c.txt"));
    }

    #[test]
    fn write_text_needs_an_existing_parent() {
        let dir = scratch("write");
        let target = dir.join("nested/deep/out.txt");
        assert!(write_text(&target.to_string_lossy(), "x").is_err());

        let ok = write_text(&dir.join("ok.txt").to_string_lossy(), "hi").unwrap();
        assert!(ok["path"].as_str().unwrap().ends_with("ok.txt"));
    }

    #[test]
    fn git_root_walks_up_to_the_repo() {
        let dir = scratch("git");
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let nested = dir.join("a/b/c");
        std::fs::create_dir_all(&nested).unwrap();

        let root = git_root(&nested.to_string_lossy());
        assert_eq!(root.as_str().unwrap(), dir.to_string_lossy());
        assert_eq!(git_root("/"), Value::Null);
    }

    #[test]
    fn data_url_cap_persists_and_clamps() {
        let dir = scratch("cap");
        assert_eq!(data_url_read_max_mb(&dir), DATA_URL_DEFAULT_MAX_MB);
        set_data_url_read_max_mb(&dir, 32).unwrap();
        assert_eq!(data_url_read_max_mb(&dir), 32);
        set_data_url_read_max_mb(&dir, 10_000).unwrap();
        assert_eq!(data_url_read_max_mb(&dir), DATA_URL_MAX_MB);
    }
}
