# gazette-cases — 最高人民法院公报·裁判文书选登

**445 judgments the Supreme People's Court published in its own Gazette**, one
per 公报 issue, 2003年第1期 through 2026年第12期. Where a guiding case is the SPC's
summary of a case, a 公报 document is the case: the 案号, the court, the parties,
the reasoning, the whole thing.

This directory holds the half of that which may be published. The judgment text
itself stays in `local_data/gazette-cases`, which `.gitignore` excludes — see
*What is not here* below.

## Files

| file | what it is |
| --- | --- |
| `cases.csv` | the 445 records in the 15 columns `convert` imports — point `raw_data_path` here and the engine reads it |
| `cases.jsonl` | the same records with the 公报 issue, the derived 案字 reading and the statutes a 摘要 quotes kept apart |
| `manifest.json` | hashes, counts, per-column provenance, the checker's expectations, and the reason half the corpus is missing |

## Import it

```bash
cp config_sample.toml config.toml
# raw_data_path = "data/gazette-cases"
# index_with_full_text = true              # 全文 is 摘要-only; without this it is stored, not searchable
./target/release/convert config.toml --compact
./target/release/index   config.toml
./target/release/check   config.toml
```

Measured on this dataset:

```
convert  rows 445 parsed 445 csv_errors 0 kept 445 dup 0 no_doc_id 0
         empty_text 0 html 0 bad_jdate 445 bad_pdate 445 no_case_year 0 law_ref 0 chars 148636
index    Total 445 documents, 8 segments, 0s
check    445 documents checked: 0 failure(s)
verify_v2  34/34 checks passed
```

Keyword search over the 摘要 answers 专利 → 51, 商标 → 26, 管辖权 → 19,
国家赔偿 → 11, 破产 → 8, 提单 → 4. `case_type=刑事` → 12,
`procedure_group=行政` → 51, `court=最高人民法院赔偿委员会` → 9.

## What this corpus has that guiding-cases does not

A guiding-case page publishes a title, a 要点 and some sections; it never
publishes a court, a procedure or a date as a field. A 公报 document publishes
all three, so for the first time in this repository those columns are populated:

| column | guiding-cases | gazette-cases |
| --- | --- | --- |
| 法院 | empty for all 279 | filled for all 445 — 最高人民法院 436, 最高人民法院赔偿委员会 9 |
| 审理程序 | empty for all 279 | filled for 443, e.g. 民事二审 205, 民事再审 99, 民事再审审查与审判监督 53 |
| 案件类型 | from 关键词 | from the 案字 of 案号: 民事 360, 行政 51, 刑事 12, 执行 11, 国家赔偿 9 |
| 案号 | 24 of 279 have none | all 445 have one, so `no_case_year` is 0 and the year facet works off 案号 |

The 案字 reading follows 法〔2015〕295号《最高人民法院关于人民法院案件案号的若干
规定》: `民四终字` is a civil appeal, `最高法行再` an administrative retrial,
`法委赔字` a state-compensation decision. Two records carry a 案字 the table does
not name (`确申字`) and their 案件类型 is empty rather than guessed.

## What is not here, and why

The 公报 prints the whole judgment. The whole judgment carries the parties:

```
14 身份证号, 24 手机号, 965 住所地, 985 法定代表人, 1232 委托代理人
across 422 of the 445 documents
```

That is not what a guiding case looks like. The SPC anonymises a guiding case
with its own hand (西某工业软件有限公司, 艾某); a 公报 judgment names 朴赞善（PARK
CHAN SUN）and gives his company's address, and in six documents it prints an
identity document number. Publishing those is not this repository's to do, and
the rule in `data/README.md` is about exactly this.

So the corpus splits in two, and `tools/build_gazette_cases.py` writes both from
one run:

| | ships | stays local |
| --- | --- | --- |
| path | `data/gazette-cases` | `local_data/gazette-cases` |
| 全文 | metadata + 裁判摘要 | the same, plus the judgment body |
| size | 2.0 MB | 27 MB |
| in git | yes | no (`.gitignore`) |

Every other field is byte-identical between the two — same 案号, same 法院, same
derived 案件类型 and 审理程序, same source URL — so a query that works on one
works on the other and only 全文 differs. The local one is what you test 当事人
extraction, 住所地 facets and full-text search against.

The split is enforced, not promised. `assert_publishable` refuses to write the
public corpus if any of those patterns appears in it, and it fails closed: the
first version of the builder put the body on the record as a field, the JSONL
then written held every 身份证号 in the corpus, and the content check passed
because it only looked at 全文. The gate now checks the structure first (no
record may carry the judgment text under any key) and the text second. It also
counts the same patterns over the withheld bodies, because a gate that matched
nothing anywhere is a gate that had broken. `check_dataset.py` re-runs the
content half from the shipped CSV alone, so the promise holds on a checkout that
never ran the builder.

One caution for whoever greps for this next: the sha256 fields in `cases.jsonl`
are hex, and a hex string contains eleven-digit runs that read as phone numbers
to any regex. The shipped CSV holds none, and the shipped JSONL holds no real
one either — the pre-existing `guiding-cases/cases.jsonl` shows the same two
false hits, which is how you can tell the pattern is the noise, not the data.

## Where every column comes from

`manifest.json` carries this table.

| column | origin | note |
| --- | --- | --- |
| 原始链接 | verbatim | the gongbao.court.gov.cn page; all 445 are distinct |
| 案号 | verbatim | printed on the document; every record has one |
| 案件名称 | verbatim | the Gazette's own case name; 90 of the 445 name a party in it, as published |
| 法院 | verbatim | printed on the document |
| 案件类型, 审理程序 | derived | the 案字 of 案号 under 法〔2015〕295号; empty for a code the table does not name |
| 来源 | fixed | 最高人民法院公报 |
| 裁判日期, 公开日期 | empty | the Gazette dates an issue, not a judgment. The year a case was filed is in 案号 |
| 当事人, 案由, 法律依据 | empty | the Gazette publishes none as a field; the party list is in the body, which stays local |
| 全文 | verbatim | title, each published field under its own label, and the 裁判摘要 |

The JSON adds `issue_year` / `issue_no`, `doc_type` (判决书 293, 裁定书 139,
决定书 9, 调解书 2, 通知书 2), `case_number_code`, `case_number_year`,
`procedure`, `summary` (the 摘要 as a list — 149 records carry more than one
numbered holding), `laws_mentioned`, and `body_chars`: how many characters of
judgment text this record deliberately does not have.

## Gaps, stated rather than papered over

* **4 documents have no 裁判摘要** — the earliest 2003 issues. Their 全文 ends at
  案号.
* **2 records have an unknown 案字** (`确申字`), so 案件类型 and 审理程序 are empty.
* **`bad_jdate` and `bad_pdate` are 445 for every row.** Both date columns are
  empty by design and the counter reads an absent date as a bad one.
  `no_case_year` is 0, which is the date facet that actually works here.
* **`law_ref` is 0.** A 裁判文书选登 document has no 相关法条 section. 151 records
  quote a 《…》 in the 摘要 (116 distinct spans), but they are not all statutes — a
  contract is quoted the same way — so they are in the JSON as `laws_mentioned`
  rather than cut into a column users filter on.
* **90 of the 445 case titles name a party**, exactly as the Gazette prints them,
  and some of those are natural persons (何荣兰诉海科公司等清偿债务纠纷案,
  重庆市人民检察院诉陈新贪污、挪用公款案). That is the citation the Gazette itself
  prints, not a party list this corpus assembled — but it is why this corpus is a
  case index rather than an anonymised one. The other 355 titles name no party.

## Rebuild and re-verify

```bash
python3 tools/build_gazette_cases.py            # writes both corpora, gate included
python3 tools/build_gazette_cases.py --no-local # the publishable one only
python3 tools/check_dataset.py --dir data/gazette-cases
python3 tools/check_dataset.py --dir local_data/gazette-cases   # runs the PII census
```

The builder clones the upstream into `tools/state/` (gitignored) and records each
source file's sha256 per record as `upstream_sha256`.

## Citing and redistributing

The documents are judicial documents published in the 最高人民法院公报, which are
outside copyright under 著作权法 第五条; the upstream compilation that structured
them, [lttxzmj/chinese-law-corpus](https://github.com/lttxzmj/chinese-law-corpus),
is CC0 1.0. Cite the 公报 year and issue and the `source_url` stored with every
record. What this repository adds is the 案字 reading, the provenance and the
publish gate — not content.

