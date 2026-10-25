#!/usr/bin/env python3
"""Fingerprint a v2 server's answers, then compare another server against it.

Two indexes over the same corpus must contain the same documents, so every total
has to match exactly and the `sort=id` hit lists must match element for element
(that order is deterministic). BM25 order is not compared: documents that score
the same may come back in a different order when the segment layout changes.

fjall has no inter-process lock, so the two indexes cannot be served at the same
time from one keyspace — take a fingerprint, rebuild, then compare:

  python3 tools/cmp_v2.py --base http://127.0.0.1:5003 --dump /tmp/fp_old.json
  python3 tools/cmp_v2.py --base http://127.0.0.1:5007 --compare /tmp/fp_old.json
"""

import argparse
import json
import urllib.parse
import urllib.request

QUERIES = [
    {"search": "合同"},
    {"search": "原告 被告"},
    {"search": "借款 纠纷 合同"},
    {"search": "股权转让 协议"},
    {"search": "合同", "case_type": "民事案件"},
    {"search": "合同", "year_from": "2012", "year_to": "2016"},
    {"search": "合同", "has_text": "false"},
    {"search": "合同", "has_text": "true", "min_text_len": "2000"},
    {"search": "合同", "procedure_group": "民事审判"},
    {"court": "慈溪市人民法院"},
    {"law": "《中华人民共和国合同法》"},
    {"court_code": "京01"},
    {"search": "合同", "sort": "id", "limit": "100"},
    {"search": "借款", "sort": "id", "limit": "100"},
    {"case_type": "民事案件", "sort": "id", "limit": "100"},
]

FACETS = ["court", "year", "case_type", "procedure_group", "procedure_kind", "source"]


def api(base, qs, path="/api/search"):
    with urllib.request.urlopen(base.rstrip("/") + path + "?" + qs, timeout=180) as r:
        return json.load(r)


def walk(base, params, cap=40):
    """Page through a filtered set with the id cursor; return how many were seen."""
    n, cursor, guard = 0, None, 0
    while guard < cap:
        p = dict(params)
        p["limit"] = "100"
        if cursor is not None:
            p["id_gt"] = str(cursor)
        page = api(base, urllib.parse.urlencode(p))["cases"]
        if not page:
            break
        n += len(page)
        cursor = page[-1]["id"]
        guard += 1
    return n


def fingerprint(base):
    out = {"totals": {}, "ids": {}, "cursor": {}, "facets": {}}
    for q in QUERIES:
        label = ",".join(f"{k}={v}" for k, v in q.items())
        out["totals"][label] = api(base, urllib.parse.urlencode(q))["search_meta"]["total"]
        if q.get("sort") == "id":
            out["ids"][label] = [c["id"] for c in api(base, urllib.parse.urlencode(q))["cases"]]
    for q in [{"search": "合同", "sort": "id"}, {"has_text": "false", "sort": "id"}]:
        label = "cursor:" + ",".join(f"{k}={v}" for k, v in q.items())
        out["cursor"][label] = walk(base, q)
    for g in FACETS:
        agg = api(base, urllib.parse.urlencode({"group_by": g, "search": "合同"}), "/api/facets")[
            "facets"
        ].get(g, {})
        # keep buckets as pairs: JSON object keys would turn the numeric year
        # buckets into strings and the comparison below would be meaningless
        out["facets"][g] = [[b["key"], b["doc_count"]] for b in agg.get("buckets", [])]
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", required=True)
    ap.add_argument("--dump", help="write the fingerprint here")
    ap.add_argument("--compare", help="compare this server against a saved fingerprint")
    ap.add_argument(
        "--facet-tolerance",
        type=int,
        default=0,
        help="allowed bucket-count delta (terms aggregation runs per segment)",
    )
    args = ap.parse_args()

    got = fingerprint(args.base)
    if args.dump:
        with open(args.dump, "w") as f:
            json.dump(got, f, ensure_ascii=False)
        print(
            f"fingerprint: {len(got['totals'])} queries, "
            f"{len(got['ids'])} cursor-ordered sets, "
            f"{sum(len(v) for v in got['facets'].values())} facet buckets -> {args.dump}"
        )
        return

    want = json.load(open(args.compare))
    fails = []

    def check(name, ok, detail=""):
        if not ok:
            fails.append(f"{name}  {detail}")
        print(f"{'PASS' if ok else 'FAIL'}  {name}  {detail}")

    for label, total in want["totals"].items():
        got_total = got["totals"].get(label)
        check(f"total: {label}", got_total == total, f"{got_total} vs {total}")
    for label, hit_ids in want["ids"].items():
        check(f"sort=id hits identical: {label}", got["ids"].get(label) == hit_ids, f"{len(hit_ids)} ids")
    for label, n in want["cursor"].items():
        check(f"{label}", got["cursor"].get(label) == n, f"{got['cursor'].get(label)} vs {n}")
    for group, buckets in want["facets"].items():
        theirs = got["facets"].get(group, [])
        theirs_d = {k: n for k, n in theirs}
        mine = {k: n for k, n in buckets}
        check(
            f"facet keys identical: {group}",
            [k for k, _ in buckets] == [k for k, _ in theirs],
            f"{len(mine)} vs {len(theirs)} buckets",
        )
        worst = max((abs(theirs_d.get(k, 0) - n) for k, n in buckets), default=0)
        check(f"facet counts within {args.facet_tolerance}: {group}", worst <= args.facet_tolerance, f"max delta {worst}")

    if fails:
        print(f"\n{len(fails)} failure(s):")
        for f in fails:
            print(" -", f)
        raise SystemExit(1)
    print("\nindexes agree on every query checked")


if __name__ == "__main__":
    main()
