//! Beyond `tauri_build`, this script holds one assertion: the version a bundle
//! announces is the version the service reports.
//!
//! `/api/health` answers with the engine crate's `CARGO_PKG_VERSION`, and SPEC §4
//! lets a consumer refuse a contract it does not speak — so the number in the
//! window's About box and in the package filename cannot be allowed to drift away
//! from it. There is no CI in this repository, so the check runs at build time:
//! a mismatched manifest stops the build instead of shipping a wrong number.

use std::path::Path;

/// The value of the first `key` on a line, taken from between the next pair of
/// quotes. Deliberately dumb: these are two small hand-written files, not inputs.
fn quoted(lines: &[&str], key: &str) -> Option<String> {
    for line in lines {
        let line = line.trim_start();
        if !line.starts_with(key) {
            continue;
        }
        let rest = &line[key.len()..];
        let open = rest.find('"')?;
        let value = &rest[open + 1..];
        let close = value.find('"')?;
        return Some(value[..close].to_owned());
    }
    None
}

/// The top-level `"version"` of a Tauri config; it is the only one this file has.
fn conf_version(path: &Path) -> String {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    quoted(&text.lines().collect::<Vec<_>>(), "\"version\"")
        .unwrap_or_else(|| panic!("no top-level \"version\" in {}", path.display()))
}

/// The `version` of the `[package]` section of a Cargo manifest — and only that
/// one, so the `rust-version` line below it cannot be mistaken for it.
fn cargo_version(path: &Path) -> String {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package && line.starts_with("version") {
            if let Some(version) = quoted(&[line], "version") {
                return version;
            }
        }
    }
    panic!("no [package] version in {}", path.display())
}

fn main() {
    let desktop = Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("the shell manifest directory");
    let engine = desktop
        .parent()
        .map(|root| root.join("Cargo.toml"))
        .expect("the shell lives inside this repository");

    for changed in ["tauri.conf.json", "Cargo.toml", engine.to_str().unwrap()] {
        println!("cargo:rerun-if-changed={changed}");
    }

    let conf = conf_version(&desktop.join("tauri.conf.json"));
    let shell = cargo_version(&desktop.join("Cargo.toml"));
    let engine = cargo_version(&engine);
    if conf != shell || shell != engine {
        panic!(
            "version drift: tauri.conf.json {conf}, desktop/Cargo.toml {shell}, \
             the engine's Cargo.toml {engine}. They must be equal: /api/health reports \
             the engine's version, and SPEC §4 lets a consumer refuse a contract it does not speak."
        );
    }

    tauri_build::build()
}
