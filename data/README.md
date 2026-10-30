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

| dataset | records | what it is |
| --- | --- | --- |
| [`guiding-cases/`](guiding-cases/) | 278 cases + 15 notices | 最高人民法院指导性案例, 指导案例1号 through 指导性案例279号, as published on court.gov.cn |

Each dataset directory holds `manifest.json` (hashes, counts, field
definitions, provenance) and a README. `python3 tools/check_dataset.py --dir
data/<name>` re-verifies a dataset against its own manifest; the crawler that
built it lives in `tools/`, so a dataset can be rebuilt rather than trusted.
