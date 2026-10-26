//! The other half of "the data lives somewhere else": a loopback forwarder that
//! speaks the same API surface as [crate::server] but answers by asking a
//! LocalCases service on another machine.
//!
//! It exists so a small laptop can hold no corpus at all. The webview keeps
//! talking to `127.0.0.1:<port>` — the address its content security policy
//! already names — and the secret the remote service expects never reaches the
//! webview: it is attached here, after the local gate has let the request through.
//!
//! Deliberately minimal: HTTP/1.1, no TLS, one request per connection. Plain
//! HTTP on a network you control is the threat model the token gate was written
//! for; further than that, put a tunnel in front (`ssh -L 5010:server:5010 host`)
//! and point this at the tunnel.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

/// Request heads above this are refused rather than buffered.
const MAX_HEAD: usize = 64 * 1024;
/// Same for a request body. The API has no upload route today; the cap is here
/// so a stray POST cannot make the process allocate without bound.
const MAX_BODY: usize = 8 * 1024 * 1024;
/// One exchange, from accepting the client to the last response byte.
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(300);

/// The service to forward to: an `http://host:port[/prefix]` origin plus the
/// token that origin expects.
#[derive(Clone, Debug)]
pub struct Upstream {
    host: String,
    port: u16,
    /// prepended to every request path, for a service behind a path prefix
    prefix: String,
    token: Option<String>,
}

impl Upstream {
    /// The port is required — guessing 80 is how a token ends up at a machine
    /// nobody meant to send it to.
    pub fn parse(base: &str, token: Option<String>) -> Result<Self, String> {
        let trimmed = base.trim().trim_end_matches('/');
        let Some(rest) = trimmed.strip_prefix("http://") else {
            return Err(format!(
                "`{base}` is not an http:// origin. This forwarder speaks plain HTTP: use it on \
                 a network you control, or tunnel (`ssh -L 5010:host:5010 host`) and point it at \
                 the tunnel"
            ));
        };
        let (authority, prefix) = match rest.find('/') {
            Some(at) => (&rest[..at], rest[at..].to_owned()),
            None => (rest, String::new()),
        };
        let (host, port) = authority
            .rsplit_once(':')
            .ok_or_else(|| format!("`{base}` names no port; name one, e.g. http://host:5010"))?;
        if host.is_empty() {
            return Err(format!("`{base}` names no host"));
        }
        let port = port
            .parse()
            .map_err(|_| format!("`{base}` has a port that is not a number"))?;
        Ok(Self {
            host: host.to_owned(),
            port,
            prefix,
            token,
        })
    }

    fn authority(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Where the request head ends: just past the blank line.
fn head_end(request: &[u8]) -> Option<usize> {
    request
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|at| at + 4)
}

/// The value of the first header line named `name`, ignoring case. Matched only
/// at the start of a line, so a header whose value mentions another header name
/// cannot be mistaken for it.
fn header_value(head: &[u8], name: &str) -> Option<String> {
    let name = name.as_bytes();
    for line in head.split(|b| *b == b'\r') {
        let line = line.strip_prefix(b"\n").unwrap_or(line);
        let mut parts = line.splitn(2, |b| *b == b':');
        let (Some(key), Some(value)) = (parts.next(), parts.next()) else {
            continue;
        };
        if key.eq_ignore_ascii_case(name) {
            let value = String::from_utf8_lossy(value).trim().to_owned();
            return (!value.is_empty()).then_some(value);
        }
    }
    None
}

/// The head minus the request line and the headers this hop owns, so the
/// upstream service sees exactly one answer for each of those.
fn forwarded_headers(head: &[u8]) -> Vec<u8> {
    let mut kept: Vec<u8> = Vec::with_capacity(head.len());
    for (at, line) in head.split(|b| *b == b'\r').enumerate() {
        let line = line.strip_prefix(b"\n").unwrap_or(line);
        if line.is_empty() || at == 0 {
            continue;
        }
        let name = line
            .splitn(2, |b| *b == b':')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if matches!(
            name.as_slice(),
            b"host" | b"connection" | b"x-localcases-token"
        ) {
            continue;
        }
        kept.extend_from_slice(line);
        kept.extend_from_slice(b"\r\n");
    }
    kept
}

/// The request line of a head, as text.
fn request_line(head: &[u8]) -> String {
    head.split(|b| *b == b'\r')
        .next()
        .and_then(|line| std::str::from_utf8(line).ok())
        .unwrap_or_default()
        .to_owned()
}

/// Method and path of a request line, or an error naming what was wrong.
fn target(line: &str) -> Result<(String, String), String> {
    let mut parts = line.split_whitespace();
    let (Some(method), Some(path)) = (parts.next(), parts.next()) else {
        return Err(format!("unparsable request line: {line}"));
    };
    if !path.starts_with('/') {
        return Err(format!("request path is not a path: {path}"));
    }
    Ok((method.to_owned(), path.to_owned()))
}

/// The browser's cross-origin probe: an `OPTIONS` that names where the request
/// comes from and which method it is asking about. A browser will never put a
/// custom header on it, so it cannot carry the loopback token, and the service
/// answers it in its CORS layer — above that service's own token gate. Letting
/// it through is what keeps this hop invisible to the webview; it names no route
/// and its answer is policy, not data.
fn is_preflight(method: &str, head: &[u8]) -> bool {
    method == "OPTIONS"
        && header_value(head, "origin").is_some()
        && header_value(head, "access-control-request-method").is_some()
}

/// Read one request head, plus the body it declared.
async fn read_request(client: &mut TcpStream) -> Result<(Vec<u8>, Vec<u8>), String> {
    let mut request = Vec::new();
    let past_head = loop {
        if let Some(at) = head_end(&request) {
            break at;
        }
        let mut chunk = [0u8; 8 * 1024];
        let read = client.read(&mut chunk).await.map_err(|e| e.to_string())?;
        if read == 0 {
            return Err("the client closed before its request ended".to_owned());
        }
        request.extend_from_slice(&chunk[..read]);
        if request.len() > MAX_HEAD {
            return Err(format!("request head over {MAX_HEAD} bytes"));
        }
    };
    let head = request[..past_head - 4].to_vec();
    let mut body = request[past_head..].to_vec();
    let declared: usize = header_value(&head, "content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if declared > MAX_BODY {
        return Err(format!("request body over {MAX_BODY} bytes"));
    }
    while body.len() < declared {
        let mut chunk = [0u8; 8 * 1024];
        let read = client.read(&mut chunk).await.map_err(|e| e.to_string())?;
        if read == 0 {
            return Err("the client closed before its body ended".to_owned());
        }
        body.extend_from_slice(&chunk[..read]);
    }
    Ok((head, body))
}

/// The request as the upstream should see it: same method, path (with the
/// configured prefix) and body, but this hop's `host`, `connection` and token
/// instead of the client's.
fn upstream_request(method: &str, path: &str, head: &[u8], body: &[u8], up: &Upstream) -> Vec<u8> {
    let mut out = Vec::with_capacity(head.len() + body.len() + 128);
    out.extend_from_slice(format!("{method} {}{path} HTTP/1.1\r\n", up.prefix).as_bytes());
    out.extend_from_slice(&forwarded_headers(head));
    out.extend_from_slice(format!("host: {}\r\nconnection: close\r\n", up.authority()).as_bytes());
    if let Some(token) = &up.token {
        out.extend_from_slice(format!("x-localcases-token: {token}\r\n").as_bytes());
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(body);
    out
}

/// Answer on the client's own connection instead of bothering the service.
async fn refuse(client: &mut TcpStream, status: u16, reason: &str, body: &str) {
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: text/plain; charset=utf-8\r\n\
         access-control-allow-origin: *\r\nconnection: close\r\ncontent-length: {}\r\n\r\n",
        body.len()
    );
    let _ = client.write_all(head.as_bytes()).await;
    let _ = client.write_all(body.as_bytes()).await;
    let _ = client.flush().await;
}

/// Ask the service, then hand its answer back byte for byte.
async fn exchange(
    client: &mut TcpStream,
    up: &Upstream,
    method: &str,
    path: &str,
    head: &[u8],
    body: &[u8],
) -> Result<(), String> {
    let request = upstream_request(method, path, head, body, up);
    let mut to_upstream = TcpStream::connect((up.host.as_str(), up.port))
        .await
        .map_err(|e| format!("cannot reach http://{}: {e}", up.authority()))?;
    to_upstream
        .write_all(&request)
        .await
        .map_err(|e| format!("cannot ask the upstream: {e}"))?;
    // No half-close here: an HTTP server that sees EOF on the request side
    // before it has answered treats the client as gone and drops the response,
    // which looks like an empty reply. `connection: close` above is what makes
    // the upstream end its answer instead.

    let mut buffer = vec![0u8; 64 * 1024];
    let mut answered = 0usize;
    loop {
        let read = to_upstream
            .read(&mut buffer)
            .await
            .map_err(|e| format!("upstream read: {e}"))?;
        if read == 0 {
            break;
        }
        answered += read;
        client
            .write_all(&buffer[..read])
            .await
            .map_err(|e| format!("client write: {e}"))?;
    }
    client.flush().await.map_err(|e| e.to_string())?;
    if answered == 0 {
        // Silent, because the client has already been told the connection ended.
        return Err("the upstream closed without answering".to_owned());
    }
    Ok(())
}

async fn handle(client: TcpStream, upstream: Upstream, local_token: Option<String>) {
    let mut client = client;
    let outcome = async {
        let (head, body) = read_request(&mut client).await?;
        let (method, path) = target(&request_line(&head))?;
        // The gate is answered here, before anything leaves this machine: a
        // client holding the wrong loopback token must not be able to make this
        // process open a connection to the machine that holds the corpus. A CORS
        // preflight is the one request that passes ungated, because a browser
        // will not attach a token to it — and it stops at the service's CORS
        // layer, which sits above that service's own gate.
        let given = header_value(&head, "x-localcases-token").unwrap_or_default();
        if !is_preflight(&method, &head)
            && let Some(wanted) = &local_token
            && given != *wanted
        {
            return Err("no token".to_owned());
        }
        exchange(&mut client, &upstream, &method, &path, &head, &body).await
    };
    match tokio::time::timeout(EXCHANGE_TIMEOUT, outcome).await {
        Err(_) => warn!("upstream exchange timed out"),
        Ok(Ok(())) => {}
        // An unconfigured browser tab or a port probe is normal noise: answer it
        // properly, log nothing.
        Ok(Err(e)) if e == "no token" => {
            let _ = refuse(&mut client, 401, "Unauthorized", "missing or wrong token").await;
        }
        Ok(Err(e)) => warn!("{e}"),
    }
}

/// Forward every request arriving on `addr` to `upstream` until the process
/// ends. `local_token` gates the loopback side exactly as [crate::server] gates
/// a library, so the two look identical to the front-end.
pub async fn serve(
    addr: SocketAddr,
    upstream: Upstream,
    local_token: Option<String>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!(
        "forwarding http://{} to http://{}{}",
        listener.local_addr()?,
        upstream.authority(),
        upstream.prefix
    );
    loop {
        let (client, peer) = listener.accept().await?;
        let upstream = upstream.clone();
        let local_token = local_token.clone();
        info!("{} reached the forwarder", peer.ip());
        tokio::spawn(async move {
            handle(client, upstream, local_token).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn an_origin_with_its_port_and_optional_prefix() {
        let up = Upstream::parse("http://192.168.1.100:5010", Some("t".into())).unwrap();
        assert_eq!(
            (up.host.as_str(), up.port, up.prefix.as_str()),
            ("192.168.1.100", 5010, "")
        );
        let up = Upstream::parse("http://localhost:5010/api/", None).unwrap();
        assert_eq!(
            (up.host.as_str(), up.port, up.prefix.as_str()),
            ("localhost", 5010, "/api")
        );
        assert_eq!(up.authority(), "localhost:5010");
    }

    #[test]
    fn origins_that_could_send_a_token_to_the_wrong_place_are_refused() {
        // No port: assuming 80 is how a token reaches an unrelated server.
        assert!(Upstream::parse("http://box", None).is_err());
        // TLS would need a client this module does not have; a tunnel in front
        // is the honest answer.
        assert!(Upstream::parse("https://box:5010", None).is_err());
        assert!(Upstream::parse("http://:5010", None).is_err());
        assert!(Upstream::parse("http://box:notaport", None).is_err());
        assert!(Upstream::parse("192.168.1.100:5010", None).is_err());
    }

    #[test]
    fn hop_headers_are_dropped_and_the_rest_survives() {
        let head = b"GET /api/health HTTP/1.1\r\nHost: 127.0.0.1:5010\r\nAccept: */*\r\n\
                     Accept-Encoding: gzip\r\nx-localcases-token: local\r\nConnection: keep-alive"
            .to_vec();
        let kept = String::from_utf8(forwarded_headers(&head)).unwrap();
        assert!(kept.contains("Accept: */*"));
        assert!(kept.contains("Accept-Encoding: gzip"));
        assert!(!kept.to_ascii_lowercase().contains("host:"));
        assert!(!kept.contains("x-localcases-token"));
        assert!(!kept.to_ascii_lowercase().contains("connection:"));
        assert!(
            !kept.contains("GET "),
            "the request line is rebuilt, not copied"
        );
    }

    #[test]
    fn header_lookup_is_line_anchored_and_case_insensitive() {
        let head = b"GET / HTTP/1.1\r\nContent-Length: 12\r\nx-localcases-token: abc\r\n\
                     X: content-length: 99"
            .to_vec();
        assert_eq!(header_value(&head, "content-length").as_deref(), Some("12"));
        assert_eq!(
            header_value(&head, "X-LocalCases-Token").as_deref(),
            Some("abc")
        );
        assert_eq!(header_value(&head, "accept-encoding"), None);
    }

    #[test]
    fn a_request_line_that_is_not_a_path_is_refused() {
        assert_eq!(
            target("GET /api/search?x=1 HTTP/1.1").unwrap().1,
            "/api/search?x=1"
        );
        assert!(target("GET http://elsewhere/api HTTP/1.1").is_err());
        assert!(target("nonsense").is_err());
    }

    /// A stand-in for the real service: records what arrived, then answers.
    struct Stub {
        port: u16,
        seen: Arc<AtomicUsize>,
        last_request: Arc<std::sync::Mutex<String>>,
    }

    fn stub(response: &'static str) -> Stub {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(AtomicUsize::new(0));
        let last = Arc::new(std::sync::Mutex::new(String::new()));
        let (seen_thread, last_thread) = (Arc::clone(&seen), Arc::clone(&last));
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                seen_thread.fetch_add(1, Ordering::SeqCst);
                // Read one request head and stop, the way an HTTP server does:
                // the forwarder keeps this connection open until the answer is
                // done, so waiting for EOF here would deadlock.
                use std::io::Read;
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(300)));
                let mut text = String::new();
                let mut byte = [0u8; 256];
                loop {
                    match stream.read(&mut byte) {
                        Ok(0) => break,
                        Ok(read) => text.push_str(&String::from_utf8_lossy(&byte[..read])),
                        Err(_) => break,
                    }
                    if text.contains("\r\n\r\n") {
                        break;
                    }
                }
                *last_thread.lock().unwrap() = text;
                use std::io::Write;
                let _ = stream.write_all(response.as_bytes());
            }
        });
        Stub {
            port,
            seen,
            last_request: last,
        }
    }

    /// A plain blocking socket. The tests drive it through `ask` instead: a
    /// blocking call on a current_thread runtime would starve the very task it
    /// is waiting for.
    fn ask_now(port: u16, path: &str, token: Option<&str>) -> String {
        use std::io::{Read, Write};
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        let token = token
            .map(|t| format!("x-localcases-token: {t}\r\n"))
            .unwrap_or_default();
        client
            .write_all(
                format!("GET {path} HTTP/1.1\r\nhost: 127.0.0.1:{port}\r\n{token}\r\n").as_bytes(),
            )
            .unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let mut answer = String::new();
        client.read_to_string(&mut answer).unwrap();
        answer
    }

    async fn ask(port: u16, path: &'static str, token: Option<&'static str>) -> String {
        tokio::task::spawn_blocking(move || ask_now(port, path, token))
            .await
            .unwrap()
    }

    /// The accept loop `serve` runs, minus the bind, so a test can hold a port
    /// it already knows.
    async fn forwarder(listener: TcpListener, upstream: Upstream, local_token: Option<String>) {
        loop {
            let Ok((client, _peer)) = listener.accept().await else {
                return;
            };
            let upstream = upstream.clone();
            let local_token = local_token.clone();
            tokio::spawn(async move {
                handle(client, upstream, local_token).await;
            });
        }
    }

    /// The point of the module: the loopback token is swapped for the upstream
    /// one, the prefix is applied, and the answer comes back whole.
    #[tokio::test]
    async fn the_local_token_is_swapped_for_the_upstream_one() {
        let upstream = stub(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 26\r\n\r\n\
             {\"ok\":true,\"indexed_documents\":989517}",
        );
        let target = Upstream::parse(
            &format!("http://127.0.0.1:{}/remote", upstream.port),
            Some("far".into()),
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_port = listener.local_addr().unwrap().port();
        let running = tokio::spawn(forwarder(listener, target, Some("near".into())));

        let answer = ask(
            local_port,
            "/api/search?search=%E5%90%88%E5%90%8C",
            Some("near"),
        )
        .await;
        assert!(answer.starts_with("HTTP/1.1 200"));
        assert!(answer.contains("989517"));

        let asked = upstream.last_request.lock().unwrap().clone();
        assert!(asked.starts_with("GET /remote/api/search?search=%E5%90%88%E5%90%8C HTTP/1.1"));
        assert!(asked.contains("x-localcases-token: far"));
        assert!(
            !asked.contains("near"),
            "the loopback secret must not travel"
        );
        assert!(asked.contains(&format!("host: 127.0.0.1:{}", upstream.port)));
        assert!(asked.contains("connection: close"));
        running.abort();
    }

    /// A client this forwarder does not know must not get to reach the machine
    /// that holds the corpus.
    #[tokio::test]
    async fn a_wrong_local_token_never_contacts_the_upstream() {
        let upstream = stub("HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok");
        let target = Upstream::parse(
            &format!("http://127.0.0.1:{}", upstream.port),
            Some("far".into()),
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_port = listener.local_addr().unwrap().port();
        let running = tokio::spawn(forwarder(listener, target, Some("near".into())));

        let answer = ask(local_port, "/api/health", Some("guess")).await;
        assert!(answer.starts_with("HTTP/1.1 401"));
        assert!(answer.contains("missing or wrong token"));
        assert_eq!(upstream.seen.load(Ordering::SeqCst), 0);
        running.abort();
    }

    /// One request exactly as written, so a test can be a browser: another
    /// method, no token header, the CORS probe headers.
    fn ask_raw(port: u16, request: &'static str) -> String {
        use std::io::{Read, Write};
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        client.write_all(request.as_bytes()).unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let mut answer = String::new();
        client.read_to_string(&mut answer).unwrap();
        answer
    }

    async fn ask_raw_on(port: u16, request: &'static str) -> String {
        tokio::task::spawn_blocking(move || ask_raw(port, request))
            .await
            .unwrap()
    }

    /// The desktop webview is a cross-origin client, so the first thing it sends
    /// is a preflight that cannot carry a token. Gating that is what let a window
    /// open, retry every 250 ms, and never reach the corpus: the service answers
    /// a preflight in its CORS layer, above its own gate, so this hop has to let
    /// it through and let the answer come back unchanged.
    #[tokio::test]
    async fn a_browser_preflight_reaches_the_service() {
        let upstream = stub(
            "HTTP/1.1 200 OK\r\naccess-control-allow-origin: http://tauri.localhost\r\naccess-control-allow-methods: GET\r\naccess-control-allow-headers: x-localcases-token\r\ncontent-length: 0\r\n\r\n",
        );
        let target = Upstream::parse(
            &format!("http://127.0.0.1:{}", upstream.port),
            Some("far".into()),
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_port = listener.local_addr().unwrap().port();
        let running = tokio::spawn(forwarder(listener, target, Some("near".into())));

        let answer = ask_raw_on(
            local_port,
            "OPTIONS /api/search?search=%E5%90%88%E5%90%8C HTTP/1.1\r\nhost: 127.0.0.1:5311\r\norigin: http://tauri.localhost\r\naccess-control-request-method: GET\r\naccess-control-request-headers: x-localcases-token\r\n\r\n",
        )
        .await;
        assert!(answer.starts_with("HTTP/1.1 200"), "preflight: {answer}");
        assert!(answer.contains("access-control-allow-methods: GET"));

        let asked = upstream.last_request.lock().unwrap().clone();
        assert!(asked.starts_with("OPTIONS /api/search?search=%E5%90%88%E5%90%8C HTTP/1.1"));
        assert!(asked.contains("origin: http://tauri.localhost"));
        assert!(asked.contains("access-control-request-method: GET"));
        running.abort();
    }

    /// An `OPTIONS` that is not the browser's probe is a method like any other:
    /// it still needs the loopback token before this process will talk to the
    /// machine that holds the corpus.
    #[tokio::test]
    async fn an_options_that_is_not_a_preflight_is_still_gated() {
        let upstream = stub("HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok");
        let target = Upstream::parse(
            &format!("http://127.0.0.1:{}", upstream.port),
            Some("far".into()),
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_port = listener.local_addr().unwrap().port();
        let running = tokio::spawn(forwarder(listener, target, Some("near".into())));

        let answer = ask_raw_on(
            local_port,
            "OPTIONS /api/search HTTP/1.1\r\nhost: 127.0.0.1:5311\r\n\r\n",
        )
        .await;
        assert!(answer.starts_with("HTTP/1.1 401"));
        assert_eq!(upstream.seen.load(Ordering::SeqCst), 0);
        running.abort();
    }

    /// An export is the biggest answer the API produces; it must arrive byte for
    /// byte, not truncated at some buffer boundary.
    #[tokio::test]
    async fn a_large_answer_streams_through_untouched() {
        let body = "x".repeat(2_000_000);
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/csv; charset=utf-8\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        );
        let upstream = stub(Box::leak(response.into_boxed_str()));
        let target = Upstream::parse(&format!("http://127.0.0.1:{}", upstream.port), None).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_port = listener.local_addr().unwrap().port();
        let running = tokio::spawn(forwarder(listener, target, None));

        let answer = ask(local_port, "/api/export.csv?export=true", None).await;
        let (head, sent) = answer.split_once("\r\n\r\n").expect("a head and a body");
        assert!(head.starts_with("HTTP/1.1 200"));
        assert_eq!(sent.len(), 2_000_000);
        assert!(sent.bytes().all(|b| b == b'x'));
        running.abort();
    }
}
