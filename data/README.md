# Bundled datasets

Everything under this directory is data the repository ships with, and the rule
for shipping it is narrow:

* a dataset here is published by a state organ as a document for courts and the
  public to use, and carries no personal data that the publisher did not itself
  publish — the guiding cases below name their parties as 某 by the publisher's
  own hand;
* every record keeps the URL it came from, so any claim about it can be checked
  at the source;
* the corpus a user imports stays the user's own (see the README: `sample_data/`,
  the library and the index are all gitignored). Nothing here is a substitute
  for that.

A fourth rule fell out of the second dataset. Some material is public at its
source and still not ours to put in a repository: a 公报 judgment is an official
judicial document, and it also prints 身份证号 and 住所地, because a court document
is meant to identify the parties. The answer is not to drop the corpus and not
to ship it — it is to ship the part that is a case citation and keep the rest in
`local_data/`, which `.gitignore` excludes. Fetch it, test against it, do not
commit it.

| dataset | records | what it is |
| --- | --- | --- |
| [`guiding-cases/`](guiding-cases/) | 279 cases + 15 notices | 最高人民法院指导性案例, 指导案例1号 through 指导性案例279号, as published on court.gov.cn (指导案例45号, which the site never gave a page, from a documented court mirror) |
| [`gazette-cases/`](gazette-cases/) | 445 documents | 最高人民法院公报·裁判文书选登, 2003–2026: 案号, 法院, 文书类型 and 裁判摘要 for every one — and, by design, not the judgment text |
| [`procuratorate-cases/`](procuratorate-cases/) | 250 cases | 最高人民检察院指导性案例, 检例第1号 through 第261号, in full: the SPP anonymises its own cases, so this one ships whole, and it is the only corpus here with a published 法律依据 |

Three corpora, three different answers to what may be shipped, which is the
thing worth reading before adding a fourth: the guiding cases name their parties
as 某 by the publisher's hand, the Gazette prints 身份证号 so only the citation
ships, and the SPP's cases name real parties in the case title and nothing that
identifies them to a stranger.

Each dataset directory holds `manifest.json` (hashes, counts, field
definitions, provenance, and the checks `tools/check_dataset.py` should run
against it) and a README. `python3 tools/check_dataset.py --dir data/<name>`
re-verifies a dataset against its own manifest; the tool that built it lives in
`tools/`, so a dataset can be rebuilt rather than trusted.

One invariant makes a recorded hash mean anything: the corpus files are LF, and
the generators write LF. `.gitattributes` pins the tree to `eol=lf`, so git
stores what a checkout yields — if a generator writes CRLF, the manifest hashes
bytes that no checkout can produce, and the corpus fails its own hash check on
every machine except the one that built it. Both generators did exactly that
before a clean checkout caught it.

`local_data/` follows the same layout and the same manifest, so the same checker
runs on it — with the opposite verdict where the two corpora differ on purpose:

```bash
python3 tools/build_gazette_cases.py                        # writes both halves
python3 tools/check_dataset.py --dir data/gazette-cases      # publish gate: must be clean
python3 tools/check_dataset.py --dir local_data/gazette-cases  # census: what is held back
```

The second command reports `14 身份证号, 24 手机号, 965 住所地` across 422 of the 445
rows. That number is the reason the first command's corpus is 2.0 MB and the
second's is 27 MB.
