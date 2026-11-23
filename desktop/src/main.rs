//! Desktop shell. It is deliberately thin: the engine is linked in and served on
//! loopback with a per-run token, and the webview only ever talks to that API.
//! Nothing here reads files or builds queries — one implementation, one place to
//! audit. The one exception is [save_csv], which shows the native save panel:
//! a WKWebView cannot download a blob, so the bytes have to leave through here.
//!
//! `LOCALCASES_REMOTE_API` turns the same window into a client for a library on
//! another machine: no engine is opened here, the loopback service forwards
//! instead. A laptop with no room for the corpus then runs the identical UI, and
//! the content security policy does not have to change, because the webview still
//! only ever sees a loopback address.
//!
//! The window follows the Mac conventions: a real menu bar with the app's
//! commands and shortcuts, one instance per machine, a window that comes back
//! when the Dock icon is clicked after the last one was closed, and a remembered
//! size and position.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::io::{self, Write};
use std::net::{SocketAddr, TcpListener};
use std::panic;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
#[cfg(target_os = "macos")]
use tauri::RunEvent;
use tauri::menu::{
    AboutMetadata, HELP_SUBMENU_ID, Menu, MenuItem, PredefinedMenuItem, Submenu, SubmenuBuilder,
    WINDOW_SUBMENU_ID,
};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

/// Where the search service lives, handed to the webview on request.
#[derive(Clone, Serialize)]
struct ApiInfo {
    addr: String,
    token: String,
}

#[tauri::command]
fn api_info(info: tauri::State<'_, ApiInfo>) -> ApiInfo {
    info.inner().clone()
}

/// Write CSV bytes out through the native save panel. The front-end fetches the
/// export from the loopback API and hands the bytes here; a blob download from
/// inside a WKWebView would silently go nowhere.
#[tauri::command]
async fn save_csv(
    app: tauri::AppHandle,
    name: String,
    data: Vec<u8>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(&name)
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let path = rx.await.map_err(|_| "the save dialog closed".to_string())?;
    match path {
        Some(path) => {
            let path = path
                .into_path()
                .map_err(|e| format!("the chosen path is not usable: {e}"))?;
            std::fs::write(&path, &data).map_err(|e| format!("cannot write the file: {e}"))?;
            Ok(Some(path.display().to_string()))
        }
        None => Ok(None),
    }
}

/// An environment variable that is set, trimmed, and not empty.
fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

/// The port range is fixed so the content security policy can name it; the scan
/// is there for the second instance the user already has open.
fn free_addr() -> Option<SocketAddr> {
    (5311..=5313).find_map(|port| {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        TcpListener::bind(addr).ok().map(|_| addr)
    })
}

/// The directory the shell keeps its own files in: the config a fresh install
/// gets and the crash log. `LOCALCASES_APP_DATA` overrides it for tests and
/// for anyone who wants the library somewhere else.
fn app_data_dir() -> PathBuf {
    if let Some(dir) = env("LOCALCASES_APP_DATA") {
        return PathBuf::from(dir);
    }
    const ID: &str = "org.localcases.desktop";
    #[cfg(target_os = "macos")]
    let base = PathBuf::from(env("HOME").unwrap_or_default())
        .join("Library")
        .join("Application Support");
    #[cfg(target_os = "windows")]
    let base = PathBuf::from(env("APPDATA").unwrap_or_default());
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let base = match env("XDG_DATA_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env("HOME").unwrap_or_default())
            .join(".local")
            .join("share"),
    };
    base.join(ID)
}

/// The config a fresh install gets, so the window opens on an empty library
/// instead of failing on a missing file: the library and the index live next
/// to the config in the app-data directory.
pub fn create_default_config(dir: &Path) -> io::Result<PathBuf> {
    let file = dir.join("config.toml");
    // Debug-formatting quotes the paths and escapes separators, which is
    // valid TOML on every platform, Windows backslashes included.
    let text = format!(
        "# written by the desktop shell on first run\ndb = {:?}\nindex_path = {:?}\naddr = \"127.0.0.1:0\"\n",
        dir.join("library"),
        dir.join("index")
    );
    fs::create_dir_all(dir)?;
    fs::write(&file, text)?;
    Ok(file)
}

/// Make sure a local-mode launch has a config to open. Development runs keep
/// their own setup: an explicit `LOCALCASES_CONFIG` or a `config.toml` in the
/// working directory wins and nothing is written. Otherwise the shell writes
/// the default into the app-data directory and points the engine at it, so a
/// fresh install opens on an empty library instead of an error.
fn ensure_local_config() {
    if env("LOCALCASES_CONFIG").is_some() || Path::new("config.toml").exists() {
        return;
    }
    let dir = app_data_dir();
    let file = dir.join("config.toml");
    if !file.exists() {
        if let Err(e) = create_default_config(&dir) {
            report_failure(&format!("cannot create the default config: {e}"));
        }
    }
    // SAFETY: this runs in `main` before any thread or window exists, so no
    // other code can observe the environment while it changes.
    unsafe { std::env::set_var("LOCALCASES_CONFIG", &file) };
}

/// Seconds since the epoch, for the crash log's timestamps.
fn epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// Say plainly why the window is not coming, leave the same line in the crash
/// log next to the app's other state, and exit.
fn report_failure(msg: &str) -> ! {
    eprintln!("{msg}");
    let dir = app_data_dir();
    let _ = fs::create_dir_all(&dir);
    if let Ok(mut log) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("crash.log"))
    {
        let _ = writeln!(log, "{}s shell exit: {msg}", epoch());
    }
    std::process::exit(2);
}

/// A panic with no window is otherwise silent: every panic now appends a
/// line and a backtrace to the crash log before the default hook prints it.
/// Nothing here swallows a panic.
fn install_crash_log() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let dir = app_data_dir();
        let _ = fs::create_dir_all(&dir);
        if let Ok(mut log) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("crash.log"))
        {
            let _ = writeln!(
                log,
                "{}s LocalCases {} panicked: {}\n{}",
                epoch(),
                env!("CARGO_PKG_VERSION"),
                info,
                std::backtrace::Backtrace::force_capture()
            );
        }
        previous(info);
    }));
}

/// What fills the window: open the library here, or ask someone else's process
/// that did. The front-end cannot tell them apart.
enum Mode {
    Local(localcases::AppState),
    Remote(localcases::proxy::Upstream),
}

/// The menu bar is where a Mac user looks for a feature they cannot find. Every
/// item here maps to a front-end action through the `menu` event; the front-end
/// also answers the same shortcuts when the UI runs in a plain browser.
fn build_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let about = PredefinedMenuItem::about(
        app,
        Some("关于 LocalCases"),
        Some(AboutMetadata {
            name: Some("LocalCases".into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
            copyright: Some("LocalCases contributors".into()),
            license: Some("Apache-2.0".into()),
            comments: Some("判决文书本地检索".into()),
            ..Default::default()
        }),
    )?;
    let app_menu = Submenu::with_items(
        app,
        "LocalCases",
        true,
        &[
            &about,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some("隐藏 LocalCases"))?,
            &PredefinedMenuItem::hide_others(app, Some("隐藏其他"))?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, Some("退出 LocalCases"))?,
        ],
    )?;
    let file_menu = Submenu::with_items(
        app,
        "文件",
        true,
        &[
            &MenuItem::with_id(app, "export", "导出 CSV…", true, Some("CmdOrCtrl+E"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, Some("关闭窗口"))?,
        ],
    )?;
    let edit_menu = SubmenuBuilder::new(app, "编辑")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let view_menu = Submenu::with_items(
        app,
        "显示",
        true,
        &[
            &MenuItem::with_id(app, "focus-search", "聚焦搜索框", true, Some("CmdOrCtrl+F"))?,
            &MenuItem::with_id(
                app,
                "toggle-facets",
                "显示/隐藏筛选侧栏",
                true,
                Some("CmdOrCtrl+Alt+S"),
            )?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::fullscreen(app, Some("全屏"))?,
        ],
    )?;
    // The Window submenu carries the id Tauri and macOS use to keep the window
    // list current; Help carries the id macOS appends its search field to.
    let window_menu = SubmenuBuilder::with_id(app, WINDOW_SUBMENU_ID, "窗口")
        .minimize()
        .maximize()
        .separator()
        .item(&PredefinedMenuItem::close_window(app, Some("关闭窗口"))?)
        .build()?;
    let help_menu = SubmenuBuilder::with_id(app, HELP_SUBMENU_ID, "帮助")
        .text("help", "配置与使用说明")
        .build()?;
    Menu::with_items(
        app,
        &[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
            &window_menu,
            &help_menu,
        ],
    )
}

/// Dock icon clicked with no window on screen: bring the window back, making a
/// fresh one if the user had closed the last.
fn show_or_create_main(app: &tauri::AppHandle) {
    match app.get_webview_window("main") {
        Some(window) => {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
        None => {
            let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("LocalCases")
                .inner_size(1240.0, 820.0)
                .min_inner_size(860.0, 520.0)
                .build();
        }
    }
}

fn main() {
    install_crash_log();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let Some(addr) = free_addr() else {
        report_failure("no free loopback port between 5311 and 5313");
    };
    let token = localcases::server::random_token();
    let info = ApiInfo {
        addr: format!("http://{addr}"),
        token: token.clone(),
    };

    // Two ways to fill the window: open the library here, or ask someone else's
    // process that did. The front-end cannot tell them apart. LOCALCASES_CONFIG
    // points at the library to open (db + index_path); without it the shell tells
    // the user what is missing instead of dying on a panic.
    let mode = if let Some(base) = env("LOCALCASES_REMOTE_API") {
        match localcases::proxy::Upstream::parse(&base, env("LOCALCASES_UPSTREAM_TOKEN")) {
            Ok(upstream) => Mode::Remote(upstream),
            Err(e) => report_failure(&format!("{e}")),
        }
    } else {
        // A fresh install has no config anywhere yet: give it one pointing at
        // an empty library in the app-data directory before opening it.
        ensure_local_config();
        match localcases::server::open_state() {
            Ok(state) => Mode::Local(state),
            Err(e) => report_failure(&format!(
                "{e}\nPoint LOCALCASES_CONFIG at a config file (see config_sample.toml), or LOCALCASES_REMOTE_API at a LocalCases service on another machine."
            )),
        }
    };

    let builder = tauri::Builder::default()
        // A second launch focuses the window that exists instead of opening a
        // second engine and a second window.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_or_create_main(app);
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(info)
        .invoke_handler(tauri::generate_handler![api_info, save_csv])
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if matches!(id, "export" | "toggle-facets" | "focus-search" | "help") {
                let _ = app.emit("menu", id);
            }
        })
        .setup(move |app| {
            app.set_menu(build_menu(app.handle())?)?;
            let token = Some(token);
            match mode {
                Mode::Local(state) => {
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = localcases::server::serve_at(state, addr, token).await {
                            eprintln!("search service stopped: {e}");
                        }
                    });
                }
                Mode::Remote(upstream) => {
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = localcases::proxy::serve(addr, upstream, token).await {
                            eprintln!("the forwarder stopped: {e}");
                        }
                    });
                }
            }
            Ok(())
        });

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while running LocalCases");
    app.run(|app_handle, event| {
        #[cfg(target_os = "macos")]
        {
            // Closing the last window leaves the app in the Dock, the way a Mac app
            // should; clicking it then lands here. Reopen is a macOS-only RunEvent
            // variant — Linux/Windows builds must not name it or they fail to compile.
            if let RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } = event
            {
                show_or_create_main(app_handle);
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (&app_handle, &event);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // The tests move the process environment, which is shared; one lock keeps
    // them from tripping over each other.
    static LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn fresh_install_gets_a_default_config_and_opens() {
        let _guard = LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("localcases-desktop-test-{}", epoch()));
        // SAFETY: the lock above keeps every other test in this binary still.
        unsafe {
            std::env::set_var("LOCALCASES_APP_DATA", &dir);
            std::env::remove_var("LOCALCASES_CONFIG");
        }
        // The dev escape hatch must be off for the fresh-install path to run;
        // `cargo test` runs from `desktop/`, where no config.toml exists.
        assert!(!Path::new("config.toml").exists());
        ensure_local_config();
        let file = dir.join("config.toml");
        assert!(file.exists(), "the config should have been created");
        let text = fs::read_to_string(&file).unwrap();
        assert!(
            text.contains(&dir.join("library").display().to_string()),
            "the config should point at a library next to it: {text}"
        );
        assert!(
            localcases::server::open_state().is_ok(),
            "an empty library should open, not fail the window"
        );
        // SAFETY: the lock still holds.
        unsafe { std::env::remove_var("LOCALCASES_APP_DATA") };
        let _ = fs::remove_dir_all(&dir);
    }
}
