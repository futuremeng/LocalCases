//! Desktop shell. It is deliberately thin: the engine is linked in and served on
//! loopback with a per-run token, and the webview only ever talks to that API.
//! Nothing here reads files or builds queries — one implementation, one place to
//! audit.
//!
//! `LOCALCASES_REMOTE_API` turns the same window into a client for a library on
//! another machine: no engine is opened here, the loopback service forwards
//! instead. A laptop with no room for the corpus then runs the identical UI, and
//! the content security policy does not have to change, because the webview still
//! only ever sees a loopback address.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::{SocketAddr, TcpListener};

use serde::Serialize;

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

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let Some(addr) = free_addr() else {
        eprintln!("no free loopback port between 5311 and 5313");
        std::process::exit(2);
    };
    let token = localcases::server::random_token();
    let info = ApiInfo {
        addr: format!("http://{addr}"),
        token: token.clone(),
    };

    // Two ways to fill the window: open the library here, or ask someone else's
    // process that did. The front-end cannot tell them apart.
    if let Some(base) = env("LOCALCASES_REMOTE_API") {
        let upstream =
            match localcases::proxy::Upstream::parse(&base, env("LOCALCASES_UPSTREAM_TOKEN")) {
                Ok(upstream) => upstream,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(2);
                }
            };
        tauri::Builder::default()
            .manage(info)
            .setup(move |_app| {
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = localcases::proxy::serve(addr, upstream, Some(token)).await {
                        eprintln!("the forwarder stopped: {e}");
                    }
                });
                Ok(())
            })
            .invoke_handler(tauri::generate_handler![api_info])
            .run(tauri::generate_context!())
            .expect("error while running LocalCases");
        return;
    }

    // LOCALCASES_CONFIG points at the library to open (db + index_path). Without
    // it the shell tells the user what is missing instead of dying on a panic.
    let state = match localcases::server::open_state() {
        Ok(state) => state,
        Err(e) => {
            eprintln!(
                "{e}\nPoint LOCALCASES_CONFIG at a config file (see config_sample.toml), or LOCALCASES_REMOTE_API at a LocalCases service on another machine."
            );
            std::process::exit(2);
        }
    };

    tauri::Builder::default()
        .manage(info)
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
