#!/usr/bin/env python3
"""Check a bundled dataset against its own manifest, and against the engine.

    python3 tools/check_dataset.py                     # data/guiding-cases
    python3 tools/check_dataset.py --dir data/other

It answers four questions:

* do the files match the hashes and record counts the manifest claims;
* is every record complete and internally consistent (a missing section is
  reported, a 全文 whose hash moved is reported);
* does the CSV carry exactly the columns `convert` recognizes, in the order
  `convert` names them — read out of src/bin/convert.rs, not hardcoded here;
* does the CSV row for a case say the same thing as the JSON row for it.

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

REQUIRED_SECTIONS = ["关键词", "基本案情", "裁判结果", "裁判理由", "相关法条"]
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
URL_RE = re.compile(r"^https://www\.court\.gov\.cn/shenpan/xiangqing/\d+\.html$")

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


def main():
    ap = argparse.ArgumentParser(description="verify a bundled dataset")
    ap.add_argument("--dir", default="data/guiding-cases")
    ap.add_argument("--repo", default=".")
    args = ap.parse_args()

    root = Path(args.dir)
    repo = Path(args.repo)
    manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
    print("dataset %s %s (%s)" % (manifest["dataset"], manifest["version"],
                                  manifest["title"]))

    print("files")
    for name, info in manifest["files"].items():
        path = root / name
        if not check(path.exists(), "%s exists" % name):
            continue
        raw = path.read_bytes()
        check(hashlib.sha256(raw).hexdigest() == info["sha256"], "%s sha256 matches" % name)
        check(len(raw) == info["bytes"], "%s is %d bytes" % (name, info["bytes"]))

    cases = [json.loads(line) for line in
             open(root / "cases.jsonl", encoding="utf-8")]
    notices = [json.loads(line) for line in
               open(root / "notices.jsonl", encoding="utf-8")]
    counts = manifest["counts"]
    print("records")
    check(len(cases) == counts["cases"], "cases.jsonl holds %d records" % counts["cases"])
    check(len(notices) == counts["notices"], "notices.jsonl holds %d records" % counts["notices"])

    numbers = [c["case_no"] for c in cases]
    check(numbers == sorted(numbers), "case numbers are in ascending order")
    check(len(set(numbers)) == len(numbers), "case numbers are unique")
    gaps = sorted(set(range(min(numbers), max(numbers) + 1)) - set(numbers))
    check(gaps == counts["case_numbers_missing"],
          "number gaps match the manifest (%s)" % (gaps or "none"))
    check(max(numbers) == counts["case_numbers_max"],
          "highest case number is %d" % counts["case_numbers_max"])

    print("content")
    no_points = [c["case_no"] for c in cases
                 if not any(name.endswith("要点") and text.strip()
                            for name, text in c["sections"].items())]
    check(not no_points,
          "every case has a non-empty 要点 section (裁判要点 / 执行实施要点)")
    bad_hash = [c["case_no"] for c in cases
                if hashlib.sha256(c["full_text"].encode("utf-8")).hexdigest() != c["full_text_sha256"]]
    check(not bad_hash, "every 全文 hash recomputes (%s)" % (bad_hash[:5] or "all"))
    bad_url = [c["case_no"] for c in cases if not URL_RE.match(c["source_url"])]
    check(not bad_url, "every record names a court.gov.cn source page (%s)" % (bad_url[:5] or "all"))
    bad_date = [c["case_no"] for c in cases if not DATE_RE.match(c["publish_date"])]
    check(not bad_date, "every record has an ISO 公开日期 (%s)" % (bad_date[:5] or "all"))
    seen_sections = {}
    for c in cases:
        for name in c["section_order"]:
            seen_sections[name] = seen_sections.get(name, 0) + 1
    check(seen_sections == counts["sections"], "section counts match the manifest")
    absent = [name for name in REQUIRED_SECTIONS
              if counts["sections"].get(name, 0) < counts["cases"]]
    print("  note sections not on every page: %s" % (absent or "none"))

    print("notices")
    bad_notice = []
    for n in notices:
        title = n.get("title", "")
        if not title or any(x in title for x in ("&nbsp;", "&amp;", "<", ">")):
            bad_notice.append(title[:30])
        for lo, hi in n.get("case_ranges", []):
            if lo > hi or hi - lo > 200:
                bad_notice.append(("range", n.get("batch"), lo, hi))
    check(not bad_notice, "every notice has a clean title and sane ranges (%s)"
          % (bad_notice[:5] or "all"))
    covered = set()
    for n in notices:
        for lo, hi in n.get("case_ranges", []):
            covered.update(range(lo, hi + 1))
    attributed = {c["case_no"] for c in cases if "batch" in c}
    check(attributed == covered & {c["case_no"] for c in cases},
          "batch attribution is exactly what the notices' ranges cover")

    print("csv")
    with open(root / "cases.csv", encoding="utf-8") as fh:
        reader = csv.DictReader(fh)
        header = reader.fieldnames
        rows = list(reader)
    want = stored_columns(repo)
    check(header == want, "header is exactly convert.rs' %d stored columns" % len(want))
    check(len(rows) == len(cases), "%d data rows" % len(cases))

    print("csv vs jsonl")
    mismatch = []
    for row, rec in zip(rows, cases):
        problems = []
        if row["原始链接"] != rec["source_url"]:
            problems.append("原始链接")
        if row["案件名称"] != rec["title_full"]:
            problems.append("案件名称")
        if row["公开日期"] != rec["publish_date"]:
            problems.append("公开日期")
        if row["案件类型"] != rec["case_group"]:
            problems.append("案件类型")
        if row["法律依据"] != rec["sections"].get("相关法条", ""):
            problems.append("法律依据")
        if hashlib.sha256(row["全文"].encode("utf-8")).hexdigest() != rec["full_text_sha256"]:
            problems.append("全文")
        if any(marker in row["全文"] for marker in ("<p", "</", "<div", "c_header")):
            problems.append("全文 carries markup")
        if problems:
            mismatch.append((rec["case_no"], problems))
    check(not mismatch, "every CSV row says what its JSON row says (%s)"
          % (mismatch[:3] or "all"))

    print("summary")
    print("  cases %d, numbers %d-%d, notices %d, attributed to a batch %d"
          % (counts["cases"], counts["case_numbers_min"], counts["case_numbers_max"],
             counts["notices"], counts["with_batch_attribution"]))
    print("  全文 %d characters total" % counts["full_text_chars_total"])
    if failures:
        print("%d check(s) failed" % len(failures))
        return 1
    print("all checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
