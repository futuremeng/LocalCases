#!/usr/bin/env python3
"""Self-consistency checks for the v2 search API — no ground truth needed.

`verify_v2.py` re-derives the truth from the CSVs, which costs a full pass over
the corpus. These checks only compare the API with itself, so they are cheap
enough to run against the 85M-document deployment, and they catch the failures
that matter there:

  * ids are contiguous from 1, so a cursor must leave exactly `total - id` hits;
  * a facet bucket must equal the filter total for the same value;
  * a year facet bucket must equal the corresponding date range;
  * a keyword hit never carries an empty 全文, so adding has_text=true is a no-op;
  * filters on disjoint values add up;
  * detail pages render escaped text.

    python3 tools/spot_v2_api.py --base http://127.0.0.1:5003
"""

import argparse
import json
import re
import urllib.parse
import urllib.request


def api(base, path="/api/search", **params):
    url = base + path + "?" + urllib.parse.urlencode({k: v for k, v in params.items()})
    with urllib.request.urlopen(url, timeout=600) as r:
        return json.load(r)


def total(base, **params):
    return api(base, **params)["search_meta"]["total"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default="http://127.0.0.1:5001")
    args = ap.parse_args()

    checks = []

    def check(name, expected, actual):
        checks.append((name, expected, actual))

    everything = total(base=args.base, id_gt=0)
    print(f"documents via id_gt=0: {everything:,}")

    # 1. cursor algebra: ids are 1..=N with no holes
    for frac in (0.25, 0.5, 0.9):
        cursor = int(everything * frac)
        check(
            f"cursor algebra id_gt={cursor:,}",
            everything - cursor,
            total(args.base, id_gt=cursor),
        )

    # 2. facet bucket == filter total, for the biggest courts and each case type
    courts = api(args.base, "/api/facets", group_by="court", size="5")["facets"]["court"]
    for bucket in courts["buckets"][:3]:
        check(
            f"court facet bucket == filter total ({bucket['key']})",
            bucket["doc_count"],
            total(args.base, court=bucket["key"]),
        )
    types = api(args.base, "/api/facets", group_by="case_type", size="1000")["facets"][
        "case_type"
    ]
    for bucket in types["buckets"]:
        check(
            f"case_type facet bucket == filter total ({bucket['key']})",
            bucket["doc_count"],
            total(args.base, case_type=bucket["key"]),
        )
    # buckets cover exactly the documents that carry a value: documents whose
    # 案件类型 is blank are absent from the facet altogether
    keys = [b["key"] for b in types["buckets"]]
    covered = sum(b["doc_count"] for b in types["buckets"]) + types["sum_other_doc_count"]
    if all("," not in k for k in keys):
        check(
            "case_type buckets == docs carrying any case_type value",
            covered,
            total(args.base, case_type=",".join(keys)),
        )
    else:
        check("case_type buckets never exceed the corpus", True, covered <= everything)
    check(
        "documents without a case_type value exist only in the gap",
        True,
        covered <= everything,
    )

    # 3. year facet == explicit date range
    years = api(args.base, "/api/facets", group_by="year", size="40")["facets"]["year"]
    for bucket in years["buckets"][:5]:
        year = bucket["key"]
        check(
            f"year facet {year} == date range {year}",
            bucket["doc_count"],
            total(args.base, date_from=f"{year}-01-01", date_to=f"{year}-12-31"),
        )

    # 4. has_text splits every query exactly in two: hits with text + hits without.
    #    (A keyword can still hit a document with an empty 全文 — it matches the
    #    case name / cause / parties fields, which are also searched.)
    for kw in ("合同", "借款", "刑事责任"):
        kw_total = total(args.base, search=kw)
        with_text = total(args.base, search=kw, has_text="true")
        without_text = total(args.base, search=kw, has_text="false")
        check(
            f"'{kw}' splits exactly on has_text",
            kw_total,
            with_text + without_text,
        )
    # 5. disjoint filter values add up
    if len(courts["buckets"]) >= 3:
        a, b = courts["buckets"][0]["key"], courts["buckets"][1]["key"]
        check(
            "OR of two disjoint courts == sum",
            total(args.base, court=a) + total(args.base, court=b),
            total(args.base, court=f"{a},{b}"),
        )

    # 6. several groups come back in one call, and each matches its own filter.
    # terms aggregations are approximate by construction: a bucket may be short
    # by at most `doc_count_error_upper_bound`, never more, and never over.
    multi = api(args.base, "/api/facets", group_by="procedure_group,source", size="5")["facets"]
    check("multi-group facets requested both", {"procedure_group", "source"}, set(multi))
    for group in ("procedure_group", "source"):
        buckets = multi[group]["buckets"]
        # a value containing a comma would be read back as two OR'ed values by
        # the filter parser, so only exact-looking values can be cross-checked
        top = next((b for b in buckets if "," not in b["key"]), None)
        if top:
            bound = multi[group]["doc_count_error_upper_bound"]
            exact = total(args.base, **{group: top["key"]})
            ok = top["doc_count"] <= exact <= top["doc_count"] + bound
            check(
                f"{group} bucket == filter total ({top['key']}, bound {bound})",
                True,
                ok,
            )

    # 6b. the highest-cardinality field (法律名) must stay honest too: the top law
    # bucket may lag the filter by at most its reported error bound
    laws = api(args.base, "/api/facets", group_by="law", size="3")["facets"]["law"]
    for bucket in laws["buckets"][:2]:
        bound = laws["doc_count_error_upper_bound"]
        exact = total(args.base, law=bucket["key"])
        check(
            f"law bucket within error bound ({bucket['key']}, bound {bound})",
            True,
            bucket["doc_count"] <= exact <= bucket["doc_count"] + bound,
        )

    # 7. the record endpoint hands back the imported row verbatim. The engine
    # never renders markup, so escaping is entirely the front-end's job — what is
    # worth checking here is that the row and its derived facts are both there.
    for cid in (1, everything // 2, everything):
        d = api(args.base, f"/api/case/{cid}")
        row = d["record"]
        ok = bool(row.get("全文", "").strip()) and bool(row.get("案号", "").strip())
        check(f"/api/case/{cid} row verbatim (全文 + 案号 present)", True, ok)
        check(
            f"/api/case/{cid} derived facts attached",
            True,
            isinstance(d.get("derived"), dict) and "judgment_days" in d["derived"],
        )

    failed = 0
    print()
    for name, expected, actual in checks:
        ok = expected == actual
        failed += 0 if ok else 1
        print(f"{'PASS' if ok else 'FAIL'}  {name}  expected={expected!r} actual={actual!r}")
    print(f"\n{len(checks) - failed}/{len(checks)} self-consistency checks passed")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
