#!/usr/bin/env python3
"""Build the `gazette-cases` datasets from the Supreme People's Court Gazette.

The 最高人民法院公报 (gongbao.court.gov.cn, 裁判文书选登) publishes the SPC's own
selected judgments — one document per 公报 issue, 445 of them across 2003-2026.
Compared with the guiding cases already bundled, a gazette record is a *complete*
judgment rather than an edited summary, and it carries the three columns a
guiding-case page never publishes: 法院, 文书类型 and a 案号 with a real 案字 code.

The corpus splits in two, and the split is the whole point of this script:

    data/gazette-cases        shipped. metadata + 裁判摘要. no party list, no
                              住所地, no natural-person names beyond what the
                              Gazette itself prints in the case title.
    local_data/gazette-cases  NOT shipped (gitignored). the same 445 records with
                              全文 carrying the judgment body, which is where the
                              personal data lives: over this corpus the bodies hold
                              14 身份证号, 24 手机号, 965 住所地 and 985 法定代表人
                              across 425 of the 445 documents.

So the public corpus is a corpus you can filter and search; the local one is the
same corpus with its text, for testing on your own machine. The public build
refuses to write if the body's personal-data patterns appear in it — see
`assert_publishable`.

    python3 tools/build_gazette_cases.py                    # both corpora
    python3 tools/build_gazette_cases.py --no-local         # public only
    python3 tools/build_gazette_cases.py --upstream /path/to/chinese-law-corpus

The upstream is a CC0 compilation of documents that are themselves outside
copyright (著作权法 第五条); it is cloned into tools/state/ (gitignored) so the
build is reproducible and every record keeps the gongbao.court.gov.cn URL it
came from. stdlib only, like the other tools here.
"""

import argparse
import csv
import hashlib
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

UPSTREAM_URL = "https://github.com/lttxzmj/chinese-law-corpus"
UPSTREAM_DIR = "gazette-cases-upstream"
SOURCE_SITE = "http://gongbao.court.gov.cn"
SOURCE_LISTING = SOURCE_SITE + "/ArticleList.html?serial_no=cpwsxd"
SOURCE_NAME = "最高人民法院公报"

# 案件类型编码, the same table extract.rs documents for the main corpus.
CASE_TYPE_CODES = {"民事": "1", "执行": "2", "行政": "3", "刑事": "4",
                   "管辖": "5", "非诉保全审查": "8", "国家赔偿": ""}

CSV_COLUMNS = ["原始链接", "案号", "案件名称", "法院", "所属地区", "案件类型",
               "案件类型编码", "来源", "审理程序", "裁判日期", "公开日期",
               "当事人", "案由", "法律依据", "全文"]

# 案字, the case-type token of a Chinese case number, as standardised by
# 法〔2015〕295号《最高人民法院关于人民法院案件案号的若干规定》. Longest first so
# 知民 is not read as 民 and 委赔 is not read as 赔.
CASE_TYPE_BY_CODE = [
    ("知民", "民事"), ("知行", "行政"), ("知产", "民事"), ("知其他", "民事"),
    ("法委赔", "国家赔偿"), ("委赔", "国家赔偿"),
    ("法引", "刑事"), ("刑引", "刑事"),
    ("民", "民事"), ("商", "民事"), ("经", "民事"), ("破", "民事"),
    ("刑", "刑事"), ("行", "行政"), ("执", "执行"), ("法保", "非诉保全审查"),
]
# The instance token of the same 案字: 终 is an appeal, 提 is a call-up for
# retrial, 申 and 监 are the application-review docket, 核 in a criminal number
# is a death-penalty review. Same longest-first rule.
PROCEDURE_KIND_BY_CODE = [
    ("委赔", "国家赔偿"), ("申", "再审审查与审判监督"), ("监", "再审审查与审判监督"),
    ("终", "二审"), ("初", "一审"), ("再", "再审"), ("提", "再审"), ("抗", "再审"),
    ("复", "复议"), ("核", "死刑复核"), ("赔", "国家赔偿"), ("执", "执行"),
    ("引", "引渡"), ("保", "非诉保全审查"),
]

# What may not appear in a corpus this script publishes. Each pattern is the
# party-block form, not the bare word: 住所地 as a legal term appears in the
# reasoning of holdings on jurisdiction, and that is not personal data — the
# block that introduces a party's address is. assert_publishable also counts the
# same patterns over the body text, so a gate that had stopped matching anything
# would be reported rather than quietly passing.
PII_PATTERNS = {
    "身份证号": re.compile(r"[1-9]\d{5}(?:19|20)\d{2}(?:0[1-9]|1[0-2])"
                          r"(?:0[1-9]|[12]\d|3[01])\d{3}[\dXx]"),
    "手机号": re.compile(r"(?<!\d)1[3-9]\d{9}(?!\d)"),
    "住所地": re.compile(r"住所地[：:]\s*[\u4e00-\u9fff]{2}"),
    "住址": re.compile(r"(?:住|通信地址|联系地址)[：:]"),
    "法定代表人": re.compile(r"法定代表人[：:]"),
    "委托代理人": re.compile(r"委托代理人[：:]"),
}

# （2002）民四终字第27号 and （2019）最高法民终123号: the year sits inside the
# brackets and the 案字 follows them, so the token is what comes after. A single
# document can carry several numbers at once (732、733、734号) or a sub-number
# (46-1号), and the 案字 is the same for all of them.
CASE_NUMBER_RE = re.compile(
    r"[（(](\d{4})[）)]\s*([^\d（）()]*?)第?(\d+(?:[、,，-]\s*\d+)*)号")


def log(msg):
    print(msg, flush=True)


def now_iso():
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def sha256_file(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def pick(table, token):
    """Longest 案字 prefix wins, so 知民终 never reads as 民终."""
    for code, value in table:
        if code in token:
            return value
    return ""


def derive(case_number):
    """(year, 案字, 案件类型, 审理程序) out of a 案号, empty for a 案字 this table
    does not know. An unknown code stays empty rather than being guessed at:
    案件类型 and 审理程序 are columns a user filters on."""
    m = CASE_NUMBER_RE.search(case_number or "")
    if not m:
        return "", "", "", ""
    year, token = m.group(1), m.group(2)
    group = pick(CASE_TYPE_BY_CODE, token)
    kind = pick(PROCEDURE_KIND_BY_CODE, token)
    if not group:
        return year, token, "", ""
    # A 法委赔字 number carries the group inside the kind; emitting
    # 国家赔偿国家赔偿 would be wrong and split_procedure would not fix it.
    return year, token, group, group if group == kind else group + kind


def full_text(rec, body=None):
    """Section-labelled plain text, the shape guiding-cases 全文 already has: the
    value of every published field under its own label, nothing reworded."""
    parts = [rec["title"]]
    for label, value in (("来源", "%s%s年第%s期 · %s" % (
            SOURCE_NAME, rec["issue_year"], rec["issue_no"], rec["category"])),
            ("审理法院", rec["court"]),
            ("文书类型", rec["doc_type"]),
            ("案号", rec["case_number"])):
        if value:
            parts.append("%s\n%s" % (label, value))
    if rec["summary"]:
        parts.append("裁判摘要\n" + "\n".join(rec["summary"]))
    if body:
        parts.append("裁判文书记载\n" + body)
    return "\n\n".join(parts) + "\n"


def assert_publishable(records, bodies):
    """Refuse to publish a corpus in which the body's personal data survived.

    Two checks, because they fail in two different ways:

    * structurally — no record may carry the judgment text under any key. The
      first version of this script put the body on the record as `_body`, and the
      JSONL then written held every 身份证号 in the corpus while the content check
      below passed, because the check only looked at 全文. A field that exists on
      a record is a field that lands on disk, so that is checked first.
    * by content — over 全文, which by construction holds every piece of prose
      this corpus publishes: the title, each labelled field and the 裁判摘要. The
      check deliberately does not scan the whole serialized record: a sha256 is
      hex, and `1[3-9]` followed by nine more digits is a phone number to a
      regex and nothing at all to a reader.

    It also reports how many of the same patterns the withheld bodies do hold — a
    gate that matched nothing anywhere would be a gate that had broken, and that
    is the failure mode worth naming here."""
    for rec in records:
        for key in sorted(set(rec) & {"body", "_body", "judgment_body"}):
            raise SystemExit("record %s carries %r: the judgment text must never "
                             "reach the published corpus" % (rec["gazette_id"], key))
    hits, in_body = {}, {}
    for rec in records:
        for name, rx in PII_PATTERNS.items():
            if rx.search(rec["full_text"]):
                hits.setdefault(name, []).append(rec["gazette_id"])
    for body in bodies.values():
        for name, rx in PII_PATTERNS.items():
            n = len(rx.findall(body))
            if n:
                in_body[name] = in_body.get(name, 0) + n
    if hits:
        for name, ids in sorted(hits.items()):
            log("  BLOCKED %s appears in %d published record(s), e.g. %s"
                % (name, len(ids), ids[0]))
        raise SystemExit("public corpus would carry personal data; not writing it")
    for name in PII_PATTERNS:
        log("  ok   no %s published; %d held back in the bodies"
            % (name, in_body.get(name, 0)))
    if not in_body:
        raise SystemExit("no pattern matched anywhere: the gate has stopped "
                         "measuring anything, so its verdict means nothing")


def load_upstream(root):
    """The upstream case JSONs plus its own manifest, which is where the fetch
    date and the per-year counts come from."""
    files = sorted(p for p in (root / "gazette-cases").glob("*.json")
                   if p.name != "manifest.json")
    if not files:
        raise SystemExit("no gazette-cases/*.json under %s" % root)
    manifest_path = root / "gazette-cases" / "manifest.json"
    upstream = json.loads(manifest_path.read_text(encoding="utf-8")) \
        if manifest_path.exists() else {}
    return files, upstream


def build_record(path):
    raw = json.loads(path.read_text(encoding="utf-8"))
    year, token, group, procedure = derive(raw["caseNumber"])
    rec = {
        "gazette_id": raw["id"],
        "issue_year": raw["issueYear"],
        "issue_no": raw["issueNo"],
        "title": raw["title"],
        "case_number": raw["caseNumber"],
        "case_number_code": token,
        "case_number_year": year,
        "court": raw["court"],
        "doc_type": raw["docType"],
        "category": raw["category"],
        "case_group": group,
        "procedure": procedure,
        "summary": raw["summary"],
        "source_url": raw["sourceUrl"],
        "upstream_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }
    rec["body_chars"] = sum(len(x) for x in raw.get("body") or [])
    return rec, raw


# The checker's dataset-specific expectations live here rather than in
# check_dataset.py, so adding a corpus does not mean editing the checker, and so
# a dataset that stops matching the importer fails loudly.
CHECKS = {
    "identity_field": "gazette_id",
    "sort_key": ["issue_year", "issue_no", "gazette_id"],
    "source_url_pattern": r"^https?://gongbao\.court\.gov\.cn/Details/[0-9a-f]+\.html$",
    "date_field": None,
    "sections_field": "sections",
    "required_sections": ["裁判摘要"],
    "text_field": "full_text",
    "publish_gate": True,
    "csv_vs_jsonl": {
        "原始链接": "source_url",
        "案件名称": "title",
        "案号": "case_number",
        "法院": "court",
        "案件类型": "case_group",
        "审理程序": "procedure",
    },
}


def csv_row(rec):
    return {
        "原始链接": rec["source_url"],
        "案号": rec["case_number"],
        "案件名称": rec["title"],
        # The Gazette prints the adjudicating court on the document itself, so
        # unlike a guiding case this one has a 法院 to put in the column.
        "法院": rec["court"],
        "所属地区": "",
        "案件类型": rec["case_group"],
        "案件类型编码": CASE_TYPE_CODES.get(rec["case_group"], ""),
        "来源": SOURCE_NAME,
        "审理程序": rec["procedure"],
        # The Gazette publishes an issue, not a judgment date; the year a case
        # was filed is in 案号, which is what the engine reads for year facets.
        "裁判日期": "",
        "公开日期": "",
        # Deliberately empty in the published corpus: the party list, with its
        # natural-person names and addresses, is in the body, and the body stays
        # local. Nothing is inferred into a column a user filters on.
        "当事人": "",
        "案由": "",
        "法律依据": "",
        "全文": rec["full_text"],
    }


def emit(out, records, manifest):
    out.mkdir(parents=True, exist_ok=True)
    jsonl, csv_path = out / "cases.jsonl", out / "cases.csv"
    with open(jsonl, "w", encoding="utf-8") as fh:
        for rec in records:
            fh.write(json.dumps(rec, ensure_ascii=False, sort_keys=True) + "\n")
    with open(csv_path, "w", encoding="utf-8", newline="") as fh:
        # LF, not the csv module's default CRLF: .gitattributes pins this tree to
        # eol=lf, so git stores the blob with LF whatever the writer emits. The
        # manifest hashes the file on disk, so a CRLF file here records a hash no
        # checkout can reproduce, and the corpus fails its own hash check on
        # every machine but the one that built it.
        writer = csv.DictWriter(fh, fieldnames=CSV_COLUMNS, quoting=csv.QUOTE_MINIMAL,
                                lineterminator="\n")
        writer.writeheader()
        for rec in records:
            writer.writerow(csv_row(rec))
    manifest["files"] = {
        p.name: {"sha256": sha256_file(p), "bytes": p.stat().st_size,
                 "records": len(records)}
        for p in (jsonl, csv_path)
    }
    (out / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    log("wrote %s: %d records, %s (%d kB)"
        % (out, len(records), ", ".join(sorted(manifest["files"])),
           sum(v["bytes"] for v in manifest["files"].values()) // 1000))


def corpus_manifest(args, records, upstream, scope, extra_notes):
    years = {}
    for rec in records:
        years[str(rec["issue_year"])] = years.get(str(rec["issue_year"]), 0) + 1
    groups, docs, courts, codes = {}, {}, {}, {}
    for rec in records:
        for table, key in ((groups, rec["case_group"] or "(未识别)"),
                           (docs, rec["doc_type"]), (courts, rec["court"]),
                           (codes, rec["case_number_code"] or "(未识别)")):
            table[key] = table.get(key, 0) + 1
    sections = {}
    for rec in records:
        for name in rec["section_order"]:
            sections[name] = sections.get(name, 0) + 1
    checks = dict(CHECKS, publish_gate=scope == "public", pii_census=scope != "public")
    return {
        "dataset": "gazette-cases",
        "title": "最高人民法院公报·裁判文书选登（%d年第%d期 至 %d年第%d期）" % (
            min(r["issue_year"] for r in records), min(r["issue_no"] for r in records),
            max(r["issue_year"] for r in records), max(r["issue_no"] for r in records)),
        "version": args.version,
        "generated_at": now_iso(),
        "generator": "tools/build_gazette_cases.py",
        "scope": scope,
        "source": {
            "publisher": SOURCE_NAME,
            "site": SOURCE_SITE,
            "listing_url": SOURCE_LISTING,
            "compilation": UPSTREAM_URL,
            "compilation_license": "CC0 1.0",
            "upstream_generated_at": upstream.get("generatedAt", ""),
            "upstream_generated_at_note": "when the upstream last synced from gongbao.court.gov.cn",
        },
        "counts": {
            "cases": len(records),
            "issues": len({(r["issue_year"], r["issue_no"]) for r in records}),
            "issue_years": len(years),
            "by_issue_year": years,
            "case_groups": groups,
            "doc_types": docs,
            "courts": courts,
            "case_number_codes": codes,
            "sections": sections,
            "with_summary": sum(1 for r in records if r["summary"]),
            "summary_lines": sum(len(r["summary"]) for r in records),
            "full_text_chars_total": sum(r["full_text_chars"] for r in records),
            "body_chars_total": sum(r["body_chars"] for r in records),
        },
        "files": {},
        "fields": FIELD_DOC,
        "checks": checks,
        "notes": extra_notes,
        "case_json_fields": [
            "gazette_id", "issue_year", "issue_no", "title", "case_number",
            "case_number_code", "case_number_year", "court", "doc_type", "category",
            "case_group", "procedure", "summary", "sections", "section_order",
            "laws_mentioned",
            "full_text", "full_text_sha256", "full_text_chars", "body_chars",
            "source_url", "upstream_sha256",
        ],
        "redistribution": {
            "basis": "judicial documents published in the 最高人民法院公报, which are outside "
                     "copyright under 著作权法 第五条; the upstream compilation that structured "
                     "them is CC0 1.0. The published corpus keeps only the Gazette's own "
                     "metadata and 裁判摘要, so it names no address, no identity document "
                     "number and no natural person the Gazette did not itself name in a title.",
            "cite": "cite the 公报 year and issue and the source_url stored with every record",
            "checked_at": now_iso(),
        },
    }


FIELD_DOC = {
    "原始链接": {"origin": "verbatim", "note": "the gongbao.court.gov.cn 裁判文书选登 page the record was read from"},
    "案号": {"origin": "verbatim", "note": "the 案号 printed on the document; every one of the 445 carries it"},
    "案件名称": {"origin": "verbatim", "note": "the title the Gazette gives the case; 90 of the 445 name a party in it exactly as published, the rest name no party"},
    "法院": {"origin": "verbatim", "note": "the adjudicating court printed on the document: 最高人民法院, or 最高人民法院赔偿委员会 for the state-compensation decisions"},
    "所属地区": {"origin": "empty", "note": "every document here was decided by the Supreme People's Court itself, so a region column would hold one value for all 445"},
    "案件类型": {"origin": "derived", "note": "the 案字 of 案号 under 法〔2015〕295号 (民/商/经/知民 → 民事, 刑/刑引 → 刑事, 行/知行 → 行政, 执 → 执行, 委赔 → 国家赔偿); empty for a 案字 the table does not name"},
    "案件类型编码": {"origin": "derived", "note": "the code extract.rs documents for that 案件类型; 国家赔偿 has no code and stays empty"},
    "来源": {"origin": "fixed", "note": "最高人民法院公报"},
    "审理程序": {"origin": "derived", "note": "案件类型 plus the instance token of the same 案字 (终 → 二审, 初 → 一审, 再/提/抗 → 再审, 申/监 → 再审审查与审判监督, 复 → 复议, 核 → 死刑复核)"},
    "裁判日期": {"origin": "empty", "note": "the Gazette dates an issue, not a judgment; the year a case was filed is in 案号, which is what the engine reads for year facets"},
    "公开日期": {"origin": "empty", "note": "same reason; 公报年份 and 期号 are in the JSON record instead"},
    "当事人": {"origin": "empty", "note": "the party list lives in the judgment body, and the body is not published with this corpus; see the notes"},
    "案由": {"origin": "empty", "note": "the Gazette prints no 案由 label; cutting one out of a case name would invent a field a user filters on"},
    "法律依据": {"origin": "empty", "note": "a 裁判文书选登 document cites statutes inside its reasoning, not in a 相关法条 section; the 《…》 spans a 裁判摘要 quotes are recorded as laws_mentioned rather than cut into a column a user filters on, because they are not all statutes — a contract is quoted the same way"},
    "全文": {"origin": "verbatim", "note": "title plus each published field under its own label, and the 裁判摘要; plain text, markup removed, nothing reworded"},
}


def resolve_upstream():
    ap = argparse.ArgumentParser(description="build the gazette-cases corpora")
    ap.add_argument("--out", default="data/gazette-cases",
                    help="where the publishable corpus goes (this is what ships)")
    ap.add_argument("--local-out", default="local_data/gazette-cases",
                    help="where the full-text corpus goes (gitignored)")
    ap.add_argument("--upstream", default=None,
                    help="a checkout of %s; cloned into tools/state/ otherwise" % UPSTREAM_URL)
    ap.add_argument("--no-local", action="store_true", help="skip the full-text corpus")
    ap.add_argument("--version", default="1.0.0")
    args = ap.parse_args()

    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")

    if args.upstream:
        root = Path(args.upstream)
        log("upstream %s" % root)
    else:
        root = Path("tools/state") / UPSTREAM_DIR
        if root.exists():
            log("upstream cached at %s (--upstream to point elsewhere)" % root)
            subprocess.run(["git", "-C", str(root), "pull", "--ff-only", "-q"],
                           check=False)
        else:
            root.parent.mkdir(parents=True, exist_ok=True)
            log("cloning %s" % UPSTREAM_URL)
            subprocess.run(["git", "clone", "--depth", "1", "-q",
                            UPSTREAM_URL, str(root)], check=True)

    files, upstream = load_upstream(root)
    log("upstream holds %d gazette documents" % len(files))
    return root, files, upstream, args


def make_record(path):
    rec, raw = build_record(path)
    rec["sections"] = {k: v for k, v in (
        ("来源", "%s%s年第%s期 · %s" % (SOURCE_NAME, rec["issue_year"],
                                   rec["issue_no"], rec["category"])),
        ("审理法院", rec["court"]), ("文书类型", rec["doc_type"]),
        ("案号", rec["case_number"]),
        ("裁判摘要", "\n".join(rec["summary"]))) if v}
    rec["section_order"] = list(rec["sections"])
    # The statutes a 裁判摘要 cites, as a JSON field rather than a CSV column. The
    # Gazette prints no 相关法条 section for a 裁判文书选登 document, and the 《…》
    # spans in a 摘要 are not all statutes — a contract reads the same way — so
    # this is a thing you can read about a case, not a thing you filter on.
    rec["laws_mentioned"] = re.findall(r"《([^》]+)》", " ".join(rec["summary"]))
    rec["full_text"] = full_text(rec, None)
    rec["full_text_chars"] = len(rec["full_text"])
    rec["full_text_sha256"] = hashlib.sha256(
        rec["full_text"].encode("utf-8")).hexdigest()
    # The body is returned beside the record, never inside it. A field that
    # happens to exist on the record is a field that gets written to disk, and
    # the public corpus must not hold the judgment text at all — see emit().
    return rec, "\n\n".join(raw.get("body") or [])


def with_body(rec, body):
    """The same record with the judgment body in 全文 — the local corpus only."""
    rec = dict(rec)
    rec["full_text"] = full_text(rec, body)
    rec["full_text_sha256"] = hashlib.sha256(
        rec["full_text"].encode("utf-8")).hexdigest()
    rec["full_text_chars"] = len(rec["full_text"])
    return rec


def body_pii_counts(bodies):
    """How much personal data the withheld bodies hold, as a sentence."""
    counts = {}
    for body in bodies.values():
        for name, rx in PII_PATTERNS.items():
            n = len(rx.findall(body))
            if n:
                counts[name] = counts.get(name, 0) + n
    docs = sum(1 for body in bodies.values()
               if any(rx.search(body) for rx in PII_PATTERNS.values()))
    return "%s across %d of the %d documents" % (
        ", ".join("%d %s" % (counts[k], k) for k in PII_PATTERNS if k in counts),
        docs, len(bodies)), counts


def main():
    root, files, upstream, args = resolve_upstream()
    built = [make_record(p) for p in files]
    records = sorted((r for r, _ in built),
                     key=lambda r: (r["issue_year"], r["issue_no"], r["gazette_id"]))
    bodies = {r["gazette_id"]: b for r, b in built}

    unknown = sorted({r["case_number_code"] for r in records if not r["case_group"]})
    log("案字 recognised for %d of %d records; unknown codes: %s"
        % (sum(1 for r in records if r["case_group"]), len(records), unknown or "none"))

    print("publish gate")
    assert_publishable(records, bodies)
    held, _ = body_pii_counts(bodies)
    emit(Path(args.out), records, corpus_manifest(args, records, upstream, "public", [
        "The 公报 prints the whole judgment, and the whole judgment carries the parties: the "
        "bodies here hold %s. This corpus therefore publishes the metadata and the 裁判摘要 "
        "only, and assert_publishable fails the build if any of those patterns appears in it."
        % held,
        "当事人, 案由, 法律依据, 裁判日期 and 公开日期 are empty because the Gazette publishes none "
        "of them as a field. 法院 is not: every document names the court that decided it.",
        "%d of the %d documents have no 裁判摘要 (the earliest 2003 issues), and %d carry more "
        "than one numbered holding." % (sum(1 for r in records if not r["summary"]),
                                      len(records),
                                      sum(1 for r in records if len(r["summary"]) > 1)),
        "%d records have a 案字 this table does not name, so 案件类型 is empty for them: %s."
        % (sum(1 for r in records if not r["case_group"]), unknown or "none"),
        "%d of the %d case titles name a party, exactly as the Gazette prints them; that is "
        "the citation, not a party list this corpus assembled."
        % (sum(1 for r in records if "诉" in r["title"]), len(records)),
        "全文 is plain text, so a search over it needs index_with_full_text = true; at %d kB "
        "of text that costs nothing." % (sum(r["full_text_chars"] for r in records) // 1000),
        "convert reports bad_jdate and bad_pdate for all %d rows: the two date columns are "
        "empty by design, and the counter reads an absent date as a bad one. no_case_year is 0 "
        "for every row, which is the date facet that actually works here — it comes from 案号."
        % len(records),
        "%d records quote a 《…》 in the 裁判摘要 (%d distinct spans); they are in the JSON as "
        "laws_mentioned and not in 法律依据, so the law facet stays honest rather than gaining "
        "contract titles. convert therefore reports law_ref 0."
        % (sum(1 for r in records if r["laws_mentioned"]),
           len({x for r in records for x in r["laws_mentioned"]})),
    ]))

    if args.no_local:
        return 0
    local = [with_body(r, bodies[r["gazette_id"]]) for r in records]
    emit(Path(args.local_out), local, corpus_manifest(
        args, local, upstream, "local", [
            "The same %d records with the judgment body in 全文. NOT for the repository: the "
            "bodies hold %s. Keep it under local_data/, which .gitignore excludes."
            % (len(records), held),
            "Use it for what the published corpus cannot do: 当事人 and 住所地 facets, "
            "full-text search over the reasoning, and how the importer behaves on documents "
            "up to 40 kB instead of 4 kB.",
            "Every other field is byte-identical to the published corpus: same 案号, same "
            "法院, same derived 案件类型 and 审理程序, same source_url. A query that works on "
            "one works on the other; only 全文 differs.",
        ]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
