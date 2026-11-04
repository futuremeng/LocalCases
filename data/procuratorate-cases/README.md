# 最高人民检察院指导性案例 (`procuratorate-cases`)

250 cases, 检例第1号 through 检例第261号, published by the Supreme People's
Procuratorate in 60 batches at <https://www.spp.gov.cn/spp/jczdal/>. Built by
`tools/crawl_procuratorate_cases.py`; verified by `tools/check_dataset.py`.

This is the third corpus in the repository and the first that ships whole. The
SPP anonymises its own cases — 陈某, 辛某, 某公司 — and the gate confirms it
rather than assuming it: no 身份证号 and no 手机号 appears anywhere in 900 kB of
text. So there is no `local_data/` half of this corpus, and 全文 is the case as
published, not a 裁判摘要.

It is also the corpus that fills a column the other two leave empty. 【相关规定】
is the list of statutes the SPP itself gives for a case, and it is published for
229 of 250 of them. That is `法律依据`, which no guiding case and no Gazette
summary carries.

## What a record holds

`检例第N号` is the identity. It is printed on the page, assigned by the SPP, and
never reused — and it has to be the identity, because a batch page carries
several cases, so the page URL is not one.

| column | filled | where it comes from |
| --- | --- | --- |
| 案件名称 | 250 | the case name the page prints, verbatim |
| 全文 | 250 | title, 检例 number, 法律依据, then each 【…】 section |
| 关键词 | 247 | 【关键词】; the JSON splits it into a list |
| 要旨 | 245 | 【要旨】 |
| 基本案情 | 250 | 【基本案情】 |
| 履职过程 | 179 | 【检察机关履职过程】 or, in the older batches, 【诉讼过程】 |
| 指导意义 | 227 | 【指导意义】 |
| 法律依据 | 229 | 【相关规定】 / 【相关立法】 — published, not derived |
| 案件类型 | 197 | **derived**: the branch of law 法律依据 cites |
| 公开日期 | 250 | the date on the batch notice |
| 原始链接 | 250 | the batch page, plus `#检例第N号` |
| 案号, 法院, 审理程序, 当事人, 案由, 裁判日期, 所属地区 | 0 | not published as a field |

The empty ones are empty because the SPP does not publish them, not because the
crawler lost them. Courts appear constantly in the narrative — 衡水市中级人民法院
sentenced…, 河北省高级人民法院 ruled… — often several per case, and picking one
would be inventing a field. 裁判日期 is empty for a reason worth stating: the
SPP dates the batch notice, never the case, so 公开日期 is the only date there is.

## 案件类型 is derived, and only from what the SPP cites

These pages print no 案号 — 第一批 happens to mention one in prose and nothing
since does — so the 案字 trick that gives the Gazette corpus its 案件类型 and
审理程序 for every record is not available here. What is available is 法律依据:
a case whose listed statutes are all criminal is a criminal case.

刑事 123, 行政 40, 民事 34, and 53 left empty. The 53 are cases citing more than
one branch, or citing only 高检发释字 rules, which name no branch. The field is
left blank rather than guessed, because the SPP did not say what the citation
list is for.

## The section labels are not stable, and that was the interesting part

Written against the 2026 batch page, the parser reads 【要旨】【基本案情】
【检察机关履职过程】【相关规定】【关键词】【指导意义】. The older batches do not
use those names: 第四批 through 第三十二批 — 11 pages, 44 cases — write
【诉讼过程】 where the recent ones write 【检察机关履职过程】, and 第一批 through
第四批 write 【相关立法】 where the recent ones write 【相关规定】.

A crawler keyed on the 2026 spellings would have written 履职过程 empty for those
44 cases and 相关规定 empty for 20 of them, and nothing would have failed: an
empty field is not an error, it is just a corpus with a quiet hole in it. Both
spellings map to one canonical section now, and the manifest says so.

## What the corpus says about people

The gate runs over the whole corpus and reports two different things, because
they are not the same thing.

**Identifying data: none.** 身份证号 0, 手机号 0, and no 住址 in the corpus
carries an address — the four places the word appears are passages arguing about
what counts as personal data, which is a thing a 指导性案例 about information
crime does.

**Role words: counted, not failed.** 法定代表人 appears 75 times and carries a
name 15 times; 住所地 appears 16 times and 10 of those are where a *company* is
registered, at city granularity. An earlier version of this gate counted the word
and called 49 of 250 records dirty, which was mostly a description of Chinese
legal prose rather than of anything exposed.

**Names: 51 case titles name a party in full** — 马世龙（抢劫）核准追诉案,
于英生申诉案, 陈满申诉案. The SPP printed those names as the case's name. They
are the citation, in the same category as the 90 Gazette titles that name a
party, and withholding one record for one 法定代表人 while shipping 51 titles
would be a rule about a pattern rather than about exposure. 130 more titles name
a party through a 某.

## Two things this corpus corrected about the repository

**The gate's patterns were defined twice and had drifted.** The checker's
法定代表人 required a colon; the crawler's did not. One program called this
corpus clean and the other called it dirty, under the same name, in the same
repository. The patterns now live in the manifest and both programs read them
from there.

**`convert` keys a document by 原始链接.** It fell back to the whole URL because
no URL here carries a `docId=`, so every case on a batch page became one
document: 250 rows imported as 60 with 190 duplicates. The 检例 number is now
appended as a fragment — it resolves to the same page and to the right case on
it — and `page_url` in the JSON holds the bare address.

## Rebuild and re-verify

```bash
python3 tools/crawl_procuratorate_cases.py            # offline, from the cache
python3 tools/crawl_procuratorate_cases.py --refresh  # live
python3 tools/check_dataset.py --dir data/procuratorate-cases
```

The listing is walked by counting: `createPageHTML` advertises 62 entries and
the first page holds 30, so there are 3 pages. That is measured rather than read
from the script's arguments, because its second argument is 3 while the page
lists 30, and reading it as a page count dropped the last page — which is where
第一批 and 第二批 live, so 检例第1号 to 第8号 were absent without anything saying
so. 60 batch pages are found against 62 advertised; the difference is recorded in
the manifest rather than smoothed over.

Fetching is by `http.client`, not `urllib`. urllib sends its header names as
`User-agent` and `Accept-language`, which RFC 9110 says is fine and which the WAF
in front of spp.gov.cn answers with 403 to the whole channel. The identical
request through `http.client`, which sends the names as given, gets 200.

The CSV is LF, as every corpus file here is: `.gitattributes` stores the tree
that way, so a generator that writes CRLF records a hash no checkout can
reproduce.

## Gaps, stated rather than papered over

- 11 numbers are absent: 检例第57–59号, 81–84号, 90–93号. They are gaps in what
  the SPP publishes as 指导性案例, not in this crawl; the manifest lists them and
  the checker re-derives them.
- The channel advertises 62 entries and links 60 batch pages.
- 履职过程 is 179 of 250: 44 cases use the older label, which is mapped, and some
  batches have no such section at all.
- A 指导性案例 is not a judgment. It is the SPP's account of a case, published to
  be followed by other procuratorates, and its 全文 is that account. The engine's
  法院 and 案号 columns being empty is the honest shape of this source, not a
  parsing failure.
