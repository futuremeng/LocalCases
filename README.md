# LocalCases

Search your own corpus of Chinese court judgments **offline**. You point it at a
directory of CSV files, it builds a local library plus a full-text index, and you
query that from a desktop window or a local HTTP API. Nothing leaves your machine;
there is no server, no account and no download of anyone else's data.

Two things it is designed around:

* **Your data is the corpus.** Import tolerates missing columns, English headers,
  spreadsheet BOMs and rows without a source link. Duplicates are detected, ids
  are stable, and an interrupted import resumes where it stopped.
* **Answers you can check.** Exact filters (court, region, procedure, statute,
  year, date range, text length) and facet counts are computed over the *same*
  query as the hits, so a bucket count always describes the filtered set.

## Import, index, search

```bash
cargo build --release                     # engine + CLI
cp config_sample.toml config.toml         # db / index_path / raw_data_path / addr
./target/release/convert config.toml --compact   # CSV -> library
./target/release/index   config.toml             # library -> index
./target/release/serve   config.toml             # local API + service
```

Then either open the desktop app (`desktop/`, see below) or query the API:

```bash
curl -s localhost:5010/api/search --get \
  --data-urlencode 'search=房屋买卖合同' \
  --data-urlencode 'court=成都市中级人民法院' \
  --data-urlencode 'year_from=2019' \
  --data-urlencode 'group_by=court,year'
```

Search is a conjunction: `借款 合同` means documents holding both. The stop-word list
is deliberately tiny — function words only — so 原告, 被告 and 判决 stay ordinary
searchable terms instead of noise dropped at index time. An import also keeps its
write journal until the library is opened again: the 1.54 GB sample this project is
verified against sits near 7 GB during a run and settles around 2 GB afterwards, and
`convert --compact` merges the trees the bulk-load settings leave behind.

`check` verifies a built library against itself (every stored id resolves back to
the record it came from, identity keys are unique, counts match the index);
`compare` diffs two libraries entry-for-entry, which is how the import path is
kept honest across changes.

## Import format

A CSV is anything with a header row. Columns are matched by name and may be
missing; unknown columns are ignored. The names below are all understood, in
Chinese or English spelling (see `COLUMNS` in `src/bin/convert.rs`):

| stored column | accepted headers |
| --- | --- |
| 原始链接 | 原始链接, url, link, doc_url, docid, 链接 |
| 案号 | 案号, case_id, case_number, caseNo |
| 案件名称 | 案件名称, case_name, title, 标题 |
| 法院 | 法院, court, court_name, 审理法院 |
| 所属地区 | 所属地区, region, area, 地区 |
| 案件类型 / 案件类型编码 | 案件类型, case_type, type / 案件类型编码, case_type_code |
| 来源 | 来源, source |
| 审理程序 | 审理程序, procedure, trial_procedure |
| 裁判日期 / 公开日期 | 裁判日期, judgment_date, date / 公开日期, public_date, publish_date |
| 当事人 / 案由 / 法律依据 | 当事人, parties, party / 案由, cause, reason / 法律依据, legal_basis, laws |
| 全文 | 全文, full_text, text, 正文, content |

Values are stored verbatim, markup included — the engine never rewrites your
text, and it never renders it either: `/api/case/{id}` returns the row as JSON and
escaping is the front-end's business.

Identity: a row is the document its `原始链接` names (the `docId=` part), so
re-importing an updated export updates in place instead of duplicating. Rows with
no usable link get a positional key and are counted as `no_doc_id`.

## Desktop

`desktop/` is a Tauri shell around the engine: the service runs inside the app on
a loopback port with a per-run token, so the webview can only talk to your own
library and a page in your browser cannot read it.

```bash
cd desktop
LOCALCASES_CONFIG=../config.toml cargo tauri dev     # run against a built library
LOCALCASES_CONFIG=../config.toml cargo tauri bundle  # .deb / .AppImage / .msi / .app
```

The shell needs Tauri's system dependencies, which on Debian/Ubuntu are:

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev
cargo install tauri-cli --locked
```

The desktop is a separate Cargo workspace, so its dependency tree differs from the
engine's; `tauri.conf.json` ships `LICENSE`, `NOTICE` and a `LICENSES.third-party.md`
inside the bundle. Generate that list from inside `desktop/` before packaging:

```bash
cd desktop && python3 ../tools/licenses.py
```

The UI lives in `ui/` (plain HTML/CSS/JS, no build step): search box, facet
sidebar, result list, full-text panel, CSV export. It renders every field through
`textContent`, so a judgment containing markup cannot inject anything.

## Layout

| path | what it is |
| --- | --- |
| `src/model.rs`, `src/extract.rs` | the row, and the facts derived from it (dates as day numbers, 案号 parts, court code, procedure split, statute count, text profile) |
| `src/search.rs` | the tantivy schema and analyzer; the on-disk format |
| `src/query.rs` | filter/sort/facet parsing into tantivy queries and aggregations |
| `src/api.rs`, `src/server.rs` | the JSON API and how it is exposed (token gate, CORS, timeouts) |
| `src/bin/convert.rs` | import: CSV/zip → library, checkpointed, parallel readers, single writer |
| `src/bin/index.rs` | library → index |
| `src/bin/check.rs`, `src/bin/compare.rs` | self-verification of a library |
| `tools/` | verification against the source CSVs, API spot checks, fingerprint comparison, third-party license inventory |
| `ui/`, `desktop/` | the desktop front-end and Tauri shell |

## Data, and what you are agreeing to

This project ships **no corpus**. Judgments contain names, addresses, phone
numbers and company details of real people: importing, storing, sharing or
publishing them is your responsibility, and in most places that means you need a
lawful basis to process them. Keep the library directory like the data itself —
the `.gitignore` here excludes it, and the API listens on loopback by default.

Nothing in LocalCases redacts anything. A name, an address or an ID number survives
import, search, the detail view and the CSV export exactly as your source file held
it, on purpose: a tool that quietly rewrote a judgment could not be checked against
the judgment. If your use requires masking, do it where you can see it — in the CSV
before importing, or on the way out of an export.

`tools/verify_v2.py` and `tools/spot_v2_api.py` expect a corpus of your own and
read it read-only; they are how a new build is checked against the files it was
fed rather than against a golden file that can drift.

## License

Apache-2.0 — see `LICENSE` and `NOTICE`. `LICENSES.third-party.md`
(`python3 tools/licenses.py`) lists every crate linked into the binaries with its
SPDX identifier; ship that file with any desktop bundle.
