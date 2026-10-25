#!/usr/bin/env python3
"""Acceptance check for the v2 structured layer.

Ground truth is computed by scanning the raw CSVs (the bottom layer) with the
same identity rule convert_v2 uses (dedup by docId, first occurrence wins).
Every structured filter / facet the API offers is then compared against that
ground truth, plus internal-consistency identities (facet bucket count == filter
total, facet sum == total, cursor algebra).

    python3 tools/verify_v2.py --base http://127.0.0.1:5001 --raw sample_data
"""

import argparse
import collections
import csv
import glob
import io
import json
import os
import re
import urllib.parse
import urllib.request
import zipfile

csv.field_size_limit(10**9)

LAW_RE = re.compile(r"《([^》]+)》")


def api(base, path, **params):
    url = base + path + "?" + urllib.parse.urlencode(params)
    with urllib.request.urlopen(url, timeout=300) as r:
        return json.load(r)


def total(base, **params):
    return api(base, "/api/search", **params)["search_meta"]["total"]


def doc_key(url):
    if "docId=" in url:
        return url.rsplit("docId=", 1)[-1]
    if "=" in url:
        return url.rsplit("=", 1)[-1]
    return ""


# Mirrors extract.rs so the derived layer is checked against an independent
# implementation rather than against itself.
PROCEDURE_KINDS = [
    "再审审查与审判监督",
    "非诉执行审查",
    "死刑复核",
    "国家赔偿",
    "其他",
    "一审",
    "二审",
    "再审",
    "复议",
    "实施",
    "审查",
    "监督",
    "清算",
    "重整",
    "和解",
    "破产",
    "执行",
    "程序",
]
PROCEDURE_GROUPS = [
    "国家赔偿",
    "司法救助",
    "强制清算",
    "知识产权",
    "民事",
    "刑事",
    "行政",
    "执行",
    "赔偿",
    "管辖",
    "区际",
]


def group_of(procedure, case_type):
    for g in PROCEDURE_GROUPS:
        if procedure.startswith(g):
            return g
    for g in PROCEDURE_GROUPS:
        if case_type.startswith(g):
            return g
    return ""


def proc_group(procedure, case_type):
    parts = procedure.split()
    if not parts:
        return ""
    first = parts[0]
    if len(parts) > 1:
        return first
    for kind in PROCEDURE_KINDS:
        if first.endswith(kind):
            group = first[: len(first) - len(kind)].strip()
            return group if group else group_of(first, case_type)
    return group_of(first, case_type)


def looks_like_html(text):
    return "<p" in text or "</" in text or "<div" in text or "c_header" in text


def scan(raw_dir):
    """Ground truth straight from the CSVs, applying v2's dedup rule."""
    kept = dup = empty_text = html_text = with_court = with_date = 0
    courts = collections.Counter()
    case_types = collections.Counter()
    proc_groups = collections.Counter()
    years = collections.Counter()
    court_year = collections.Counter()
    laws = collections.Counter()
    dates = []
    seen = set()

    for zip_path in sorted(glob.glob(os.path.join(raw_dir, "*.zip"))):
        with zipfile.ZipFile(zip_path) as z:
            members = sorted(n for n in z.namelist() if n.endswith(".csv"))
            for member in members:
                with z.open(member) as fh:
                    reader = csv.DictReader(
                        # utf-8-sig: the CSVs start with a BOM, which would
                        # otherwise poison the first column name.
                        io.TextIOWrapper(fh, encoding="utf-8-sig", errors="replace")
                    )
                    for i, row in enumerate(reader, 1):
                        key = doc_key((row.get("原始链接") or "").strip())
                        if not key:
                            key = f"row/{os.path.basename(zip_path)}/{member}#{i}"
                        if key in seen:
                            dup += 1
                            continue
                        seen.add(key)
                        kept += 1

                        text = (row.get("全文") or "").strip()
                        if not text:
                            empty_text += 1
                        if looks_like_html(text):
                            html_text += 1
                        court = (row.get("法院") or "").strip()
                        if court:
                            with_court += 1
                            courts[court] += 1
                        ctype = (row.get("案件类型") or "").strip()
                        if ctype:
                            case_types[ctype] += 1
                        proc_groups[proc_group(row.get("审理程序") or "", ctype)] += 1
                        jdate = (row.get("裁判日期") or "").strip()
                        if len(jdate) >= 4 and jdate[:4].isdigit():
                            with_date += 1
                            years[int(jdate[:4])] += 1
                            court_year[(court, int(jdate[:4]))] += 1
                            dates.append(jdate)
                        # distinct per document: the index stores one term per
                        # (doc, law), and a doc citing a law twice is one hit.
                        for name in {n.strip() for n in LAW_RE.findall(row.get("法律依据") or "")}:
                            laws[name] += 1

    return dict(
        kept=kept,
        dup=dup,
        empty_text=empty_text,
        html_text=html_text,
        proc_groups=proc_groups,
        with_court=with_court,
        with_date=with_date,
        courts=courts,
        case_types=case_types,
        years=years,
        court_year=court_year,
        laws=laws,
        max_date=max(dates) if dates else None,
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default="http://127.0.0.1:5001")
    ap.add_argument("--raw", default="sample_data")
    args = ap.parse_args()

    gt = scan(args.raw)
    checks = []

    def check(name, expected, actual):
        checks.append((name, expected, actual, expected == actual))

    all_total = total(args.base, id_gt=0)
    check("index document count", gt["kept"], all_total)

    # presence filters
    check("has_text=false", gt["empty_text"], total(args.base, has_text="false"))
    check("has_text=true", gt["kept"] - gt["empty_text"], total(args.base, has_text="true"))
    # markup detection drives the escaping guarantee on the detail page
    check("has_html=false", gt["kept"] - gt["html_text"], total(args.base, has_html="false"))
    check("has_html=true", gt["html_text"], total(args.base, has_html="true"))

    # derived procedure group: independent re-derivation of extract::split_procedure
    for group, _ in gt["proc_groups"].most_common(3):
        if group:
            check(
                f"procedure_group={group}",
                gt["proc_groups"][group],
                total(args.base, procedure_group=group),
            )
    # exact term filters
    top_courts = [c for c, _ in gt["courts"].most_common(3)]
    for court in top_courts:
        check(f"court={court}", gt["courts"][court], total(args.base, court=court))
    check(
        "court=a,b (OR of disjoint values)",
        sum(gt["courts"][c] for c in top_courts),
        total(args.base, court=",".join(top_courts)),
    )
    for ctype, _ in gt["case_types"].most_common(3):
        check(f"case_type={ctype}", gt["case_types"][ctype], total(args.base, case_type=ctype))
    for law, _ in gt["laws"].most_common(3):
        check(f"law={law}", gt["laws"][law], total(args.base, law=law))

    # ranges
    for year in sorted(gt["years"])[-3:]:
        expected = gt["years"][year]
        check(
            f"year_from={year}&year_to={year}",
            expected,
            total(args.base, year_from=year, year_to=year),
        )
        check(
            f"date_from={year}-01-01&date_to={year}-12-31",
            expected,
            total(args.base, date_from=f"{year}-01-01", date_to=f"{year}-12-31"),
        )
        check(
            f"bare year bounds {year}",
            expected,
            total(args.base, date_from=year, date_to=year),
        )

    court, year = top_courts[0], sorted(gt["years"])[-1]
    check(
        f"conjunction court+year ({court}, {year})",
        gt["court_year"][(court, year)],
        total(args.base, court=court, year_from=year, year_to=year),
    )

    # facets: sums and cross-checks against filters
    facets = api(args.base, "/api/facets", group_by="court", size="1000")["facets"]["court"]
    check(
        "facet court sum (+other) == docs carrying a court",
        gt["with_court"],
        facets["sum_other_doc_count"] + sum(b["doc_count"] for b in facets["buckets"]),
    )
    check("docs without a court", gt["kept"] - gt["with_court"], all_total - gt["with_court"])
    # the derived group must cover every document its re-derivation labels
    pg = api(args.base, "/api/facets", group_by="procedure_group", size="1000")["facets"][
        "procedure_group"
    ]
    check(
        "facet procedure_group sum (+other) == docs with a derivable group",
        sum(v for k, v in gt["proc_groups"].items() if k),
        pg["sum_other_doc_count"] + sum(b["doc_count"] for b in pg["buckets"]),
    )
    if facets["buckets"]:
        top = facets["buckets"][0]
        check(
            f"facet bucket == filter total ({top['key']})",
            top["doc_count"],
            total(args.base, court=top["key"]),
        )
    years_facet = api(args.base, "/api/facets", group_by="year", size="500")["facets"]["year"]
    check(
        "facet year sum == docs with valid judgment date",
        sum(gt["years"].values()),
        years_facet["sum_other_doc_count"]
        + sum(b["doc_count"] for b in years_facet["buckets"]),
    )

    # sorting
    by_date = api(args.base, "/api/search", sort="date")["cases"][0]
    check("sort=date first judgment_date", gt["max_date"], by_date["judgment_date"])
    by_id = api(args.base, "/api/search", sort="id")["cases"]
    check("sort=id first id", 1, by_id[0]["id"])
    check("sort=id ascending", len(by_id), by_id[-1]["id"])

    # cursor algebra: ids are contiguous from 1, so everything above a cursor
    # must be exactly `kept - cursor` documents (cursor derived from the corpus
    # size, so it stays meaningful for small samples too)
    cursor = max(1, gt["kept"] // 2)
    check("id_gt algebra", gt["kept"] - cursor, total(args.base, id_gt=cursor))
    check(
        "id_gt beyond the highest id returns nothing",
        0,
        total(args.base, id_gt=gt["kept"] + 1000),
    )
    page1 = api(args.base, "/api/search", sort="id", export="true", id_gt=0)["cases"]
    page2 = api(
        args.base, "/api/search", sort="id", export="true", id_gt=page1[-1]["id"]
    )["cases"]
    check("cursor page sizes", (10000, 10000), (len(page1), len(page2)))
    check("cursor pages ordered", True, page1[-1]["id"] < page2[0]["id"])

    # keyword + filter combination
    kw = "合同"
    kw_total = total(args.base, search=kw)
    kw_facets = api(args.base, "/api/facets", search=kw, group_by="court", size="5")["facets"][
        "court"
    ]
    for bucket in kw_facets["buckets"]:
        check(
            f"keyword facet bucket == filtered total ({bucket['key']})",
            bucket["doc_count"],
            total(args.base, search=kw, court=bucket["key"]),
        )
    covered = sum(b["doc_count"] for b in kw_facets["buckets"]) + kw_facets["sum_other_doc_count"]
    check(
        "keyword facet coverage <= keyword total (docs without court excluded)",
        True,
        covered <= kw_total,
    )

    # the record endpoint: the engine returns the stored row verbatim, so markup
    # can only ever be introduced by the front-end. What matters here is that the
    # row, its derived facts and its id all come back intact.
    case_id = by_id[0]["id"]
    rec = api(args.base, f"/api/case/{case_id}")
    body = rec["record"]["全文"]
    check("record endpoint returns 全文", True, bool(body.strip()))
    check("record endpoint returns 案号", True, bool(rec["record"]["案号"].strip()))
    check("record endpoint carries derived facts", True, "judgment_days" in rec.get("derived", {}))
    check("record id matches the request", case_id, rec["id"])

    width = max(len(c[0]) for c in checks)
    failed = 0
    for name, expected, actual, ok in checks:
        if not ok:
            failed += 1
        print(f"{'PASS' if ok else 'FAIL'}  {name.ljust(width)}  expected={expected!r} actual={actual!r}")
    print(f"\n{len(checks) - failed}/{len(checks)} checks passed")
    print(
        f"ground truth: kept={gt['kept']} dup={gt['dup']} empty_text={gt['empty_text']} "
        f"html={gt['html_text']} groups={len([k for k in gt['proc_groups'] if k])} "
        f"courts={len(gt['courts'])} case_types={len(gt['case_types'])} laws={len(gt['laws'])} "
        f"years={min(gt['years'])}-{max(gt['years'])}"
    )
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
