//! Desktop shell. It is deliberately thin: the engine is linked in and served on
//! loopback with a per-run token, and the webview only ever talks to that API.
//! Nothing here reads files or builds queries — one implementation, one place to
//! audit.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::{SocketAddr, TcpListener};

use serde::Serialize;

/// Where the embedded search service lives, handed to the webview on request.
#[derive(Clone, Serialize)]
struct ApiInfo {
    addr: String,
    token: String,
}

#[tauri::command]
fn api_info(info: tauri::State<'_, ApiInfo>) -> ApiInfo {
    info.inner().clone()
}

/// The port range is fixed so the content security policy can name it; the scan
/// is there for the second instance the user already has open.
fn free_addr() -> Option<SocketAddr> {
    (5311..=5313).find_map(|port| {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        TcpListener::bind(addr).ok().map(|_| addr)
    })
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // LOCALCASES_CONFIG points at the library to open (db + index_path). Without
    // it the shell tells the user what is missing instead of dying on a panic.
    let state = match localcases::server::open_state() {
        Ok(state) => state,
        Err(e) => {
            eprintln!("{e}\nPoint LOCALCASES_CONFIG at a config file (see config_sample.toml).");
            std::process::exit(2);
        }
    };
    let Some(addr) = free_addr() else {
        eprintln!("no free loopback port between 5311 and 5313");
        std::process::exit(2);
    };
    let token = localcases::server::random_token();

    tauri::Builder::default()
        .manage(ApiInfo {
            addr: format!("http://{addr}"),
            token: token.clone(),
        })
        .setup(move |_app| {
            tauri::async_runtime::spawn(async move {
                if let Err(e) = localcases::server::serve_at(state, addr, Some(token)).await {
                    eprintln!("search service stopped: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![api_info])
        .run(tauri::generate_context!())
        .expect("error while running LocalCases");
}
