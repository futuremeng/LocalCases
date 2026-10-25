//! How the API is exposed: loopback HTTP, optional shared token, CORS for the
//! desktop webview. The desktop shell reuses [router] / [serve] instead of having
//! its own copy of this, so there is one answer to "who may talk to the library".

use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    Router,
    extract::Request,
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use fjall::Config as Library;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    timeout::TimeoutLayer,
};
use tracing::info;

use crate::api;
use crate::config::CONFIG;
use crate::v2::{P_CASES, P_FIELDS};
use crate::{AppState, Search};

/// Origins a webview may call from: the packaged shell and `tauri dev`.
const ALLOWED_ORIGINS: &[&str] = &[
    "tauri://localhost",
    "http://tauri.localhost",
    "https://tauri.localhost",
    "http://localhost:1420",
    "http://127.0.0.1:1420",
];

/// The service listens on loopback, which a page in the user's browser can still
/// reach. A token the page cannot guess closes that: the shell passes it in the
/// environment when it spawns us, or generates one for itself when it embeds us.
async fn gate(token: Arc<String>, request: Request, next: Next) -> Response {
    let given = request
        .headers()
        .get("x-localcases-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if given == token.as_str() {
        next.run(request).await
    } else {
        (StatusCode::UNAUTHORIZED, "missing or wrong token").into_response()
    }
}

pub fn router(state: AppState, token: Option<String>) -> Router {
    let app = match token {
        Some(token) => {
            let token = Arc::new(token);
            api::routes(state).layer(middleware::from_fn(move |req, next| {
                let token = Arc::clone(&token);
                async move { gate(token, req, next).await }
            }))
        }
        None => api::routes(state),
    };
    app.layer(
        ServiceBuilder::new()
            .layer(CompressionLayer::new())
            .layer(TimeoutLayer::with_status_code(
                StatusCode::REQUEST_TIMEOUT,
                Duration::from_secs(60),
            ))
            .layer(
                CorsLayer::new()
                    .allow_origin(AllowOrigin::list(
                        ALLOWED_ORIGINS
                            .iter()
                            .map(|o| o.parse().expect("a valid origin constant")),
                    ))
                    .allow_headers([
                        header::CONTENT_TYPE,
                        header::HeaderName::from_static("x-localcases-token"),
                    ]),
            ),
    )
}

/// Open the library and index named in `CONFIG` and serve until the process ends.
/// `port` 0 lets the OS pick one, which is what the desktop shell wants.
pub fn open_state() -> Result<AppState, String> {
    let searcher = Search::searcher().map_err(|e| format!("cannot open the index: {e}"))?;
    let library = Library::new(CONFIG.db.as_str())
        .open()
        .map_err(|e| format!("cannot open the library: {e}"))?;
    // Opened with the same create options the importer uses, so a service pointed
    // at a half-built library gets the same partitions instead of a silently
    // different layout. The `Keyspace` itself goes into the state: fjall drops the
    // file-handle table when the last clone of it is gone, which would otherwise
    // abort the first read that needs a segment file.
    let records = library
        .open_partition(P_CASES, crate::record_partition_options())
        .map_err(|e| format!("library has no records partition: {e}"))?;
    let derived = library
        .open_partition(P_FIELDS, crate::record_partition_options())
        .map_err(|e| format!("library has no derived partition: {e}"))?;
    Ok(AppState {
        records,
        derived,
        searcher: Arc::new(searcher),
        library,
    })
}

/// Serve `state` until the process ends. The desktop shell embeds this; the
/// headless binary calls it too, so there is one serving path.
pub async fn serve_at(
    state: AppState,
    addr: SocketAddr,
    token: Option<String>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!("listening on http://{}", listener.local_addr()?);
    axum::serve(listener, router(state, token)).await
}

/// Open the configured library, then serve it.
pub async fn serve(addr: SocketAddr, token: Option<String>) -> std::io::Result<()> {
    let state = open_state().expect("cannot open the library");
    serve_at(state, addr, token).await
}

/// A fresh secret for the token gate, from the operating system CSPRNG.
pub fn random_token() -> String {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).expect("no system randomness");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use tower::ServiceExt;

    fn gated(token: Option<&str>) -> Router {
        let app = Router::new().route("/ping", get(|| async { "pong" }));
        match token {
            Some(t) => {
                let token = Arc::new(t.to_owned());
                app.layer(middleware::from_fn(move |req, next| {
                    let token = Arc::clone(&token);
                    async move { gate(token, req, next).await }
                }))
            }
            None => app,
        }
    }

    async fn status(app: Router, with_header: Option<&str>) -> StatusCode {
        let mut builder = axum::http::Request::builder().method("GET").uri("/ping");
        if let Some(h) = with_header {
            builder = builder.header("x-localcases-token", h);
        }
        let request = builder.body(axum::body::Body::empty()).unwrap();
        app.oneshot(request).await.unwrap().status()
    }

    #[tokio::test]
    async fn no_token_configured_means_open() {
        assert_eq!(status(gated(None), None).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn token_required_and_checked() {
        assert_eq!(
            status(gated(Some("s3cret")), None).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status(gated(Some("s3cret")), Some("wrong")).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status(gated(Some("s3cret")), Some("s3cret")).await,
            StatusCode::OK
        );
    }

    #[test]
    fn token_is_hex_and_long_enough() {
        let t = random_token();
        assert_eq!(t.len(), 48);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// fjall drops a keyspace's file-handle table when the last clone of the
    /// `Keyspace` goes away, which is why the service state owns one. Reading
    /// back a row that lives in the value log (over the separation threshold) is
    /// the operation that used to abort with `should acquire file handle`.
    #[test]
    fn a_partition_stays_usable_while_its_state_holds_the_library() {
        struct Holder {
            records: fjall::PartitionHandle,
            // Declared last, so dropped last.
            library: fjall::Keyspace,
        }

        let dir = std::env::temp_dir().join(format!("localcases-keyspace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let library = Library::new(dir.clone()).open().unwrap();
        let records = library
            .open_partition(P_CASES, crate::record_partition_options())
            .unwrap();
        let row = vec![b'x'; 8_000];
        records.insert(7u32.to_be_bytes(), row.clone()).unwrap();
        library.force_flush().unwrap();

        let holder = Holder { records, library };
        // Going through the holder's own handle is the point: the partitions are
        // only usable while something owns the keyspace.
        holder.library.force_flush().unwrap();
        let got = holder.records.get(7u32.to_be_bytes()).unwrap().unwrap();
        assert_eq!(got.as_ref(), row.as_slice());

        drop(holder);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
