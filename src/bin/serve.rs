//! Headless entry point: the same service the desktop shell embeds, so there is
//! only one implementation of "open the library and answer queries".
//!
//! With `LOCALCASES_REMOTE_API` set this process opens no library at all: it
//! forwards to a LocalCases service somewhere else. That is how a machine with no
//! room for the corpus still answers queries, and how the desktop shell on such a
//! machine is started — same binary, same API, no data.

use std::net::SocketAddr;

use tracing::warn;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use localcases::{CONFIG, proxy, server};

#[cfg(not(target_os = "windows"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

/// An environment variable that is set, trimmed, and not empty.
fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

/// When set, every API call must present it as `x-localcases-token`.
fn token() -> Option<String> {
    env("LOCALCASES_TOKEN")
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info,tantivy=warn"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let addr: SocketAddr = CONFIG.addr.parse().expect("`addr` must be host:port");
    let token = token();

    if let Some(base) = env("LOCALCASES_REMOTE_API") {
        // Only `addr` is read from the config in this mode: the library paths in
        // it are never opened, so a remote-mode config can leave them stale.
        let upstream = proxy::Upstream::parse(&base, env("LOCALCASES_UPSTREAM_TOKEN"))
            .expect("`LOCALCASES_REMOTE_API` is not a usable origin");
        if !addr.ip().is_loopback() {
            warn!(
                "forwarding from {addr}, which is reachable beyond this machine: without \
                 LOCALCASES_TOKEN this is an open proxy into the other machine's library"
            );
        }
        if let Err(e) = proxy::serve(addr, upstream, token).await {
            // The usual reason is that another instance is already there, which
            // is a fact worth stating rather than a backtrace worth printing.
            eprintln!("cannot forward from {addr}: {e}");
            std::process::exit(1);
        }
        return;
    }

    if token.is_none() && !addr.ip().is_loopback() {
        warn!(
            "serving on {addr} without LOCALCASES_TOKEN: anyone who can reach this address can \
             read the whole library"
        );
    }
    if let Err(e) = server::serve(addr, token).await {
        eprintln!("cannot serve on {addr}: {e}");
        std::process::exit(1);
    }
}
