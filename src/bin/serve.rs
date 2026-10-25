//! Headless entry point: the same service the desktop shell embeds, so there is
//! only one implementation of "open the library and answer queries".

use std::net::SocketAddr;

use tracing::warn;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use localcases::{CONFIG, server};

#[cfg(not(target_os = "windows"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

/// When set, every API call must present it as `x-localcases-token`.
fn token() -> Option<String> {
    std::env::var("LOCALCASES_TOKEN")
        .ok()
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info,tantivy=warn"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let addr: SocketAddr = CONFIG.addr.parse().expect("`addr` must be host:port");
    let token = token();
    if token.is_none() && !addr.ip().is_loopback() {
        warn!(
            "serving on {addr} without LOCALCASES_TOKEN: anyone who can reach this \
             address can read the whole library"
        );
    }
    server::serve(addr, token)
        .await
        .expect("cannot start the service");
}
