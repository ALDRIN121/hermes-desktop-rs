#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Rust shell for the Hermes desktop UI.
//!
//! The renderer is built from the pinned Hermes source with the approved UI.
//! What is Rust here
//! is the privileged half Electron used to own: window creation, the preload
//! bridge, local backend discovery, the REST proxy and the `hermes-media`
//! protocol.

mod commands;
mod desktop_ops;
mod fs_ops;
mod gateway_ws;
mod git_ops;
mod launch;
mod oauth_callback;
mod plugin_dial;
mod prepared_journal;
mod preview_watch;
mod system_ops;
mod task_ops;
mod terminal;
mod windows;

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{WebviewUrl, WebviewWindowBuilder};

pub fn hermes_home() -> PathBuf {
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
        "selfTest": cfg!(debug_assertions) && std::env::var("HERMES_SHELL_TEST_INSTANCE").is_ok() && std::env::var("HERMES_SHELL_SELFTEST").map(|v| v == "1").unwrap_or(false),
        "mediaSelfTest": cfg!(debug_assertions) && std::env::var("HERMES_SHELL_TEST_INSTANCE").is_ok() && std::env::var("HERMES_SHELL_MEDIA_SELFTEST").map(|v| v == "1").unwrap_or(false),
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
        let _ = existing.show();
        let _ = existing.unminimize();
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

/// `--route <path>` opens one in-app screen directly, so a review can inspect a
/// destination in the real window without driving the UI.
static MAIN_ROUTE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn set_main_route(route: Option<String>) {
    if let Ok(mut guard) = MAIN_ROUTE.lock() {
        *guard = route;
    }
}

fn apply_route(window: &tauri::WebviewWindow) {
    let route = MAIN_ROUTE.lock().ok().and_then(|guard| guard.clone());
    let Some(route) = route else {
        return;
    };
    let literal = serde_json::to_string(&route).unwrap_or_else(|_| "\"\"".to_owned());
    let _ = window.eval(&format!("location.hash = {literal};"));
}

pub fn show_main_window(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        apply_route(&window);
        return;
    }
    let Some(script) = BOOTSTRAP.get() else {
        return;
    };
    let built = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Hermes")
        .inner_size(1360.0, 900.0)
        .min_inner_size(720.0, 480.0)
        .initialization_script(script)
        .on_page_load(|window, payload| {
            // Historical firstPaintMs records document-load completion, not a
            // compositor paint or React readiness. See PERFORMANCE.md.
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                crate::commands::stamp_first_paint();
            }
            let _ = window;
        })
        .build();
    if let Ok(window) = built {
        apply_route(&window);
    }
}

/// Menu-bar presence: a template icon plus Open / Workspace / Quit. This is the
/// app's "always available" affordance, so closing the chat window hides it
/// instead of quitting.
fn install_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let open = MenuItem::with_id(app, "open", "Open Hermes", true, None::<&str>)?;
    let workspace = MenuItem::with_id(app, "workspace", "Workspace review", true, None::<&str>)?;
    let classic = MenuItem::with_id(
        app,
        "task-preview",
        "Legacy task preview",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit Hermes Rust", true, Some("Cmd+Q"))?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &workspace,
            &classic,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("hermes-tray")
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../icons/tray.png"
        ))?)
        .tooltip("Hermes Rust")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main_window(app),
            "task-preview" => {
                if let Err(error) = open_task_window(app, false) {
                    eprintln!("task window: {error}");
                }
            }
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
    open_task_window_with_args(app, selftest, &[])
}

fn open_task_window_with_args(
    app: &tauri::AppHandle,
    selftest: bool,
    args: &[String],
) -> Result<(), String> {
    use tauri::Manager;
    // Query params let a self-test (or the CLI) point the window at a fixture
    // session or repository: `hermes-rust task --session <id> --repo <path>`.
    let mut args = args.to_vec();
    if selftest {
        args.push("--selftest".into());
    }
    let launch = launch::parse(&args);
    let target = launch.task_url;
    if let Some(existing) = app.get_webview_window("task") {
        let _ = existing.show();
        let _ = existing.unminimize();
        let _ = existing.set_focus();
        if target != "task.html" {
            let _ = existing.navigate(
                tauri::Url::parse(&format!("tauri://localhost/{target}"))
                    .map_err(|e| e.to_string())?,
            );
        }
        return Ok(());
    }
    // `--width 1024 --height 700` reproduces a breakpoint for verification.
    WebviewWindowBuilder::new(app, "task", tauri::WebviewUrl::App(target.into()))
        .title("Task")
        .inner_size(launch.width, launch.height)
        .min_inner_size(900.0, 600.0)
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn main() {
    let _ = commands::STARTED.set(std::time::Instant::now());
    let home = hermes_home();
    let initial_args: Vec<String> = std::env::args().collect();
    let parsed = launch::parse(&initial_args);
    let view = parsed.view;
    set_main_route(parsed.route);
    let mut context = tauri::generate_context!();
    // A debug-only fixture can coexist with the user's app. Its home must be
    // inside the OS temporary directory; it never changes the installed app.
    #[cfg(debug_assertions)]
    if let Ok(instance) = std::env::var("HERMES_SHELL_TEST_INSTANCE") {
        let temporary = std::env::temp_dir()
            .canonicalize()
            .expect("temporary directory");
        let fixture_home = home.canonicalize().expect("fixture home must exist");
        assert!(
            fixture_home.starts_with(&temporary)
                && !instance.is_empty()
                && instance
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "invalid isolated fixture instance"
        );
        context.config_mut().identifier = format!("ai.nous.hermes.rust-shell.fixture-{instance}");
    }

    tauri::Builder::default()
        // A second launch (or `hermes-rust` while the app is open) forwards its
        // arguments here instead of starting a second copy.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if launch::parse(&argv).view == launch::View::Task {
                // The forwarded call can land while the previous instance is still
                // releasing its window labels; retry once before giving up.
                if open_task_window_with_args(app, false, &argv).is_err() {
                    std::thread::sleep(std::time::Duration::from_millis(400));
                    let _ = open_task_window_with_args(app, false, &argv);
                }
            } else if launch::parse(&argv).view == launch::View::Workspace {
                let _ = open_workspace_window(app, false);
            } else {
                set_main_route(launch::parse(&argv).route);
                show_main_window(app);
            }
        }))
        .manage(commands::ShellState::new(home.clone()))
        .manage(windows::WindowRoutes::default())
        .manage(Arc::new(prepared_journal::PreparedJournal::default()))
        .manage(plugin_dial::PreparedDials::default())
        .manage(preview_watch::PreviewWatches::default())
        .manage(Arc::new(oauth_callback::CallbackRegistry::default()))
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
            let range = request
                .headers()
                .get("range")
                .and_then(|value| value.to_str().ok());
            commands::media_response(&request.uri().to_string(), range)
        })
        .setup(move |app| {
            let script = bootstrap_script(&home);
            let _ = BOOTSTRAP.set(script.clone());
            // One primary app. Legacy shell views are explicit comparison tools.
            if view == launch::View::Main {
                show_main_window(app.handle());
            }

            // The review/workspace window is opt-in on launch (it is a second
            // surface, not a replacement for the chat window).
            let workspace_selftest = std::env::var("HERMES_WORKSPACE_SELFTEST")
                .map(|v| v == "1")
                .unwrap_or(false);
            let workspace_launch = std::env::var("HERMES_WORKSPACE_WINDOW")
                .map(|v| v == "1")
                .unwrap_or(false);
            if workspace_selftest || workspace_launch || view == launch::View::Workspace {
                open_workspace_window(app.handle(), workspace_selftest)
                    .map_err(std::io::Error::other)?;
            }

            if view == launch::View::Task {
                // A window failure must not abort setup (that left the app running
                // with no task window at all, which reads as "nothing happened").
                if let Err(error) = open_task_window_with_args(app.handle(), false, &initial_args) {
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
            if let tauri::WindowEvent::Destroyed = event {
                use tauri::Manager;
                window
                    .state::<windows::WindowRoutes>()
                    .forget(window.label());
                window
                    .state::<plugin_dial::PreparedDials>()
                    .close_window(window.label());
                window
                    .state::<preview_watch::PreviewWatches>()
                    .close_window(window.label());
                window
                    .state::<Arc<oauth_callback::CallbackRegistry>>()
                    .close_window(window.label());
                window
                    .state::<Arc<gateway_ws::SocketRegistry>>()
                    .close_window(window.label());
            }
            // Closing the chat window parks the app in the menu bar; Quit lives
            // in the tray menu (and Cmd+Q).
            if window.label() == "main" || window.label() == "task" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(context)
        .expect("hermes desktop shell failed to start");
}
