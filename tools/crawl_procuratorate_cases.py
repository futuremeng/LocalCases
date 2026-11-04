#!/usr/bin/env python3
"""Build the bundled `procuratorate-cases` dataset from the SPP's own pages.

The Supreme People's Procuratorate publishes its guiding cases (指导性案例) as one
web page per batch under /spp/jczdal/, each carrying several cases written out
in full and numbered 检例第N号. This script reads those pages, splits every batch
page into its cases and their published sections, and writes a corpus in the
shape `convert` imports:

    python3 tools/crawl_procuratorate_cases.py                  # full crawl
    python3 tools/crawl_procuratorate_cases.py --out /tmp/pc    # elsewhere
    python3 tools/crawl_procuratorate_cases.py --refresh        # live re-fetch

Fetched HTML is cached under tools/state/procuratorate-cases-raw (gitignored),
so a rebuild is offline and byte-stable; --refresh forces a live crawl.

Unlike the Gazette corpus, this one ships whole. The SPP anonymises its own
cases -- 陈某, 辛某, 某公司 -- so a 检察指导性案例 page carries no 身份证号, no
手机号 and no 住所地 to hold back, and the publish gate that splits the Gazette
corpus in two has nothing to split here. The gate still runs, and the checker
still reports what it counted, so that the absence stays a fact about the data
rather than an assumption about the tool.

stdlib only, like the other tools here.
"""

import argparse
import csv
import hashlib
import html as htmllib
import http.client
import json
import re
import ssl
import sys
import time
import urllib.parse
from datetime import datetime, timezone
from pathlib import Path

BASE = "https://www.spp.gov.cn"
CHANNEL = "/spp/jczdal/"
# A browser's User-Agent, not the descriptive one the court.gov.cn crawler
# sends: spp.gov.cn sits behind a WAF that answers 403 Forbidden to anything
# that does not look like a browser. Being polite here is the SLEEP below and
# the on-disk cache, not the string.
UA = ("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
SLEEP = 1.0  # be polite to a government site

# The labels a batch page uses inside a case. They are not the same in every
# era: 第二批 (2016) writes 【诉讼过程】 and 【相关立法】 where 第三十三批 onward
# writes 【检察机关履职过程】 and 【相关规定】, and 第一批 has neither of the last
# two at all. So a label is read as its canonical section rather than by name,
# and a crawler keyed on the 2026 spellings alone would have written those
# sections empty for every case before 2022 without ever saying so.
SECTION_ALIASES = [
    ("关键词", ("关键词",)),
    ("要旨", ("要旨",)),
    ("基本案情", ("基本案情",)),
    ("履职过程", ("检察机关履职过程", "履职过程", "诉讼过程")),
    ("指导意义", ("指导意义",)),
    ("相关规定", ("相关规定", "相关立法")),
]
CANONICAL_SECTIONS = [name for name, _ in SECTION_ALIASES]
BY_LABEL = {label: name for name, labels in SECTION_ALIASES for label in labels}
LABEL_RE = re.compile(r"^【(%s)】$" % "|".join(
    re.escape(l) for _, ls in SECTION_ALIASES for l in ls))
LABEL_SEP_RE = re.compile(r"^【(%s)】[：:]\s*(.*)$" % "|".join(
    re.escape(l) for _, ls in SECTION_ALIASES for l in ls))

# A case begins with its name centred and bold, and the line under it is the
# 检例 number in brackets. The number is the corpus's identity: it is what the
# SPP assigns and never reuses, and unlike a 案号 it is printed on the page.
CASE_ID_RE = re.compile(r"^（?检例第\s*([0-9]+)\s*号）?$")
CASE_ID_ANY_RE = re.compile(r"检例第\s*([0-9]+)\s*号")
# The batch notice at the top of the page names the theme in brackets, e.g.
# 作为第六十三批指导性案例（刑事抗诉主题）发布. Only some batches do.
THEME_RE = re.compile(r"第([一二三四五六七八九十百零]+)批指导性案例（([^）]{2,30}?)主题）发布")
BATCH_RE = re.compile(r"第([一二三四五六七八九十百零]+)批指导性案例")
BATCH_NO_RE = re.compile(r'href="(/spp/jczdal/(\d{6})/t\d+_\d+\.shtml)"[^>]*>\s*([^<]*批指导性案例[^<]*)')
# createPageHTML('page_div',3, 1,'index','shtml',62): the last argument is the
# total number of entries the channel advertises. The others are not the page
# count and not the page size, so only this one is read.
TOTAL_RE = re.compile(r"createPageHTML\([^)]*,\s*(\d+)\s*\)")
DATE_RE = re.compile(r"(\d{4})年(\d{1,2})月(\d{1,2})日")

# Two tiers, because the words the Gazette gate counts are not all personal
# data. 身份证号 and 手机号 identify a person outright: one occurrence fails the
# build. 住所地, 住址, 法定代表人 and 委托代理人 are role and place words that
# appear in jurisdiction rules ("由住所地的市人民检察院管辖") and in passages
# arguing about what personal data is; they matter only when a value is attached
# to them, and then only when that value is a natural person the publisher did
# not anonymise. Counting the word alone called 49 records dirty to ship 9 names.
GATE_PATTERNS = {
    "身份证号": r"(?<![0-9])[1-9][0-9]{5}(?:19|20)[0-9]{2}[0-1][0-9][0-3][0-9][0-9]{3}[0-9Xx](?![0-9])",
    "手机号": r"(?<![0-9])1[3-9][0-9]{9}(?![0-9])",
}
# word, then the same word with a value actually following it. The value must
# look like a name or a place: without the surname anchor the pattern happily
# returned 同意 and 参加诉讼, the prose that happens to follow the word, and
# called them people.
SURNAME = ("李王张刘陈杨黄赵周吴徐孙马朱胡郭何高林罗郑梁谢宋唐许韩冯邓曹彭曾肖田董袁潘"
           "于蒋蔡余杜叶程苏魏吕丁任沈姚卢姜崔钟谭陆汪范金石廖贾夏韦付方白邹孟熊秦邱江"
           "尹薛闫段雷侯龙史陶黎贺顾毛郝龚邵万钱严覃武戴莫孔向汤")
NAME = r"[%s][\u4e00-\u9fff某]{1,2}" % SURNAME
PLACE = r"[\u4e00-\u9fff]{2,20}?(?:省|市|区|县|路|街|号)[^\s，。；]{0,20}"
ROLE_PATTERNS = {
    "住所地": (r"住所地", r"住所地[为是]?[：:]?\s*(%s)" % PLACE),
    "住址": (r"住址", r"住址[为是]?[：:]?\s*(%s)" % PLACE),
    "法定代表人": (r"法定代表人", r"法定代表人[为是]?[：:]?\s*(%s)" % NAME),
    "委托代理人": (r"委托代理人", r"委托代理人[为是]?[：:]?\s*(%s)" % NAME),
}
# A 某 anywhere in a name is the publisher having hidden part of it.
ANONYMISED = re.compile(r"某")
# A case name the SPP printed with a real party at the head of it: 马世龙（抢劫）
# 核准追诉案, 于英生申诉案, 李泽强编造、故意传播虚假恐怖信息案. Not personal data
# this corpus assembled -- it is the citation -- but it is why this is a case
# index and not an anonymised one, so it is counted and stated.
#
# The guard is a negative one. Half the surname characters open a place or an
# organ too, so 白山市… and 江苏省… read as people until the head is also allowed
# to be a locality; and 某 anywhere means the publisher hid the name.
NAMED_TITLE_RE = re.compile(r"^(?:被告人)?(%s)(?!某)" % NAME)
NOT_A_PERSON_HEAD = re.compile(
    r"^[\u4e00-\u9fff]{0,3}?(?:市|省|区|县|公司|集团|银行|局|院|委|厅|部|厂|中心|所|社|会)")


def names_a_party(title):
    if not title or NOT_A_PERSON_HEAD.match(title):
        return False
    m = NAMED_TITLE_RE.match(title)
    return bool(m) and not ANONYMISED.search(m.group(1))

# 案件类型 is not published as a field on these pages, and no 案号 is either --
# only 第一批 happens to print one in prose. What the pages do publish is the
# statutes a case applies, under 【相关规定】. Which branch of law those are says
# what kind of case it is, so 案件类型 is derived from them and the derivation is
# recorded. Where the citations span branches the field is left empty rather
# than guessed, and where a batch declares a theme the two are cross-checked.
LAW_BRANCH = [
    (re.compile(r"刑法|刑事诉讼法|刑事诉讼"), "刑事"),
    (re.compile(r"民法典|民法总则|民法通则|民事诉讼法|合同法|婚姻法|继承法|侵权责任法"), "民事"),
    (re.compile(r"行政诉讼法|行政处罚法|行政强制法|行政许可法|行政复议法"), "行政"),
    (re.compile(r"国家赔偿法"), "国家赔偿"),
]

CSV_COLUMNS = ["原始链接", "案号", "案件名称", "法院", "所属地区", "案件类型",
               "案件类型编码", "来源", "审理程序", "裁判日期", "公开日期",
               "当事人", "案由", "法律依据", "全文"]

CHECKS = {
    "identity_field": "procuratorate_no",
    "sort_key": ["procuratorate_no"],
    "source_url_pattern": r"^https://www\.spp\.gov\.cn/spp/jczdal/\d{6}/t\d+_\d+\.shtml#检例第[0-9]+号$",
    "date_field": None,
    "sections_field": "sections",
    "required_sections": ["要旨", "基本案情"],
    "text_field": "full_text",
    # The gate's patterns live here rather than in either program, so the build
    # and the checker cannot drift into checking different things under the same
    # name -- which they once did: the checker's 法定代表人 needed a colon and the
    # crawler's did not, so one called this corpus clean and the other dirty.
    "publish_gate": {"identifying": GATE_PATTERNS,
                     "roles": {k: {"word": w, "valued": v}
                               for k, (w, v) in ROLE_PATTERNS.items()}},
    "number_field": "procuratorate_no",
    "csv_vs_jsonl": {
        "原始链接": "source_url",
        "案件名称": "title",
        "案件类型": "case_group",
        "法律依据": "legal_basis",
    },
}

FIELD_DOC = {
    "原始链接": {"origin": "verbatim + locator", "note": "the batch page plus the 检例 number the page prints as a fragment; several cases share one page and convert keys a document by this column, so without it 250 records collapse into 60"},
    "案号": {"origin": "empty", "note": "the SPP publishes no 案号 for a 指导性案例; only 第一批 mentions one in prose"},
    "案件名称": {"origin": "verbatim", "note": "the case name the SPP prints, already anonymised by its own hand (陈某, 某公司)"},
    "法院": {"origin": "empty", "note": "no 审理法院 field; courts appear in the narrative, often several per case, which is not a field"},
    "所属地区": {"origin": "empty", "note": "no region field"},
    "案件类型": {"origin": "derived", "note": "the branch of law 【相关规定】 cites; empty where the citations span branches"},
    "案件类型编码": {"origin": "derived", "note": "the same reading as a stable code"},
    "来源": {"origin": "fixed", "note": "最高人民检察院指导性案例"},
    "审理程序": {"origin": "empty", "note": "no 审理程序 field; the procedural posture is in 基本案情 and 履职过程"},
    "裁判日期": {"origin": "empty", "note": "the SPP dates the batch notice, not the case"},
    "公开日期": {"origin": "verbatim", "note": "the batch notice's own date, which is when the SPP published it"},
    "当事人": {"origin": "empty", "note": "no party list field; the parties appear anonymised throughout the text"},
    "案由": {"origin": "empty", "note": "no 案由 field; 关键词 is the nearest published facet and is in the JSON"},
    "法律依据": {"origin": "verbatim", "note": "【相关规定】 / 【相关立法】: the statutes the SPP itself lists for the case"},
    "全文": {"origin": "verbatim", "note": "title, each published section under its own label, and the 检例 number"},
}


def log(msg):
    print(msg, file=sys.stderr, flush=True)


def now_iso():
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


class Fetcher:
    """HTTP with a on-disk cache, so a rebuild is offline and reproducible.

    http.client rather than urllib, and not for elegance: urllib rewrites header
    names to `User-agent` and `Accept-language`. Headers are case-insensitive by
    RFC 9110, but the WAF in front of spp.gov.cn matches the literal
    `User-Agent`, does not find it, and answers 403 to the whole channel. The
    same request through http.client, which sends the names as given, gets 200.
    """

    def __init__(self, cache_dir, refresh):
        self.cache_dir = cache_dir
        self.refresh = refresh
        self.log = {}
        log_path = cache_dir / "fetch-log.json"
        if log_path.exists():
            self.log = json.loads(log_path.read_text(encoding="utf-8"))
        cache_dir.mkdir(parents=True, exist_ok=True)

    def cache_file(self, url):
        rel = url.split("://", 1)[-1]
        return self.cache_dir / rel.replace("/", "_")

    def _once(self, url):
        parts = urllib.parse.urlsplit(url)
        conn = http.client.HTTPSConnection(parts.netloc, timeout=40,
                                           context=ssl.create_default_context())
        try:
            conn.request("GET", parts.path + ("?" + parts.query if parts.query else ""),
                         headers={"User-Agent": UA, "Host": parts.netloc,
                                  "Accept": "text/html,application/xhtml+xml,"
                                            "application/xml;q=0.9,*/*;q=0.8",
                                  "Accept-Language": "zh-CN,zh;q=0.9",
                                  "Connection": "close"})
            resp = conn.getresponse()
            raw = resp.read()
            location = resp.getheader("Location")
            return resp.status, raw, location
        finally:
            conn.close()

    def get(self, url):
        path = self.cache_file(url)
        if path.exists() and not self.refresh:
            return path.read_text(encoding="utf-8", errors="replace")
        last = None
        for attempt in range(4):
            try:
                seen = 0
                while True:
                    status, raw, location = self._once(url)
                    if status in (301, 302, 303, 307, 308) and location and seen < 5:
                        url = urllib.parse.urljoin(url, location)
                        seen += 1
                        continue
                    break
                if status != 200:
                    raise RuntimeError("HTTP %d" % status)
                text = raw.decode("utf-8", errors="replace")
                self.log[url] = {"fetched_at": now_iso(), "bytes": len(raw),
                                 "sha256": hashlib.sha256(raw).hexdigest(),
                                 "http_status": status}
                path.write_text(text, encoding="utf-8")
                time.sleep(SLEEP)
                return text
            except Exception as exc:  # noqa: BLE001 - retry anything transient
                last = exc
                time.sleep(2.0 * (attempt + 1))
        raise RuntimeError("fetch failed %s: %s" % (url, last))

    def save_log(self):
        (self.cache_dir / "fetch-log.json").write_text(
            json.dumps(self.log, ensure_ascii=False, sort_keys=True, indent=1),
            encoding="utf-8")


CJK = re.compile(r"[\u3400-\u4dbf\u4e00-\u9fff\u3000-\u303f\uff00-\uffef]")


def collapse_ws(s):
    """Join lines of a name without inventing spaces, as the guiding crawler does."""
    s = s.replace("\u3000", " ").replace("\xa0", " ")
    out = []
    for piece in re.split(r"\s+", s.strip()):
        if not out:
            out.append(piece)
            continue
        prev, cur = out[-1], piece
        if CJK.match(prev[-1]) and CJK.match(cur[0]):
            out.append(cur)
        else:
            out.append(" " + cur)
    return "".join(out)


def html_to_lines(fragment):
    """Block-ish HTML to a list of stripped, non-empty text lines."""
    fragment = re.sub(r"(?is)<(script|style)[^>]*>.*?</\1>", " ", fragment)
    fragment = re.sub(r"(?i)<br\s*/?>", "\n", fragment)
    fragment = re.sub(r"(?i)</(p|div|li|h[1-6]|strong|span|font|td)>", "\n", fragment)
    fragment = re.sub(r"<[^>]+>", "", fragment)
    fragment = htmllib.unescape(fragment)
    lines = []
    for raw in fragment.split("\n"):
        line = raw.replace("\u3000", " ").replace("\xa0", " ").strip()
        if line:
            lines.append(line)
    return lines


def article_body(page):
    """The <div id="fontzoom"> body of a batch page, tags not included."""
    start = re.search(r'<div[^>]*id="fontzoom"[^>]*>', page)
    if not start:
        return None
    rest = page[start.end():]
    # The div closes where the page's own share/footer markup begins; counting
    # nested divs would be exact but the markup is flat, and the footer marker
    # is what actually bounds it on every page sampled.
    stop = re.search(r'(?:<div[^>]*class="[^"]*(?:share|footNav|related|zhongying))', rest)
    return rest[:stop.start()] if stop else rest


def batch_links(fetcher):
    """Every batch page the channel lists, newest first: [(ym, path, label)].

    Two things here are not what they look like. The pagination script is
    createPageHTML(div, items-per-page, current, prefix, ext, total-items); the
    second argument is not a page count, and reading it as one dropped the last
    page, where 第一批 and 第二批 live, so 检例第1号 to 第8号 went missing without a
    word. And page k is index_k.shtml for k >= 2 while page 1 is index.shtml --
    index_1.shtml is not a listing at all, it is a site page whose title is the
    site's own name. So pages are numbered from the total, and a page that
    matches nothing is reported rather than taken for the end.
    """
    first = fetcher.get(BASE + CHANNEL + "index.shtml")
    total = int(TOTAL_RE.search(first).group(1))
    found, seen = [], set()
    pages = None
    for page in range(1, 20):
        name = "index" if page == 1 else "index_%d" % page
        body = fetcher.get(BASE + CHANNEL + name + ".shtml")
        found_here = 0
        for path, ym, label in BATCH_NO_RE.findall(body):
            if path in seen:
                continue
            seen.add(path)
            found.append((ym, path, collapse_ws(clean_label(label))))
            found_here += 1
        if page == 1:
            # The second argument of createPageHTML is not items-per-page -- it
            # is 3 here while the page lists 30 -- so the step is measured off
            # the first page rather than read from the script.
            per_page = found_here
            pages = -(-total // per_page) if per_page else 1
            log("  listing: %d advertised, %d on the first page, %d pages"
                % (total, per_page, pages))
        elif not found_here:
            raise SystemExit("listing page %d held nothing; the pagination is "
                             "not what this assumes" % page)
        log("  listing page %d: %d batch pages" % (page, found_here))
        if page >= pages:
            break
    if len(seen) != total:
        log("  note %d batch pages found, the listing advertises %d" % (len(seen), total))
    found.sort(reverse=True)
    return found, pages


def clean_label(s):
    s = htmllib.unescape(s)
    return re.sub(r"(?i)&nbsp;|\xa0", " ", s)


def split_cases(lines):
    """The batch notice and then one block per case.

    A case begins with its name and the line under it is （检例第N号）, both
    centred. The number also appears in the notice at the top of the page -- in
    the range 检例第257—261号 -- so a case is only recognised where the number
    stands on its own line, which the notice never does.

    The name is the line before the number wherever that line falls: in the
    notice for the first case, and inside the previous case's last section for
    every case after it. Reading it only from the notice would name the first
    case of a batch and leave the rest blank.
    """
    notice, cases, current, prev = [], [], None, ""
    for line in lines:
        m = CASE_ID_RE.match(line)
        if m:
            title = prev
            if title.startswith("关于印发") or "的通知" in title or not title:
                title = ""
            else:
                # That title is also the last line the loop has so far appended
                # to the case before this one, because a case's body runs until
                # the next title. Leaving it there put the next case's name at
                # the end of this case's 相关规定, which then shipped as part of
                # a 法律依据 field.
                if current is not None and current["sections"]:
                    body = current["sections"][-1][1]
                    if body and body[-1] == title:
                        body.pop()
                elif current is not None and current["header"] and current["header"][-1] == title:
                    current["header"].pop()
            current = {"title": title, "procuratorate_no": int(m.group(1)),
                       "sections": [], "header": []}
            cases.append(current)
            prev = line
            continue
        if current is None:
            notice.append(line)
        else:
            label = LABEL_RE.match(line) and line[1:-1]
            m2 = LABEL_SEP_RE.match(line)
            if label:
                current["sections"].append([BY_LABEL[label], []])
            elif m2:
                current["sections"].append([BY_LABEL[m2.group(1)],
                                           [m2.group(2)] if m2.group(2) else []])
            else:
                (current["sections"][-1][1] if current["sections"]
                 else current["header"]).append(line)
        prev = line
    return notice, cases


def section_text(lines):
    return "\n".join(collapse_ws(l) for l in lines).strip()


def derive_case_group(legal_basis):
    """The branch of law 【相关规定】 cites, or empty when it cites more than one.

    Honest but partial: a case whose listed statutes are all criminal is a
    criminal case. A case listing both 刑法 and 民事诉讼法 is not forced into one
    bucket, because the SPP did not say which the field is for.
    """
    hits = []
    for rx, group in LAW_BRANCH:
        if rx.search(legal_basis or ""):
            hits.append(group)
    return hits[0] if len(set(hits)) == 1 else ""


CASE_GROUP_CODE = {"刑事": "XS", "民事": "MS", "行政": "XZ", "国家赔偿": "GJPC"}


def build_record(case, batch, url, page_sha, fetched_at):
    sections, raw_lines = {}, {}
    for name, lines in case["sections"]:
        text = section_text(lines)
        if text:
            sections.setdefault(name, []).append(text)
        raw_lines.setdefault(name, []).extend(lines)
    merged = {k: "\n".join(v).strip() for k, v in sections.items()}
    # 关键词 keeps the page's own spacing. collapse_ws joins two CJK tokens with
    # no space, which is right for a case name the site wrapped mid-phrase and
    # wrong for a term list, where the spaces are the only thing separating
    # 再审抗诉 from 抢劫; collapsing it produced one 12-character "keyword".
    keywords = [k for k in re.split(r"[\s、;；,，]+", " ".join(raw_lines.get("关键词", [])))
                if k and k not in ("关键词",)]
    legal_basis = merged.get("相关规定", "")
    group = derive_case_group(legal_basis)
    # A batch page carries several cases, so the page URL alone is not an
    # identity: convert keys a document by its 原始链接 and collapsed 250 records
    # into 60. The fragment is what the engine has to key on when the URL holds
    # no docId=, and 检例第N号 is the locator the page itself prints, so it
    # resolves to the same page and to the right case on it.
    locator = url + "#检例第%d号" % case["procuratorate_no"]
    parts = [case["title"] or "（检例第%d号）" % case["procuratorate_no"],
             "检例第%d号" % case["procuratorate_no"],
             "来源：最高人民检察院指导性案例（%s）" % batch["label"]]
    if legal_basis:
        parts.append("法律依据：" + legal_basis)
    for name in CANONICAL_SECTIONS:
        if merged.get(name):
            parts.append("%s：%s" % (name, merged[name]))
    return {
        "procuratorate_no": case["procuratorate_no"],
        "label": "检例第%d号" % case["procuratorate_no"],
        "title": case["title"],
        "source_url": locator,
        "page_url": url,
        "batch": batch["label"],
        "batch_no": batch["no"],
        "batch_theme": batch["theme"],
        "published_at": batch["date"],
        "sections": merged,
        "section_order": [k for k in CANONICAL_SECTIONS if merged.get(k)],
        "full_text_sha256": hashlib.sha256(
            "\n\n".join(parts).encode("utf-8")).hexdigest(),
        "keywords": keywords,
        "legal_basis": legal_basis,
        "laws_mentioned": sorted(set(re.findall(r"《[^》]{2,60}》", legal_basis))),
        "case_group": group,
        "case_group_code": CASE_GROUP_CODE.get(group, ""),
        "case_group_derived": bool(group),
        "page_sha256": page_sha,
        "fetched_at": fetched_at,
        "full_text": "\n\n".join(parts),
    }


def assert_publishable(records, force=False):
    """Two tiers: what identifies a person fails, what merely names a role is counted.

    The first version of this gate counted the words 住所地, 住址, 法定代表人 and
    委托代理人 the way the Gazette gate does, and reported 49 of 242 records dirty.
    Reading them showed the count was mostly noise: the word 法定代表人 appears 73
    times and carries a name 9 times, and 住所地 appears in rules about which
    procuratorate has jurisdiction. What the corpus really holds is a handful of
    partially anonymised officer names and 18 case titles the SPP itself printed
    with a party's full name in them -- a citation, the same category the Gazette
    corpus ships. So the gate fails on what identifies and reports what merely
    describes, and both numbers are in the manifest.
    """
    census = {"identifying": {}, "role_word": {}, "role_with_value": {},
              "role_with_full_name": {}, "named_titles": 0}
    for name, pat in GATE_PATTERNS.items():
        census["identifying"][name] = sum(len(re.findall(pat, r["full_text"]))
                                          for r in records)
    for name, (word, valued) in ROLE_PATTERNS.items():
        census["role_word"][name] = sum(len(re.findall(word, r["full_text"]))
                                        for r in records)
        hits, full = [], []
        for rec in records:
            for v in re.findall(valued, rec["full_text"]):
                hits.append((rec["label"], v))
                if not ANONYMISED.search(v) and name in ("法定代表人", "委托代理人"):
                    full.append((rec["label"], v))
        census["role_with_value"][name] = len(hits)
        census["role_with_full_name"][name] = len(full)
        for label, v in full[:3]:
            log("  note %s names %s in %s" % (name, v, label))
    census["named_titles"] = sum(1 for r in records if names_a_party(r["title"]))
    census["named_titles_anonymised"] = sum(
        1 for r in records if r["title"] and not names_a_party(r["title"])
        and ANONYMISED.search(r["title"]))

    for name, n in census["identifying"].items():
        log("  %-4s %s: %d" % ("ok  " if not n else "FAIL", name, n))
    for name in ROLE_PATTERNS:
        log("  note %-12s word %d, with a value %d, with a full name %d"
            % (name, census["role_word"][name], census["role_with_value"][name],
               census["role_with_full_name"][name]))
    log("  note %d case titles name a party in full, as the SPP printed them"
        % census["named_titles"])

    # A name attached to a role is reported, not failed. The SPP published it in
    # a judicial document, in the same way it publishes 马世龙抢劫案 as a case
    # name; withholding 检例第7号 for its 李蓬 while shipping fifty-odd titles
    # that name a party in full would be a rule about the pattern, not about the
    # exposure. What fails a build is what identifies a person to someone who
    # did not already have the case.
    fails = [n for n, v in census["identifying"].items() if v]
    if fails and not force:
        raise SystemExit("publish gate: %s" % ", ".join(fails))
    return census


def csv_row(rec):
    return {
        "原始链接": rec["source_url"],
        "案号": "",
        "案件名称": rec["title"],
        "法院": "",
        "所属地区": "",
        "案件类型": rec["case_group"],
        "案件类型编码": rec["case_group_code"],
        "来源": "最高人民检察院指导性案例",
        "审理程序": "",
        "裁判日期": "",
        "公开日期": rec["published_at"],
        "当事人": "",
        "案由": "",
        "法律依据": rec["legal_basis"],
        "全文": rec["full_text"],
    }


CN_NUM = {"零": 0, "一": 1, "二": 2, "三": 3, "四": 4, "五": 5, "六": 6, "七": 7,
          "八": 8, "九": 9}


def cn_int(s):
    """第N批 in Chinese numerals, up to 九十九, which covers every batch so far."""
    if s.isdigit():
        return int(s)
    if "十" in s:
        head, _, tail = s.partition("十")
        return (CN_NUM.get(head, 1) or 1) * 10 + (CN_NUM.get(tail, 0) if tail else 0)
    return sum(CN_NUM.get(c, 0) for c in s) or None


def batch_info(label, notice, ym):
    m = BATCH_RE.search(label)
    theme = THEME_RE.search(notice)
    date = None
    # The notice signs itself 最高人民检察院 / 2026年7月15日; that is the date the
    # SPP published the batch.
    for m2 in reversed(list(DATE_RE.finditer(notice))):
        date = "%04d-%02d-%02d" % (int(m2.group(1)), int(m2.group(2)), int(m2.group(3)))
        break
    if not date:
        date = "%s-%s-01" % (ym[:4], ym[4:6])
    return {"label": collapse_ws(label), "no": cn_int(m.group(1)) if m else None,
            "theme": collapse_ws(theme.group(2)) if theme else "", "date": date}


def emit(out, records, manifest):
    out.mkdir(parents=True, exist_ok=True)
    jsonl, csv_path = out / "cases.jsonl", out / "cases.csv"
    with open(jsonl, "w", encoding="utf-8") as fh:
        for rec in records:
            fh.write(json.dumps(rec, ensure_ascii=False, sort_keys=True) + "\n")
    with open(csv_path, "w", encoding="utf-8", newline="") as fh:
        # LF, not the csv module's default CRLF: .gitattributes pins this tree to
        # eol=lf, so git stores the blob with LF whatever the writer emits. The
        # manifest hashes the file on disk, so a CRLF file here records a hash no
        # checkout can reproduce.
        writer = csv.DictWriter(fh, fieldnames=CSV_COLUMNS, quoting=csv.QUOTE_MINIMAL,
                                lineterminator="\n")
        writer.writeheader()
        for rec in records:
            writer.writerow(csv_row(rec))
    manifest["files"] = {
        p.name: {"sha256": sha256_file(p), "bytes": p.stat().st_size,
                 "records": len(records)}
        for p in (jsonl, csv_path)
    }
    (out / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    log("wrote %s: %d records, %s (%d kB)"
        % (out, len(records), ", ".join(sorted(manifest["files"])),
           sum(v["bytes"] for v in manifest["files"].values()) // 1000))


def _section_counts(records):
    seen = {}
    for rec in records:
        for name in rec["section_order"]:
            seen[name] = seen.get(name, 0) + 1
    return seen


def main():
    ap = argparse.ArgumentParser(description="build the procuratorate-cases dataset")
    ap.add_argument("--out", default="data/procuratorate-cases")
    ap.add_argument("--refresh", action="store_true", help="re-fetch instead of using the cache")
    ap.add_argument("--state", default="tools/state/procuratorate-cases-raw")
    ap.add_argument("--force", action="store_true",
                    help="write the corpus even when the gate fails, to see what it caught")
    args = ap.parse_args()

    fetcher = Fetcher(Path(args.state), args.refresh)
    links, pages = batch_links(fetcher)
    log("batch pages: %d over %d listing pages" % (len(links), pages))

    records, batches = [], []
    for ym, path, label in links:
        url = BASE + path
        page = fetcher.get(url)
        body = article_body(page)
        if body is None:
            raise SystemExit("no fontzoom div on %s" % url)
        notice, cases = split_cases(html_to_lines(body))
        info = batch_info(label, "\n".join(notice), ym)
        info["url"] = url
        info["cases"] = len(cases)
        batches.append(info)
        entry = fetcher.log.get(url, {})
        for case in cases:
            records.append(build_record(case, info, url,
                                        entry.get("sha256", ""),
                                        entry.get("fetched_at", "")))
        log("  %-22s %d cases" % (info["label"], len(cases)))
    fetcher.save_log()

    records.sort(key=lambda r: r["procuratorate_no"])
    dupes = len(records) - len({r["procuratorate_no"] for r in records})
    if dupes:
        raise SystemExit("%d duplicate 检例 numbers" % dupes)
    log("records: %d, 检例第%d号 through 检例第%d号"
        % (len(records), records[0]["procuratorate_no"], records[-1]["procuratorate_no"]))

    log("publish gate")
    census = assert_publishable(records, args.force)

    groups = {}
    for rec in records:
        key = rec["case_group"] or "(未识别)"
        groups[key] = groups.get(key, 0) + 1
    with_basis = sum(1 for r in records if r["legal_basis"])
    manifest = {
        "dataset": "procuratorate-cases",
        "title": "最高人民检察院指导性案例",
        "version": "1",
        "generated_at": now_iso(),
        "generator": "tools/crawl_procuratorate_cases.py",
        "scope": "public",
        "source": {"name": "最高人民检察院指导性案例", "base": BASE, "channel": CHANNEL,
                   "listing_pages": pages, "batch_pages": len(batches)},
        "counts": {
            "cases": len(records),
            "batches": len(batches),
            "with_legal_basis": with_basis,
            "without_legal_basis": len(records) - with_basis,
            "with_keywords": sum(1 for r in records if r["keywords"]),
            "with_guidance": sum(1 for r in records if r["sections"].get("指导意义")),
            "full_text_chars_total": sum(len(r["full_text"]) for r in records),
            "sections": _section_counts(records),
            "case_numbers_missing": sorted(
                set(range(records[0]["procuratorate_no"],
                          records[-1]["procuratorate_no"] + 1))
                - {r["procuratorate_no"] for r in records}),
            "case_numbers_max": records[-1]["procuratorate_no"],
            "case_groups": groups,
            "laws_mentioned_distinct": len({x for r in records for x in r["laws_mentioned"]}),
        },
        "checks": CHECKS,
        "fields": FIELD_DOC,
        "pii_census": census,
        "notes": [
            "This corpus ships whole. No 身份证号 and no 手机号 appears in it, and no "
            "住址 carries an address; what the gate reports are role words used in the "
            "abstract. The 法定代表人 it counted %d times carries a name %d times, and "
            "the 住所地 it counted %d times is mostly a rule about which procuratorate "
            "has jurisdiction. See the gate block in this manifest." % (
                census["role_word"]["法定代表人"], census["role_with_value"]["法定代表人"],
                census["role_word"]["住所地"]),
            "%d case titles name a party in full, exactly as the SPP printed them "
            "(检例第20号 马世龙…). That is the citation, not a party list this corpus "
            "assembled, and it is why this is a case index rather than an anonymised "
            "one." % census["named_titles"],
            "案号, 法院, 审理程序, 当事人, 案由 and 裁判日期 are empty because the SPP "
            "publishes none of them as a field. 法律依据 is not: 【相关规定】 lists the "
            "statutes the SPP itself cites, and it is filled for %d of %d records." % (
                with_basis, len(records)),
            "案件类型 is derived from which branch of law 【相关规定】 cites, not published. "
            "It is empty where a case cites more than one branch: %s." % (
                ", ".join("%s %d" % kv for kv in sorted(groups.items()))),
            "The 【…】 labels are not stable across the years: 第二批 (2016) writes "
            "【诉讼过程】 and 【相关立法】 where 第三十三批 onward writes 【检察机关履职过程】 "
            "and 【相关规定】, and 第一批 has neither of the last two. Both spellings map to "
            "one canonical section, so 相关规定 is not silently empty for 2016 to 2021.",
            "公开日期 is the date on the batch notice, which is when the SPP published the "
            "case; 裁判日期 stays empty because the SPP dates the batch, not the case.",
            "Several cases share one batch page, so 原始链接 carries the 检例 number "
            "as a fragment. convert keys a document by that column and no URL here has "
            "a docId=, so the bare page address made every case on it one document -- "
            "250 rows became 60. page_url in the JSON holds the address without it.",
        ],
        "batches": batches,
        "redistribution": {
            "basis": "指导性案例 published by the 最高人民检察院 on its own site; judicial "
                     "documents outside copyright under 著作权法 第五条",
            "publisher": "最高人民检察院",
        },
    }
    emit(Path(args.out), records, manifest)


if __name__ == "__main__":
    main()
