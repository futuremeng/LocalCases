#!/usr/bin/env python3
"""Generate LICENSES.third-party.md from Cargo.lock.

Every dependency's SPDX id is read out of the packaged `Cargo.toml` inside the
crates.io cache (`~/.cargo/registry/cache/*/<name>-<version>.crate`), so the
result describes exactly what is linked into the shipped binaries — which is what
Apache-2.0 §4 (license text + NOTICE + state changes) and the MIT/BSD/ISC
"keep the copyright notice" clauses actually ask for.

Usage: tools/licenses.py [--out LICENSES.third-party.md] [--check]
  --check  exit non-zero instead of writing, after comparing with the file
"""

import argparse
import collections
import glob
import gzip
import json
import os
import re
import sys
import tarfile

LOCK = "Cargo.lock"
REGISTRY = os.path.expanduser("~/.cargo/registry")
SKIP = {"localcases", "localcases-desktop"}


def lock_packages(path=LOCK):
    out = []
    # The top of the file carries a lock-format `version = 3`, so only look at
    # what follows each [[package]] header.
    for chunk in open(path, encoding="utf-8").read().split("[[package]]")[1:]:
        name = re.search(r'^name = "([^"]+)"', chunk, re.M)
        version = re.search(r'^version = "?([^"\n]+)"?', chunk, re.M)
        if name:
            out.append((name.group(1), version.group(1) if version else "?"))
    return out


def crate_dir_names():
    dirs = []
    for base in ("cache", "src"):
        root = os.path.join(REGISTRY, base)
        if os.path.isdir(root):
            for idx in os.listdir(root):
                dirs.append(os.path.join(root, idx))
    return dirs


def index_cache_path(name):
    """Where cargo caches the sparse-index metadata for one crate."""
    n = name.lower()
    if len(n) == 1:
        sub = f"1/{n}"
    elif len(n) == 2:
        sub = f"2/{n}"
    elif len(n) == 3:
        sub = f"3/{n[0]}/{n}"
    else:
        sub = f"{n[:2]}/{n[2:4]}/{n}"
    for root in glob.glob(os.path.join(REGISTRY, "index", "*", ".cache")):
        p = os.path.join(root, sub)
        if os.path.exists(p):
            return p
    return None


def license_from_index(name, version):
    """SPDX id straight out of cargo's sparse-index cache (covers crates that
    were never unpacked here, e.g. foreign-target ones). The cache is a small
    binary frame — index version, etag, then one JSON record per version, NUL
    separated; older cargo versions gzip the same payload."""
    p = index_cache_path(name)
    if not p:
        return None
    raw = open(p, "rb").read()
    if raw.startswith(b"\x1f\x8b"):
        try:
            raw = gzip.decompress(raw)
        except OSError:
            return None
    for chunk in raw.split(b"\x00"):
        if not chunk.startswith(b"{"):
            continue
        try:
            entry = json.loads(chunk.decode("utf-8", "replace"))
        except ValueError:
            continue
        if entry.get("vers") == version:
            return entry.get("license") or "(no license field)"
    return None


def license_of(name, version):
    """SPDX id from the packaged Cargo.toml, via the .crate tarball or the
    unpacked registry source, then the sparse index."""
    for d in crate_dir_names():
        crate = os.path.join(d, f"{name}-{version}.crate")
        if os.path.exists(crate):
            with tarfile.open(crate, "r:gz") as tf:
                for member in tf.getmembers():
                    if member.name.endswith("Cargo.toml") and member.name.count("/") == 1:
                        raw = tf.extractfile(member).read().decode("utf-8", "replace")
                        m = re.search(r'^license\s*=\s*"([^"]+)"', raw, re.M)
                        return m.group(1) if m else "(no license field)"
        src = os.path.join(d.replace("/cache/", "/src/"), f"{name}-{version}", "Cargo.toml")
        if os.path.exists(src):
            m = re.search(r'^license\s*=\s*"([^"]+)"', open(src, encoding="utf-8", errors="replace").read(), re.M)
            return m.group(1) if m else "(no license field)"
    return license_from_index(name, version)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="LICENSES.third-party.md")
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    pkgs = [(n, v) for n, v in lock_packages() if n not in SKIP]
    rows, missing = [], []
    for name, version in sorted(set(pkgs)):
        lic = license_of(name, version)
        if lic is None:
            missing.append((name, version))
            lic = "(not in local cargo cache — re-check with network)"
        rows.append((name, version, lic))

    hist = collections.Counter(r[2] for r in rows)
    lines = [
        "# Third-party licenses",
        "",
        "The binaries in this repository statically link the crates below. The",
        "application code itself is Apache-2.0 (see `LICENSE` and `NOTICE`); the",
        "notices below are the attribution required by those crates' own terms",
        "(MIT/BSD/ISC keep the copyright notice, Apache-2.0 additionally asks for a",
        "copy of its license and any NOTICE file, and for stating that the sources",
        "were modified — the exact license texts ship inside the published crates",
        "and are unchanged).",
        "",
        "`desktop/` is a separate workspace with its own lock file: run this tool",
        "from that directory as well before shipping a desktop bundle.",
        "",
        "Regenerate: `python3 tools/licenses.py`.",
        "",
        "## Summary",
        "",
        "| SPDX expression | crates |",
        "| --- | --- |",
    ]
    for lic, n in sorted(hist.items(), key=lambda kv: (-kv[1], kv[0])):
        lines.append(f"| {lic} | {n} |")
    lines += ["", "## All crates", "", "| crate | version | license |", "| --- | --- | --- |"]
    for name, version, lic in rows:
        lines.append(f"| {name} | {version} | {lic} |")
    if missing:
        lines += ["", f"## Unresolved locally ({len(missing)})", "",
                  "These were not in the local cargo cache when the file was generated:", ""]
        lines += [f"- {n} {v}" for n, v in missing]
    text = "\n".join(lines) + "\n"

    if args.check:
        want = open(args.out, encoding="utf-8").read() if os.path.exists(args.out) else ""
        same = want.splitlines()[7:] == text.splitlines()[7:]
        print(f"{args.out}: {'up to date' if same else 'STALE — run tools/licenses.py'}")
        return 0 if same else 1
    open(args.out, "w", encoding="utf-8").write(text)
    print(f"wrote {args.out}: {len(rows)} crates, {len(hist)} distinct licenses, {len(missing)} unresolved")
    return 0


if __name__ == "__main__":
    sys.exit(main())
