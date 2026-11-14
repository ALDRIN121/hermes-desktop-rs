//! Wire types shared with the renderer (see `apps/desktop/src/global.d.ts`).

use serde::{Deserialize, Serialize};

/// `HermesConnection` in `src/global.d.ts`. Field names are camelCase on the
/// wire because the renderer consumes them verbatim.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HermesConnection {
    pub base_url: String,
    pub is_fullscreen: bool,
    pub native_overlay_width: f64,
    pub token: String,
    pub ws_url: String,
    pub logs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_window_controls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub darwin_major: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_maximized: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_identity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_hermes_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub registry_scoped: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub shared_primary: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub shared_remote: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_button_position: Option<WindowButtonPosition>,
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowButtonPosition {
    pub x: f64,
    pub y: f64,
}

/// `HermesApiRequest` in `src/global.d.ts`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HermesApiRequest {
    pub path: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub body: Option<serde_json::Value>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub profile: Option<serde_json::Value>,
    #[serde(default)]
    pub connection_id: Option<serde_json::Value>,
    #[serde(default)]
    pub priority: Option<String>,
}

/// The subtree of `hermes gateway attach` output this shell consumes.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AttachReceipt {
    pub endpoint: AttachEndpoint,
    pub url: String,
    #[serde(default)]
    pub ticket: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AttachEndpoint {
    pub api_origin: String,
    #[serde(default)]
    pub instance_id: Option<String>,
    #[serde(default)]
    pub supervisor: Option<String>,
    #[serde(default)]
    pub profile_id: Option<String>,
}

/// UI preferences snapshot shared by the native app and the webapp
/// (`~/.hermes/ui-preferences.json`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiPreferences {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub generation: String,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub values: serde_json::Map<String, serde_json::Value>,
}

fn default_version() -> u32 {
    1
}
