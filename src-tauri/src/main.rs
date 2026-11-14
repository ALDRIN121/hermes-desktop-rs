#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Rust shell for the Hermes desktop UI.
//!
//! The renderer is the shipped Hermes web bundle, unchanged. What is Rust here
//! is the privileged half Electron used to own: window creation, the preload
//! bridge, local backend discovery, the REST proxy and the `hermes-media`
//! protocol.

mod commands;
mod fs_ops;
mod gateway_ws;
mod git_ops;
mod system_ops;
mod task_ops;
mod terminal;

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{WebviewUrl, WebviewWindowBuilder};

fn hermes_home() -> PathBuf {
    std::env::var_os("HERMES_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".hermes")
        })
}

/// Values Electron used to answer synchronously at preload time, resolved once
/// here and baked into the injected bridge script.
fn bootstrap_script(home: &PathBuf) -> String {
    let prefs = commands::load_preferences(home);
    let skin: Option<serde_json::Value> = std::fs::read_to_string(home.join("skin.json"))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok());

    let boot = serde_json::json!({
        "glassSupported": false,
        "translucencySupported": false,
        "localModelsEnabled": false,
        "assumeFreshInstall": false,
        "selfTest": std::env::var("HERMES_SHELL_SELFTEST").map(|v| v == "1").unwrap_or(false),
        "localSkin": skin,
        "uiPreferences": prefs,
        "home": home,
    });

    include_str!("bridge.js").replace("__HERMES_BOOTSTRAP__", &boot.to_string())
}

/// Open the review/workspace window (`ui/codex.html`), the shell-owned surface
/// that pairs repository status with a diff and per-file git actions.
pub fn open_workspace_window(app: &tauri::AppHandle, selftest: bool) -> Result<(), String> {
    use tauri::Manager;
    if let Some(existing) = app.get_webview_window("workspace") {
        let _ = existing.set_focus();
        return Ok(());
    }
    let mut codex_url = String::from("codex.html");
    let mut params: Vec<String> = Vec::new();
    if selftest {
        params.push("selftest=1".into());
    }
    if let Ok(repo) = std::env::var("HERMES_WORKSPACE_REPO") {
        if !repo.trim().is_empty() {
            params.push(format!("repo={}", urlencoding::encode(repo.trim())));
        }
    }
    if !params.is_empty() {
        codex_url.push('?');
        codex_url.push_str(&params.join("&"));
    }
    let url = tauri::WebviewUrl::App(codex_url.into());
    WebviewWindowBuilder::new(app, "workspace", url)
        .title("Workspace")
        .inner_size(1100.0, 760.0)
        .min_inner_size(640.0, 420.0)
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Bring the chat window back after it was closed to the menu bar.
/// The injected bridge script, kept once so the classic window can be created
/// from the tray after launch (its identity is per-user-home).
static BOOTSTRAP: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn show_main_window(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }
    let Some(script) = BOOTSTRAP.get() else { return };
    let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Hermes (classic)")
        .inner_size(1360.0, 900.0)
        .min_inner_size(720.0, 480.0)
        .initialization_script(script)
        .build();
}

/// Menu-bar presence: a template icon plus Open / Workspace / Quit. This is the
/// app's "always available" affordance, so closing the chat window hides it
/// instead of quitting.
fn install_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let open = MenuItem::with_id(app, "open", "Open Hermes", true, None::<&str>)?;
    let workspace = MenuItem::with_id(app, "workspace", "Workspace review", true, None::<&str>)?;
    let classic = MenuItem::with_id(app, "classic", "Open classic chat", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Hermes Rust", true, Some("Cmd+Q"))?;
    let menu = Menu::with_items(
        app,
        &[&open, &workspace, &classic, &PredefinedMenuItem::separator(app)?, &quit],
    )?;

    let mut builder = TrayIconBuilder::with_id("hermes-tray")
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../icons/tray.png"
        ))?)
        .tooltip("Hermes Rust")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                if let Err(error) = open_task_window(app, false) {
                    eprintln!("task window: {error}");
                }
            }
            "classic" => show_main_window(app),
            "workspace" => {
                if let Err(error) = open_workspace_window(app, false) {
                    eprintln!("workspace window: {error}");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    // macOS renders template images with the menu-bar's own tint, so the glyph
    // follows light/dark automatically.
    #[cfg(target_os = "macos")]
    {
        builder = builder.icon_as_template(true);
    }

    builder.build(app)?;
    Ok(())
}

/// Open the task view (`ui/task.html`): the Codex-style three-pane window —
/// session rail, live transcript, and an Outputs card with tools/subagents.
pub fn open_task_window(app: &tauri::AppHandle, selftest: bool) -> Result<(), String> {
    use tauri::Manager;
    // Query params let a self-test (or the CLI) point the window at a fixture
    // session or repository: `hermes-rust task --session <id> --repo <path>`.
    let mut query = Vec::new();
    if selftest {
        query.push("selftest=1".to_string());
    }
    if std::env::args().any(|arg| arg == "--audit" || arg == "audit") {
        query.push("audit=1".to_string());
    }
    let args: Vec<String> = std::env::args().collect();
    let value_after = |flag: &str| -> Option<String> {
        args.iter()
            .position(|a| a == flag || a == &format!("--{flag}"))
            .and_then(|index| args.get(index + 1))
            .cloned()
    };
    if let Some(session) = value_after("session") {
        query.push(format!("session={}", urlencoding::encode(&session)));
    }
    if let Some(repo) = value_after("repo") {
        query.push(format!("repo={}", urlencoding::encode(&repo)));
    }
    let target = if query.is_empty() {
        "task.html".to_string()
    } else {
        format!("task.html?{}", query.join("&"))
    };
    if let Some(existing) = app.get_webview_window("task") {
        let _ = existing.set_focus();
        let _ = existing.navigate(tauri::Url::parse(&format!("tauri://localhost/{target}")).ok().unwrap_or_else(|| tauri::Url::parse("tauri://localhost/task.html").expect("task url")));
        return Ok(());
    }
    // `--width 1024 --height 700` reproduces a breakpoint for verification.
    let width = value_after("width")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1440.0);
    let height = value_after("height")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(900.0);
    WebviewWindowBuilder::new(app, "task", tauri::WebviewUrl::App(target.into()))
        .title("Task")
        .inner_size(width, height)
        .min_inner_size(900.0, 600.0)
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn main() {
    let home = hermes_home();
    let wants_workspace = std::env::args().any(|arg| arg == "--workspace" || arg == "workspace");
    let wants_task = std::env::args().any(|arg| arg == "--task" || arg == "task");
    let task_selftest = wants_task && std::env::args().any(|arg| arg == "--selftest");

    tauri::Builder::default()
        // A second launch (or `hermes-rust` while the app is open) forwards its
        // arguments here instead of starting a second copy.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if argv.iter().any(|arg| arg == "--task" || arg == "task") {
                // The forwarded call can land while the previous instance is still
                // releasing its window labels; retry once before giving up.
                if open_task_window(app, false).is_err() {
                    std::thread::sleep(std::time::Duration::from_millis(400));
                    let _ = open_task_window(app, false);
                }
            } else if argv.iter().any(|arg| arg == "--workspace" || arg == "workspace") {
                let _ = open_workspace_window(app, false);
            } else {
                show_main_window(app);
            }
        }))
        .manage(commands::ShellState::new(home.clone()))
        .manage(Arc::new(gateway_ws::SocketRegistry::default()))
        .invoke_handler(tauri::generate_handler![
            commands::hermes_ipc,
            commands::hermes_ws_open,
            commands::hermes_ws_send,
            commands::hermes_ws_close,
            commands::open_workspace,
            commands::open_task,
            commands::task_call,
            commands::task_submit,
            commands::task_stats,
            commands::task_audit,
            commands::shell_diag,
            commands::shell_info
        ])
        .register_uri_scheme_protocol("hermes-media", |_app, request| {
            commands::media_response(&request.uri().to_string())
        })
        .setup(move |app| {
            let script = bootstrap_script(&home);
            let _ = BOOTSTRAP.set(script.clone());
            // The shipped React renderer is the *old* UI. It is only built when
            // asked for (`--classic`), so a normal launch shows exactly one
            // window: the Rust workspace view.
            let classic = std::env::args().any(|arg| arg == "--classic" || arg == "classic");
            if classic {
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("Hermes (classic)")
                    .inner_size(1360.0, 900.0)
                    .min_inner_size(720.0, 480.0)
                    .initialization_script(&script)
                    .build()?;
            }

            // The review/workspace window is opt-in on launch (it is a second
            // surface, not a replacement for the chat window).
            let workspace_selftest = std::env::var("HERMES_WORKSPACE_SELFTEST")
                .map(|v| v == "1")
                .unwrap_or(false);
            let workspace_launch = std::env::var("HERMES_WORKSPACE_WINDOW")
                .map(|v| v == "1")
                .unwrap_or(false);
            if workspace_selftest || workspace_launch || wants_workspace {
                open_workspace_window(app.handle(), workspace_selftest)
                    .map_err(std::io::Error::other)?;
            }

            if wants_task || !wants_workspace {
                // A window failure must not abort setup (that left the app running
                // with no task window at all, which reads as "nothing happened").
                if let Err(error) = open_task_window(app.handle(), task_selftest) {
                    let _ = std::fs::write(
                        std::env::temp_dir().join("hermes-rust-boot.log"),
                        format!("open_task_window: {error}"),
                    );
                }
            }

            install_tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the chat window parks the app in the menu bar; Quit lives
            // in the tray menu (and Cmd+Q).
            if window.label() == "main" || window.label() == "task" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("hermes desktop shell failed to start");
}
