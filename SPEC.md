# Specification

What LocalCases must do, written before the code that does it. This file is also
the record of how the repository was produced: components marked
**reimplemented** were written against this specification and not against another
project's sources; components marked **carried over** were written for the same
author's earlier tool and contain no third-party code.

## 1. Purpose

A user has their own corpus of court judgments as CSV files. They want to search
it on their own machine: keyword search in Chinese, exact filters, counts by
court/year/type, and a desktop window. No network service, no shared corpus, no
account.

## 2. Import

* Input: a directory of source files. Accept ZIP archives whose members are CSV,
  and loose CSV files, with no packaging step for a user's own export.
* Columns are identified by header name, not position. The 15 columns of the
  original export and their common English/alternate spellings are recognized; an
  unrecognizable header must abort that file with a clear error rather than import
  empty rows; a missing column yields an empty value; an extra column is ignored.
* Stored values are byte-identical to the source, markup included. No stripping,
  no re-wrapping, no entity games.
* Identity: a row is identified by the document its link names. Re-importing an
  updated export must not create a second copy of a document that is already
  stored, and must not renumber documents that are already stored.
* Rows without a usable identity are still importable, get a positional key that
  cannot collide with another row, and are counted.
* A run that is interrupted resumes where it stopped without replaying rows into
  the counters twice.
* Ids are small integers, stable, and handed out in a deterministic order: running
  the import with different parallelism produces a byte-identical library.
* Counters describe reality: rows seen, kept, duplicates, empty bodies, rows
  without identity, date-parse failures, body length totals. A user must be able
  to see whether their corpus is dirty.
* Re-running the import of a corpus that is already imported changes nothing.

## 3. Index

* Full-text: Chinese tokenization, a stop-word list that is deliberately small —
  legal terms a searcher types (原告, 被告, 判决, 合同, 纠纷) must stay searchable.
  Words longer than a token threshold are junk and dropped.
* Every keyword search is a conjunction: two terms mean both.
* Identifiers (案号, 案件名称) rank above prose; a case number typed into the box
  finds that case first.
* Exact layer: values that are filtered or aggregated (court, region, cause, type,
  procedure, procedure group/kind, source, statute, court code) are indexed as
  single terms with column access, next to the tokenized fields rather than in
  place of them, so adding filters never changes existing keyword results.
* Numeric layer: year/month/day, judgment and publication dates as day numbers
  (unknown = a sentinel that is still countable), case year/sequence, party and
  statute counts, body length, has-text, has-markup.
* Sorting by relevance, by judgment date, or by id; cursor pagination by id must
  stay cheap however deep the page is.
* Facet counts are computed over the same query as the hits, including the error
  bound of approximate terms aggregations.
* An index built by an earlier version of this format must open and answer
  unchanged; the field names, analyzer chain and options are the on-disk contract.

## 4. Query API

Local HTTP, JSON only, no HTML rendering anywhere in the engine.

* `GET /api/search` — hits + `search_meta` (+ `facets` when `group_by` names
  groups). Parameters: keyword, offset, sort, the exact filters, date ranges,
  text-length bounds, `id_gt` cursor, facet groups and size.
* `GET /api/facets` — the same query, buckets instead of documents.
* `GET /api/case/{id}` — the stored row verbatim plus its derived facts. The
  engine returns text; the caller decides how to display it.
* `GET /api/export.csv` — the same query as a download, capped by a configured
  limit.
* `GET /api/health` — the service is up, and how many documents are indexed.
* A filter value that cannot be parsed is a client error, never a silently
  widened query. Unknown parameters are ignored, so older clients keep working.
* Full-width/half-width and simplified/traditional spellings find each other.

## 5. Exposure and safety

* Loopback by default. Nothing listens on a routable address unless the operator
  asks for it, and doing so without a token is logged as a warning.
* A shared secret (`x-localcases-token`) gates the API when configured: without
  it, any page open in the user's browser could read the whole library off
  loopback. The desktop shell always configures one.
* The webview is restricted to the loopback origins the shell actually binds.
* Requests time out; responses are compressed.
* The corpus directory is never committed, never uploaded, never sent anywhere.
* Importing is an act by the user, who is responsible for having a lawful basis to
  process the personal data inside judgments. The application must say so.

## 6. Desktop

* Tauri shell; the engine is linked into the app and served on a loopback port
  from a fixed range, with a token generated per run.
* The front-end is plain HTML/CSS/JS with no build step, and must never build
  markup out of a corpus value: a judgment containing HTML cannot inject content.
* The shell owns no query logic: one implementation of search and exposure, in the
  engine.

## 7. Verification (how a change is allowed to land)

* Importing the reference corpus again must produce a library identical
  entry-for-entry to the reference library.
* A fresh index of that library must answer a fixed query set with the same
  totals, cursors and facet buckets as the reference build.
* Counters, filters, facets and sorting are cross-checked against numbers
  recomputed from the source CSV files, not against a golden file.
* The API's internal consistency (facet bucket vs. filter total within the
  reported error bound, cursor paging vs. offset paging, export cap) is checked on
  every run.
* Unit tests cover header mapping, identity extraction, date and case-number
  parsing, procedure splitting and the token gate.

### What a run actually reports, and the one expected difference

On a 989,517-row corpus imported twice — once by this author's earlier tool, once
here — `compare` finds the two libraries identical entry for entry, and the query
fingerprint over a fresh index of each finds 31 of 32 checks identical. The one
difference is required by §3 rather than tolerated: that earlier tool's stop-word
list contained 原告, 被告 and 判决, so in its build a search for 原告 returned 0
documents. Here the stop-word list holds function words only, and the same queries
return 93,342, 119,287 and (both terms) 91,940. Any query naming one of those words
therefore differs between the two builds *by design*, and the fingerprint tool will
keep reporting it.

The storage layout — which values go into the value log, how large those files are
— is this project's own choice and deliberately not the earlier tool's. It changes
the shape of the files, not the entries a library holds, which is what `compare`
checks.

## 8. Provenance

| component | status |
| --- | --- |
| `src/model.rs`, `src/extract.rs`, `src/query.rs` | carried over (own work) |
| `src/bin/convert.rs`, `index.rs`, `check.rs`, `compare.rs` | carried over (own work); extended here for loose CSV and aliased headers |
| `tools/*.py` | carried over (own work) |
| `src/lib.rs`, `config.rs`, `search.rs`, `api.rs`, `server.rs`, `bin/serve.rs` | reimplemented from this specification |
| `stopwords.txt`, `ui/`, `desktop/`, this file | written for this project |

The library and index formats are deliberately compatible with the "v2" on-disk
format of the MPL-2.0 project this author's earlier tool belonged to, so a library
built there opens here. Compatibility is a property of the format: no source,
comment, string, help text, stop-word list or document from that project was
copied into this repository.

That was checked rather than asserted. The authorship of every file in the earlier
repository was read out of its history, which is what separates *carried over*
(this author's own files) from *reimplemented* (files the project's original author
wrote). The reimplemented files were then compared line by line against those
originals: no comment or prose line is shared with them, and the identical lines
that remain are derive/serde/cfg attributes, third-party API calls, and identifiers
the on-disk format forces (the partition names, the 15 stored column names). The
closest structural similarity of any reimplemented file is 34%; the stop-word list,
the README, the front-end and the desktop shell share nothing with the earlier
project at all.

