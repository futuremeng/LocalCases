#!/usr/bin/env python3
"""Check a bundled dataset against its own manifest, and against the engine.

    python3 tools/check_dataset.py                      # data/guiding-cases
    python3 tools/check_dataset.py --dir data/gazette-cases
    python3 tools/check_dataset.py --dir local_data/gazette-cases

It answers five questions:

* do the files match the hashes and record counts the manifest claims;
* is every record complete and internally consistent — a missing section is
  reported, a 全文 whose hash moved is reported;
* does the CSV carry exactly the columns `convert` recognizes, in the order
  `convert` names them, read out of src/bin/convert.rs rather than hardcoded;
* does the CSV row for a record say the same thing as the JSON row for it;
* and, for a corpus that declares a publish gate, does the text it ships actually
  stay clear of the patterns that gate exists to keep out.

What is specific to one corpus lives in that corpus's manifest under `checks`:
which field identifies a record, what order the records are in, what a source URL
has to look like, which CSV columns have to agree with which JSON fields. The
checker is then the same code for every corpus, so adding a dataset means writing
a manifest rather than editing a checker — and a corpus that drifts fails here
instead of in silence.

Nothing here touches the network: the dataset is what was already fetched.
"""

import argparse
import csv
import hashlib
import json
import re
import sys
from pathlib import Path

csv.field_size_limit(10**9)

DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
# The patterns a publish gate keeps out of a shipped corpus, duplicated here from
# the builder on purpose: the checker has to be able to confirm the promise from
# the files alone, on a machine that never ran the builder.
PII_PATTERNS = {
    "身份证号": re.compile(r"[1-9]\d{5}(?:19|20)\d{2}(?:0[1-9]|1[0-2])"
                          r"(?:0[1-9]|[12]\d|3[01])\d{3}[\dXx]"),
    "手机号": re.compile(r"(?<!\d)1[3-9]\d{9}(?!\d)"),
    "住所地": re.compile(r"住所地[：:]\s*[\u4e00-\u9fff]{2}"),
    "住址": re.compile(r"(?:住|通信地址|联系地址)[：:]"),
    "法定代表人": re.compile(r"法定代表人[：:]"),
    "委托代理人": re.compile(r"委托代理人[：:]"),
}

failures = []


def check(ok, msg):
    if ok:
        print("  ok   %s" % msg)
    else:
        print("  FAIL %s" % msg)
        failures.append(msg)
    return ok


def stored_columns(repo):
    """The stored column names, read out of convert.rs' COLUMNS table."""
    src = (repo / "src/bin/convert.rs").read_text(encoding="utf-8")
    block = re.search(r"const COLUMNS:.*?=\s*\[(.*?)\];", src, re.S)
    if not block:
        raise SystemExit("no COLUMNS table found in src/bin/convert.rs")
    return re.findall(r'\(\s*\n?\s*"([^"]+)",', block.group(1))


def at(rec, field):
    """A checks-block field path, so `sections.相关法条` reads inside a section."""
    for part in field.split("."):
        rec = rec.get(part, "") if isinstance(rec, dict) else ""
    return rec


def main():
    ap = argparse.ArgumentParser(description="verify a bundled dataset")
    ap.add_argument("--dir", default="data/guiding-cases")
    ap.add_argument("--repo", default=".")
    args = ap.parse_args()

    # A Windows console hands stdout the active code page (cp1252 on the runners),
    # and this report is full of the Chinese section names it cannot encode -- the
    # check then died printing the result of a check that had passed.
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")

    root = Path(args.dir)
    repo = Path(args.repo)
    manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
    want = manifest.get("checks") or {}
    print("dataset %s %s (%s)%s" % (manifest["dataset"], manifest["version"],
                                    manifest["title"],
                                    "  [%s]" % manifest["scope"]
                                    if manifest.get("scope") else ""))
    if not want:
        print("  note no `checks` block in the manifest; running what is generic")

    print("files")
    for name, info in manifest["files"].items():
        path = root / name
        if not check(path.exists(), "%s exists" % name):
            continue
        raw = path.read_bytes()
        if not check(hashlib.sha256(raw).hexdigest() == info["sha256"],
                     "%s sha256 matches" % name):
            # Two opposite mistakes both land here, and telling them apart is the
            # whole diagnosis. A Windows checkout with core.autocrlf=true turns the
            # committed LF into CRLF; a generator that wrote CRLF in the first place
            # records a hash of bytes git never stores, because .gitattributes pins
            # the tree to eol=lf. The first is the machine's; the second is ours.
            lf = raw.replace(b"\r\n", b"\n")
            if hashlib.sha256(lf).hexdigest() == info["sha256"]:
                print("       note: this is the committed file with CRLF endings -- the checkout\n"
                      "       converted them (core.autocrlf=true). Set it false and check the\n"
                      "       files out again; what git holds is LF.")
            else:
                crlf = lf.replace(b"\n", b"\r\n")
                if hashlib.sha256(crlf).hexdigest() == info["sha256"]:
                    print("       note: the manifest hashed a CRLF file, but git stores LF here\n"
                      "       (.gitattributes eol=lf), so no checkout can match it. The generator\n"
                      "       must write LF; regenerate and re-record the hashes.")
        check(len(raw) == info["bytes"], "%s is %d bytes" % (name, info["bytes"]))
        # What makes the hash portable at all: the bytes a checkout yields are the
        # bytes the generator hashed only if neither side rewrote a line ending.
        if name.endswith((".csv", ".jsonl")):
            check(b"\r\n" not in raw, "%s is LF-only, as .gitattributes eol=lf stores it" % name)

    cases = [json.loads(line) for line in
             open(root / "cases.jsonl", encoding="utf-8")]
    counts = manifest["counts"]
    ident = want.get("identity_field", "case_no")
    key = lambda r: r.get(ident, r.get("case_no"))

    print("records")
    check(len(cases) == counts["cases"], "cases.jsonl holds %d records" % counts["cases"])
    for name, count in want.get("extra_files", {}).items():
        rows = [json.loads(line) for line in open(root / name, encoding="utf-8")]
        check(len(rows) == manifest["counts"][count],
              "%s holds %d records" % (name, manifest["counts"][count]))
    ids = [key(c) for c in cases]
    check(len(set(map(str, ids))) == len(ids), "%s values are unique" % ident)
    order = want.get("sort_key")
    if order:
        seq = [tuple(c.get(f) for f in order) for c in cases]
        check(seq == sorted(seq), "records are in %s order" % "+".join(order))
    number_field = want.get("number_field")
    if number_field:
        numbers = [c[number_field] for c in cases]
        check(numbers == sorted(numbers), "case numbers are in ascending order")
        gaps = sorted(set(range(min(numbers), max(numbers) + 1)) - set(numbers))
        check(gaps == counts.get("case_numbers_missing"),
              "number gaps match the manifest (%s)" % (gaps or "none"))
        check(max(numbers) == counts.get("case_numbers_max"),
              "highest case number is %d" % max(numbers))

    print("content")
    text_field = want.get("text_field", "full_text")
    bad_hash = [str(key(c)) for c in cases
                if hashlib.sha256(c[text_field].encode("utf-8")).hexdigest()
                != c.get("full_text_sha256")]
    check(not bad_hash, "every 全文 hash recomputes (%s)" % (bad_hash[:5] or "all"))
    url_field = want.get("url_field", "source_url")
    pattern = want.get("source_url_pattern")
    if pattern:
        rx = re.compile(pattern)
        bad_url = [str(key(c)) for c in cases if not rx.match(c[url_field])]
        check(not bad_url, "every record names a documented source page (%s)"
              % (bad_url[:5] or "all"))
    date_field = want.get("date_field")
    if date_field:
        bad_date = [str(key(c)) for c in cases if not DATE_RE.match(c[date_field])]
        check(not bad_date, "every record has an ISO %s (%s)"
              % (date_field, bad_date[:5] or "all"))
    sections_field = want.get("sections_field")
    if sections_field:
        seen = {}
        for c in cases:
            for name in c["section_order"]:
                seen[name] = seen.get(name, 0) + 1
        check(seen == counts["sections"], "section counts match the manifest")
        absent = [n for n in want.get("required_sections", [])
                  if counts["sections"].get(n, 0) < counts["cases"]]
        print("  note sections not on every record: %s" % (absent or "none"))
        suffix = want.get("any_section_suffix")
        if suffix:
            none = [str(key(c)) for c in cases
                    if not any(n.endswith(suffix) and t.strip()
                               for n, t in c[sections_field].items())]
            check(not none, "every record has a non-empty *%s section (%s)"
                  % (suffix, none[:3] or "all"))

    print("csv")
    with open(root / "cases.csv", encoding="utf-8") as fh:
        reader = csv.DictReader(fh)
        header = reader.fieldnames
        rows = list(reader)
    columns = stored_columns(repo)
    check(header == columns, "header is exactly convert.rs' %d stored columns"
          % len(columns))
    check(len(rows) == len(cases), "%d data rows" % len(rows))

    print("csv vs jsonl")
    mapping = want.get("csv_vs_jsonl", {})
    mismatch = []
    for row, rec in zip(rows, cases):
        problems = [col for col, field in mapping.items()
                    if row[col] != at(rec, field)]
        if hashlib.sha256(row["全文"].encode("utf-8")).hexdigest() != rec["full_text_sha256"]:
            problems.append("全文")
        if any(mark in row["全文"] for mark in ("<p", "</", "<div", "c_header")):
            problems.append("全文 carries markup")
        if problems:
            mismatch.append((str(key(rec)), problems))
    check(not mismatch, "every CSV row says what its JSON row says (%s)"
          % (mismatch[:3] or "all"))

    # The publish gate, checked from the outside. A corpus that declares one
    # promises that the text it ships holds none of these patterns; this reads
    # the shipped CSV and confirms it, so the promise is a property of the
    # repository rather than of the build that produced it.
    gate = want.get("publish_gate")
    if gate:
        # A corpus that declares a gate promises that the text it ships holds
        # none of these patterns; this reads the shipped CSV and confirms it, so
        # the promise is a property of the repository rather than of the build
        # that produced it. When the manifest spells the patterns out, they are
        # run as written; `true` means the built-in set, which is what the
        # Gazette corpus was gated with.
        print("publish gate")
        if gate is True:
            identifying = {k: v.pattern for k, v in PII_PATTERNS.items()}
            roles = {}
        else:
            identifying = gate.get("identifying", {})
            roles = gate.get("roles", {})
        leaked = {}
        for row in rows:
            for label, pat in identifying.items():
                if re.search(pat, row["全文"]):
                    leaked.setdefault(label, []).append(row["案件名称"][:24])
        for label, names in sorted(leaked.items()):
            check(False, "shipped 全文 holds no %s (%d row(s), e.g. %s)"
                  % (label, len(names), names[0]))
        if not leaked:
            check(True, "shipped 全文 holds none of %s" % " / ".join(identifying))
        for label, pair in sorted(roles.items()):
            word = sum(len(re.findall(pair["word"], r["全文"])) for r in rows)
            valued = sum(len(re.findall(pair["valued"], r["全文"])) for r in rows)
            print("  note %-12s word %d, with a value %d" % (label, word, valued))
        held = manifest["counts"].get("body_chars_total", 0)
        if held:
            print("  note %d characters of judgment body are held back by design" % held)

    # The other side of the same promise: a corpus held back on purpose should
    # say how much it holds back, so the reason it is not in the repository is a
    # measured fact in its manifest rather than a comment someone can forget.
    if want.get("pii_census"):
        held_back = manifest.get("scope") == "local"
        print("personal-data census (%s)"
              % ("this corpus is held back, not shipped" if held_back
                 else "this corpus ships; what the census found stays in it"))
        census, rows_with = {}, 0
        for row in rows:
            found = False
            for label, rx in PII_PATTERNS.items():
                n = len(rx.findall(row["全文"]))
                if n:
                    census[label] = census.get(label, 0) + n
                    found = True
            rows_with += found
        for label in PII_PATTERNS:
            if label in census:
                print("  note %6d %s" % (census[label], label))
        if held_back:
            print("  note %6d of %d rows carry at least one, which is why this lives"
                  " under local_data/\n       and not in the repository"
                  % (rows_with, len(rows)))
        else:
            print("  note %6d of %d rows carry at least one" % (rows_with, len(rows)))

    print("summary")
    print("  %d records, %s characters of 全文" % (counts["cases"],
                                                  counts["full_text_chars_total"]))
    if failures:
        print("%d check(s) failed" % len(failures))
        return 1
    print("all checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
