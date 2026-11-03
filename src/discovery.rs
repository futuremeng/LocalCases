//! Token discovery for an app installed next to LocalCases. The desktop shell
//! generates its token per run and never persists it, so without this a
//! consumer has no way to obtain it. A running service therefore publishes one
//! small file — `<state dir>/localcases.json`, mode 0600 — when it starts, and
//! removes it when it exits cleanly. A service that was killed leaves the file
//! behind, which is why it carries `pid`: a consumer must check the process is
//! alive before trusting the token in it.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::Value;

/// What a running service publishes about itself. Field names are the contract
/// consumers read; `schema` is [crate::SCHEMA], so a consumer that mismatches
/// can refuse rather than mis-parse.
#[derive(Serialize)]
pub struct Announcement<'a> {
    /// where to reach this service, e.g. `http://127.0.0.1:5311`
    pub addr: String,
    /// the token the gate expects; null when the service is open
    pub token: Option<&'a str>,
    pub pid: u32,
    pub version: &'static str,
    pub schema: u32,
    /// null in forwarder mode: this process does not hold the index
    pub indexed_documents: Option<u64>,
    /// `local` (a library is open here) or `forward` (requests go elsewhere)
    pub mode: &'a str,
}

/// Keeps the discovery file alive for as long as the service runs.
pub struct Guard {
    path: PathBuf,
    token: Option<String>,
}

/// An environment variable that is set, trimmed, and not empty.
fn env(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.to_string_lossy().trim().is_empty())
        .map(PathBuf::from)
}

/// Where the discovery file lives: `LOCALCASES_STATE_DIR` for anyone who wants
/// to name it, else the platform's state directory, plus `localcases/`.
pub fn state_dir() -> PathBuf {
    let base = env("LOCALCASES_STATE_DIR")
        .or_else(|| env("XDG_STATE_HOME"))
        .or_else(|| {
            #[cfg(windows)]
            let w = env("APPDATA");
            #[cfg(not(windows))]
            let w: Option<PathBuf> = None;
            w
        })
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .unwrap_or_default();
            home.join(".local/state")
        });
    base.join("localcases")
}

/// Publish `info` in [state_dir] and return the guard that un-publishes it.
pub fn announce(info: &Announcement) -> io::Result<Guard> {
    announce_at(&state_dir(), info)
}

/// The same into `dir` — the state directory is a parameter so tests (and an
/// operator who wants the file somewhere specific) do not need environment
/// variables. The file is written through a private temporary and renamed, so
/// a consumer either sees the whole file or the previous one, never a half.
pub fn announce_at(dir: &Path, info: &Announcement) -> io::Result<Guard> {
    fs::create_dir_all(dir)?;
    let path = dir.join("localcases.json");
    let tmp = dir.join(format!("localcases.json.tmp-{}", info.pid));
    let body = serde_json::to_vec_pretty(info)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(&body)?;
        // The token is a secret; make the file unreadable to the group and
        // the world before the rename makes it visible under its real name.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
        }
    }
    fs::rename(&tmp, &path)?;
    Ok(Guard {
        path,
        token: info.token.map(str::to_owned),
    })
}

impl Drop for Guard {
    fn drop(&mut self) {
        // Only remove the file if it is still ours: a newer run replaced it,
        // and deleting that would hide a service that is up.
        if let Ok(text) = fs::read_to_string(&self.path)
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
        {
            let ours = match &self.token {
                Some(token) => value.get("token").and_then(Value::as_str) == Some(token.as_str()),
                None => value.get("token").is_none_or(Value::is_null),
            };
            let alive =
                value.get("pid").and_then(Value::as_u64) == Some(u64::from(std::process::id()));
            if ours && alive {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(token: Option<&'static str>) -> Announcement<'static> {
        Announcement {
            addr: "http://127.0.0.1:5311".to_owned(),
            token,
            pid: std::process::id(),
            version: env!("CARGO_PKG_VERSION"),
            schema: crate::SCHEMA,
            indexed_documents: Some(279),
            mode: "local",
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "localcases-discovery-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn the_file_holds_the_contract_and_dies_with_the_guard() {
        let dir = temp_dir("basic");
        let guard = announce_at(&dir, &info(Some("s3cret"))).unwrap();
        let path = dir.join("localcases.json");
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["addr"], "http://127.0.0.1:5311");
        assert_eq!(value["token"], "s3cret");
        assert_eq!(value["pid"], u64::from(std::process::id()));
        assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["schema"], crate::SCHEMA);
        assert_eq!(value["indexed_documents"], 279);
        assert_eq!(value["mode"], "local");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        drop(guard);
        assert!(!path.exists(), "a clean exit removes the file");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_replaced_file_survives_the_guard_it_replaced() {
        let dir = temp_dir("replace");
        let first = announce_at(&dir, &info(Some("one"))).unwrap();
        let second = announce_at(&dir, &info(Some("two"))).unwrap();
        let path = dir.join("localcases.json");
        drop(first);
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["token"], "two", "the newer run stays discoverable");
        drop(second);
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_open_service_announces_a_null_token() {
        let dir = temp_dir("open");
        let guard = announce_at(&dir, &info(None)).unwrap();
        let text = fs::read_to_string(dir.join("localcases.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(value["token"].is_null());
        drop(guard);
        assert!(!dir.join("localcases.json").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
