# guiding-cases — 最高人民法院指导性案例

The guiding cases the Supreme People's Court publishes on its own site:
**279 case records** (指导案例1号 through 指导性案例279号, complete — 45号 has
no page on the site itself and comes from a documented mirror, see Gaps) and
**15 batch notices** (发布第N批指导性案例的通知). 780,339 characters of
judgment text, 2.3 MB of CSV.

This is the corpus the project's own README says it does not ship: it ships no
*judgment* corpus. Guiding cases are the exception — they are official
documents the SPC publishes precisely so that courts refer to them, the
publisher anonymises the parties itself (西某工业软件有限公司, 艾某, 王某群),
and each one is short enough to check by hand against the page it came from.

## Files

| file | what it is |
| --- | --- |
| `cases.csv` | the 279 cases in exactly the 15 columns `convert` imports — point `raw_data_path` at this directory and the engine reads it |
| `cases.jsonl` | the same 279 cases, one JSON object per line, with the published sections kept apart and the derived fields labelled |
| `notices.jsonl` | the batch notices, with the 文号, the batch number and the case-number ranges each one announces |
| `manifest.json` | hashes, counts, per-column provenance, fetch window, known gaps |

## Import it

```bash
cp config_sample.toml config.toml
# raw_data_path = "data/guiding-cases"     # loose CSV is accepted
# index_with_full_text = true              # the text is 全文; without this it is stored, not searchable
./target/release/convert config.toml --compact
./target/release/index   config.toml
./target/release/check   config.toml
```

Measured on this dataset, on the machine this was built on:

```
convert  rows 279 parsed 279 csv_errors 0 kept 279 dup 0 no_doc_id 0
         empty_text 0 html 0 bad_pdate 0 no_case_year 24 law_ref 278 chars 780339
index    Total 279 documents, 8 segments, 0s
check    279 documents checked: 0 failure(s)
```

Identity comes from `原始链接`: `doc_key()` falls back to the whole URL when no
`docId=` parameter is present, and every source URL is unique (the one mirror
URL among the court.gov.cn pages included), so `no_doc_id` is 0 and
re-importing changes nothing.

With `index_with_full_text = true`, keyword search over the corpus answers, for
example: 跳单 → 1 (指导案例1号), 证据保全 → 5, 违约 → 34, 合同 → 118, and
`case_type=刑事` alone → 48.

## Where every column comes from

`manifest.json` carries this table; it is the difference between a dataset and a
bunch of rows.

| column | origin | note |
| --- | --- | --- |
| 原始链接 | verbatim | the page the record was read from |
| 案件名称 | verbatim | the published title, 指导性案例NNN号 prefix included |
| 公开日期 | verbatim | the date on the listing page |
| 法律依据 | verbatim | the page's 相关法条 section |
| 全文 | verbatim | title, 发布 note and every published section, markup removed, nothing reworded |
| 案号 | derived | the case number of the **effective** judgment, read out of 裁判结果: the last number a 判决 follows, because a later 民事裁定 decides a retrial application |
| 案件类型 | derived | the first term of 关键词, with 民事诉讼/刑事诉讼/行政诉讼 folded into 民事/刑事/行政 |
| 案件类型编码 | derived | the code `extract.rs` documents for that 案件类型 |
| 法院, 所属地区, 审理程序, 当事人, 案由, 裁判日期 | empty | a guiding-case page publishes none of these as a field; nothing is inferred into a column a user filters on |

The JSON adds what the CSV cannot hold: `sections` (关键词 / 裁判要点 or
执行实施要点 / 基本案情 / 法院经审理查明 / 裁判结果 / 裁判理由 / 相关法条, in
the order the page prints them), `judgment_case_numbers` (every number 裁判结果
names), `courts_mentioned` (the courts that appear as the author of a decision:
`…法院于YYYY年…`), `keywords`, `batch` (from the notice that announced the
number), `publish_note`, `source_url`, `page_sha256` and `fetched_at`.

## Gaps, stated rather than papered over

* **指导案例45号 comes from a mirror, not from court.gov.cn.** 第10批
  (法〔2015〕85号) announces 指导案例45-52号, and the site's 指导案例 listing
  carries pages for 44 and 46 but was never given a page for 45 (no xiangqing
  page for it exists). Its record is the published text taken verbatim from the
  洛阳中院 mirror (`hnlyzy.hncourt.gov.cn/public/detail.php?id=6738`, a gov.cn
  court site that reprints the SPC's guiding-case page in full; cross-checked
  against a second court mirror and the SPC Gazette). All 1-279 numbers are
  therefore present; the crawler's `SUPPLEMENTS` table names the mirror.
* **24 cases have no 案号**: their pages publish no 裁判结果 section, or name no
  case number in it.
* **7 执行 cases use 执行实施要点** instead of 裁判要点; both are kept under their
  own section name, and "does this case have a 要点 section" is what the checker
  asserts.
* 基本案情 / 裁判结果 / 裁判理由 / 相关法条 are missing on a handful of pages
  (the counts are in the manifest); the publisher does not print them there.
* 79 of the 279 cases carry a `batch`. The listing holds 15 notices — 第1批 and
  第3批 through 第16批 — and 14 of them name the case-number ranges they cover
  (9-87, which is 79 of the cases here). The 第1批 notice names its four cases by
  title only, the 第2批 notice is not in this listing, and nothing after 第16批
  (2017) has one.

## Rebuild and re-verify

```bash
python3 tools/crawl_guiding_cases.py            # reads the cache in tools/state/, --refresh refetches
python3 tools/check_dataset.py --dir data/guiding-cases
```

The crawler caches every page under `tools/state/guiding-cases-raw` (gitignored)
and records its sha256 and fetch time per record, which is what `page_sha256`
is — the 指导案例45号 mirror page is cached there like any other. The checker
recomputes the file hashes, the record counts, the 全文 hashes,
and reads the expected CSV header out of `COLUMNS` in `src/bin/convert.rs`
rather than hardcoding it — so a dataset that stops matching the importer fails
loudly.

## Citing and redistributing

Cite the 指导案例 number and the `source_url` stored with the record. The text
is the SPC's, published for courts to refer to; this repository adds structure
and provenance, not content. The site footer carries 中华人民共和国最高人民法院
版权所有 — treat the text as official government work you may quote and cite,
and link to the source page rather than presenting your copy as the document.
