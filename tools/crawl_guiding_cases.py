#!/usr/bin/env python3
"""Build the bundled `guiding-cases` dataset from the SPC's own pages.

The Supreme People's Court publishes its guiding cases (指导性案例) as plain
web pages under /shenpan/gengduo/77.html (15 listing pages) and
/shenpan/xiangqing/<id>.html (one page per case, plus the batch notices). This
script reads those pages, splits every case page into its published sections,
and writes a small corpus in the shape `convert` imports:

    python3 tools/crawl_guiding_cases.py                     # full crawl
    python3 tools/crawl_guiding_cases.py --out /tmp/gc       # elsewhere

Fetched HTML is cached under tools/state/guiding-cases-raw (gitignored), so a
re-run is offline and byte-stable; --refresh forces a live crawl. The cache and
the fetch log are what make the dataset reproducible: manifest.json records the
URL, fetch time and content hash of every page it was built from.

stdlib only, like the other tools here.
"""

import argparse
import csv
import hashlib
import html as htmllib
import http.cookiejar
import json
import re
import sys
import time
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

BASE = "https://www.court.gov.cn"
LIST_PATH = "/shenpan/gengduo/77{page}.html"
UA = "Mozilla/5.0 (X11; Linux x86_64) LocalCases dataset builder"
SLEEP = 1.0  # be polite to a government site

# The labels the SPC uses inside a guiding-case page. Order varies between
# pages (older ones put 相关法条 before 基本案情), so sections are keyed by
# label rather than by position.
SECTION_LABELS = [
    "关键词",
    "裁判要点",
    "执行实施要点",
    "裁判理由",
    "裁判结果",
    "基本案情",
    "相关法条",
    "诉辩主张",
    "法院经审理查明",
    "适用范围",
    "注意",
]
LABEL_RE = re.compile(r"^(%s)$" % "|".join(re.escape(x) for x in SECTION_LABELS))
# A label sharing a line with its value: either the page writes "相关法条："
# and the text follows on the same line, or (关键词 only) the term list starts
# immediately after the label with no separator at all.
LABEL_SEP_RE = re.compile(r"^(%s)[：:]\s*(.*)$" % "|".join(re.escape(x) for x in SECTION_LABELS))
KEYWORDS_INLINE_RE = re.compile(r"^(关键词)\s*(\S.*)$")

# 关键词 opens with the family of the case; the older pages write 民事诉讼 /
# 行政诉讼 / 刑事诉讼 where the newer ones write 民事 / 行政 / 刑事, and one page
# (指导案例40号) runs the family and the next term together.
CASE_GROUP_ALIASES = {"民事诉讼": "民事", "刑事诉讼": "刑事", "行政诉讼": "行政"}
CASE_GROUP_FAMILIES = ("国家赔偿", "海事诉讼", "民事", "刑事", "行政", "执行")

CASE_LABEL_RE = re.compile(r"^(指导案例|指导性案例)\s*(\d+)\s*号[:：]?\s*(.*)$")
# （2018）粤73民初1099号 / (2009)虹民三（民）初字第912号
CASE_NO_RE = re.compile(r"[（(]\d{4}[）)][^\s，。；：]{1,30}?号")
# A court that names itself as the author of a decision: "…法院于YYYY年…".
# Anchoring on the date is what keeps prose such as "撤销上海市虹口区人民法院"
# out of the list.
COURT_RE = re.compile(
    r"[\u4e00-\u9fff]{2,20}?(?:人民法院|军事法院|海事法院|知识产权法院|金融法院|"
    r"互联网法院|铁路运输法院)(?=\s*于\s*\d{4}年)")
COURT_JUNK = ("撤销", "维持", "驳回", "依照", "按照", "责令", "确认", "变更", "上诉",
              "申请", "提审", "指令", "指定", "移送", "报送", "接受", "向", "由", "经",
              "至", "在", "与", "及", "或", "其", "该", "将", "对", "系", "被", "并",
              "均", "已", "从", "到")
BATCH_RE = re.compile(r"第\s*([0-9一二三四五六七八九十百]+)\s*批")
BATCH_RANGE_RE = re.compile(r"指导(?:性)?案例\s*(\d+)(?:\s*[-—至]\s*(\d+))?\s*号")
DOC_NO_RE = re.compile(r"^[法〔（(].{0,20}[〕）)]\s*\d{1,4}号$")

CN_DIGITS = {"一": 1, "二": 2, "三": 3, "四": 4, "五": 5, "六": 6, "七": 7,
             "八": 8, "九": 9, "十": 10, "十一": 11, "十二": 12, "十三": 13,
             "十四": 14, "十五": 15, "十六": 16, "十七": 17, "十八": 18,
             "十九": 19, "二十": 20, "二十一": 21, "二十二": 22, "二十三": 23,
             "二十四": 24, "二十五": 25, "二十六": 26, "二十七": 27, "二十八": 28,
             "二十九": 29, "三十": 30, "三十一": 31, "三十二": 32, "三十三": 33,
             "三十四": 34, "三十五": 35, "三十六": 36, "三十七": 37, "三十八": 38,
             "三十九": 39, "四十": 40, "四十一": 41, "四十二": 42, "四十三": 43,
             "四十四": 44, "四十五": 45, "四十六": 46, "四十七": 47, "四十八": 48,
             "四十九": 49}

# 案件类型编码, the same table extract.rs documents for the main corpus.
CASE_TYPE_CODES = {"民事": "1", "执行": "2", "行政": "3", "刑事": "4",
                   "管辖": "5", "非诉保全审查": "8"}

CSV_COLUMNS = ["原始链接", "案号", "案件名称", "法院", "所属地区", "案件类型",
               "案件类型编码", "来源", "审理程序", "裁判日期", "公开日期",
               "当事人", "案由", "法律依据", "全文"]

# Cases the SPC announced but never published a page for in the 指导案例
# listing. The 第10批 notice announces 指导案例45-52号; the listing carries
# 46-52 but no page for 45号 (checked: no xiangqing page exists for it), so
# its published text is taken verbatim from a court-run mirror that reprints
# the SPC's guiding-case page in full.
SUPPLEMENTS = [
    {
        "case_no": 45,
        "title": "指导案例45号：北京百度网讯科技有限公司诉青岛奥商网络技术有限公司等不正当竞争纠纷案",
        "url": "https://hnlyzy.hncourt.gov.cn/public/detail.php?id=6738",
        "list_date": "2015-04-23",  # the 第10批 notice's date in the listing
    },
]


def log(msg):
    print(msg, flush=True)


def now_iso():
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def sha256_text(s):
    return hashlib.sha256(s.encode("utf-8")).hexdigest()


def sha256_file(p):
    h = hashlib.sha256()
    with open(p, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()



class Fetcher:
    """HTTP with a on-disk cache, so a rebuild is offline and reproducible."""

    def __init__(self, cache_dir, refresh):
        self.cache_dir = cache_dir
        self.refresh = refresh
        self.jar = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(
            urllib.request.HTTPCookieProcessor(self.jar))
        self.log = {}
        log_path = cache_dir / "fetch-log.json"
        if log_path.exists():
            self.log = json.loads(log_path.read_text(encoding="utf-8"))
        cache_dir.mkdir(parents=True, exist_ok=True)

    def cache_file(self, url):
        rel = url.split("://", 1)[-1]
        return self.cache_dir / rel.replace("/", "_")

    def get(self, url):
        path = self.cache_file(url)
        if path.exists() and not self.refresh:
            return path.read_text(encoding="utf-8", errors="replace")
        last = None
        for attempt in range(4):
            try:
                req = urllib.request.Request(url, headers={"User-Agent": UA})
                with self.opener.open(req, timeout=40) as resp:
                    raw = resp.read()
                text = raw.decode("utf-8", errors="replace")
                self.log[url] = {"fetched_at": now_iso(), "bytes": len(raw),
                                 "sha256": hashlib.sha256(raw).hexdigest(),
                                 "http_status": resp.status}
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
    """Join lines of a title/heading without inventing spaces.

    The SPC wraps long case names mid-phrase, so a newline inside a name must
    vanish rather than become a space; a space between two Latin/ASCII tokens
    must survive.
    """
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
    """Block-ish HTML to a list of stripped, non-empty text lines.

    Whitespace inside a line is kept: in an older case page the 关键词 line is
    "关键词 民事 居间合同 二手房买卖 违约", and those spaces are the separators.
    Only the indentation full-width spaces are turned into ordinary ones.
    """
    fragment = re.sub(r"(?is)<(script|style)[^>]*>.*?</\1>", " ", fragment)
    fragment = re.sub(r"(?i)<br\s*/?>", "\n", fragment)
    fragment = re.sub(r"(?i)</(p|div|li|h[1-6]|tr|strong|span|font)>", "\n", fragment)
    fragment = re.sub(r"<[^>]+>", "", fragment)
    fragment = htmllib.unescape(fragment)
    lines = []
    for raw in fragment.split("\n"):
        line = raw.replace("\u3000", " ").replace("\xa0", " ").strip()
        if line:
            lines.append(line)
    return lines


def article_body(page):
    """The article div of a court.gov.cn detail page, tags not included."""
    start = re.search(r'<div[^>]*class="txt_txt"[^>]*>', page)
    if not start:
        return None
    end = re.search(r'<div[^>]*class="txt_etr"', page[start.end():])
    if end:
        return page[start.end():start.end() + end.start()]
    stop = page.find("<!--分享-->", start.end())
    return page[start.end():stop if stop > 0 else len(page)]


def mirror_article(page):
    """The article span of a SUPPLEMENTS mirror page, tags not included."""
    m = re.search(r'<span class="detail_content">(.*)', page, re.S)
    if not m:
        return None
    body = m.group(1)
    stop = re.search(r"责任编辑|〖关闭窗口〗", body)
    return body[:stop.start()] if stop else body


def clean_title(s):
    """Unescape a listing title, then drop the entities that were markup.

    The 第6批 and 第8批 entries write `&amp;nbsp;` inside the title attribute:
    one unescape turns that into the literal text `&nbsp;`, which is markup
    residue around the notice's name rather than part of it.
    """
    s = htmllib.unescape(s)
    return re.sub(r"(?i)&nbsp;|\xa0", " ", s)


def listing_items(page):
    """(title, path, list_date) of every entry on one listing page."""
    items = []
    for m in re.finditer(
        r'<a\s+title="([^"]*)"[^>]*href="(/shenpan/xiangqing/\d+\.html)"[^>]*>.*?</a>\s*'
        r'<i class="date">(\d{4}-\d{2}-\d{2})</i>',
        page,
        re.S,
    ):
        items.append({
            "title": collapse_ws(clean_title(m.group(1))),
            "path": m.group(2),
            "list_date": m.group(3),
        })
    return items


def last_listing_page(page):
    """The highest listing page number, read off the 尾页 link."""
    nums = [int(x) for x in re.findall(r"77_(\d+)\.html\">尾页", page)]
    return max(nums) if nums else 1


def split_sections(lines):
    """Split an article's lines into (label, [lines]) plus the leading header."""
    header, sections, current = [], [], None
    for line in lines:
        if LABEL_RE.match(line):
            current = line
            sections.append([current, []])
            continue
        m = LABEL_SEP_RE.match(line) or KEYWORDS_INLINE_RE.match(line)
        if m:
            current = m.group(1)
            sections.append([current, [m.group(2)] if m.group(2) else []])
            continue
        if current is None:
            header.append(line)
        else:
            sections[-1][1].append(line)
    return header, sections


def parse_case(lines, item):
    """A guiding-case page -> the structured record this dataset stores."""
    header, sections = split_sections(lines)
    label_no, label = None, ""
    for line in header:
        m = CASE_LABEL_RE.match(line)
        if m:
            label_no = int(m.group(2))
            label = "%s%s号" % (m.group(1), m.group(2))
            break
    publish_note = ""
    if label_no is None:
        m = CASE_LABEL_RE.match(item["title"])
        if m:
            label_no = int(m.group(2))
            label = "%s%s号" % (m.group(1), m.group(2))
    if label_no is None:
        raise ValueError("no case number on page: %s" % item["path"])
    title_parts = []
    for line in header:
        if not publish_note and line.startswith(("（", "(")) and "发布" in line:
            publish_note = line
            continue
        if label and line == label:
            continue
        title_parts.append(line)
    title = collapse_ws(" ".join(title_parts))
    sec_map = {}
    order = []
    for name, body in sections:
        text = "\n".join(body).strip()
        if name in sec_map:
            sec_map[name] = (sec_map[name] + "\n" + text).strip()
        else:
            sec_map[name] = text
            order.append(name)

    keywords = sec_map.get("关键词", "")
    group = ""
    if keywords:
        first = re.split(r"[/／\s]", keywords.strip(), maxsplit=1)[0].strip()
        group = CASE_GROUP_ALIASES.get(first, "")
        if not group:
            group = next((name for name in CASE_GROUP_FAMILIES
                          if first.startswith(name)), first)
    name_body = re.sub(r"^(指导案例|指导性案例)\s*\d+\s*号[:：]", "", item["title"])
    result_text = sec_map.get("裁判结果", "")
    numbers, seen_numbers = [], set()
    for cand in CASE_NO_RE.findall(result_text):
        cand = collapse_ws(cand)
        key = cand.replace("（", "(").replace("）", ")")
        if key in seen_numbers:
            continue
        seen_numbers.add(key)
        numbers.append(cand)
    courts, seen_courts = [], set()
    for cand in COURT_RE.findall(" ".join(sec_map.values())):
        while cand and any(cand.startswith(junk) for junk in COURT_JUNK):
            cand = next((cand[len(junk):] for junk in COURT_JUNK
                         if cand.startswith(junk)), cand)
        if len(cand) < 4:
            continue
        if cand in seen_courts:
            continue
        seen_courts.add(cand)
        courts.append(cand)
    return {
        "case_no": label_no,
        "case_no_label": label,
        "title": name_body,
        "title_full": item["title"],
        "publish_note": publish_note,
        "keywords": keywords,
        "case_group": group,
        "sections": sec_map,
        "section_order": order,
        "judgment_case_numbers": numbers,
        "courts_mentioned": courts,
    }


def parse_notice(lines, item):
    """A batch notice -> its own small record, used for batch attribution."""
    text = "\n".join(lines).strip()
    doc_no = ""
    for line in lines[:4]:
        if DOC_NO_RE.match(line):
            doc_no = line
            break
    batch = None
    m = BATCH_RE.search(text)
    if m:
        token = m.group(1)
        batch = int(token) if token.isdigit() else CN_DIGITS.get(token)
    ranges = []
    for m in BATCH_RANGE_RE.finditer(text):
        lo = int(m.group(1))
        ranges.append([lo, int(m.group(2) or lo)])
    return {
        "title": item["title"],
        "doc_no": doc_no,
        "batch": batch,
        "case_ranges": ranges,
        "publish_date": item["list_date"],
        "text": text,
    }


def compose_full_text(rec):
    """The 全文 column: the page's own text, section by section, no markup."""
    parts = [rec["title_full"]]
    if rec["publish_note"]:
        parts.append(rec["publish_note"])
    for name in rec["section_order"]:
        body = rec["sections"].get(name, "")
        if body:
            parts.append("%s\n%s" % (name, body))
    return "\n\n".join(parts)


def primary_case_number(rec):
    """The case number of the effective judgment, when the page names one.

    裁判结果 lists every instance in order. The effective one is the last
    number that a 判决 follows — a later 民事裁定 decides a retrial
    application, it is not the judgment the guiding case rests on.
    """
    result = rec["sections"].get("裁判结果", "")
    numbers = rec["judgment_case_numbers"]
    if not numbers:
        return ""
    for num in reversed(numbers):
        idx = result.rfind(num)
        if idx >= 0 and result[idx + len(num):idx + len(num) + 2].startswith("判决"):
            return num
    return numbers[-1]


def csv_row(rec):
    group = rec["case_group"]
    return {
        "原始链接": rec["source_url"],
        "案号": primary_case_number(rec),
        "案件名称": rec["title_full"],
        "法院": "",
        "所属地区": "",
        "案件类型": group,
        "案件类型编码": CASE_TYPE_CODES.get(group, ""),
        "来源": "最高人民法院指导性案例",
        "审理程序": "",
        "裁判日期": "",
        "公开日期": rec["publish_date"],
        "当事人": "",
        "案由": "",
        "法律依据": rec["sections"].get("相关法条", ""),
        "全文": rec["full_text"],
    }


# What each CSV column holds and where it came from: verbatim from the page,
# derived by this script under a documented rule, or deliberately empty because
# the publisher does not publish that fact for a guiding case.
FIELD_DOC = {
    "原始链接": {"origin": "verbatim", "note": "the court.gov.cn page the record was read from; 指导案例45号 has no such page, so its record names the court-run mirror it was read from instead"},
    "案号": {"origin": "derived", "note": "case number of the effective judgment, read out of 裁判结果; empty when the page names none"},
    "案件名称": {"origin": "verbatim", "note": "the published title, 指导性案例NNN号 prefix included; line wraps inside the title are joined"},
    "法院": {"origin": "empty", "note": "a guiding-case page names no single adjudicating court; the courts it does mention are in the JSON field courts_mentioned"},
    "所属地区": {"origin": "empty", "note": "not published for guiding cases"},
    "案件类型": {"origin": "derived", "note": "the first term of 关键词 (民事 / 刑事 / 行政 / 执行 / …)"},
    "案件类型编码": {"origin": "derived", "note": "the code extract.rs documents for that 案件类型; empty when the group has no code"},
    "来源": {"origin": "fixed", "note": "最高人民法院指导性案例"},
    "审理程序": {"origin": "empty", "note": "a guiding case spans instances, so no single procedure applies"},
    "裁判日期": {"origin": "empty", "note": "the SPC publishes a 发布日期 for a guiding case, not a judgment date"},
    "公开日期": {"origin": "verbatim", "note": "the date shown on the listing page"},
    "当事人": {"origin": "empty", "note": "the publisher anonymises the parties itself (某); no party list is published"},
    "案由": {"origin": "empty", "note": "a guiding case publishes no 案由 label, and cutting a phrase out of the case name would invent one; the 关键词 terms are the closest published thing"},
    "法律依据": {"origin": "verbatim", "note": "the page's 相关法条 section"},
    "全文": {"origin": "verbatim", "note": "title, 发布 note and every published section, plain text, markup removed, nothing reworded"},
}


def main():
    ap = argparse.ArgumentParser(description="build the bundled guiding-cases dataset")
    ap.add_argument("--out", default="data/guiding-cases")
    ap.add_argument("--cache", default="tools/state/guiding-cases-raw")
    ap.add_argument("--refresh", action="store_true", help="refetch every page")
    ap.add_argument("--version", default="1.1.0")
    args = ap.parse_args()

    out, cache = Path(args.out), Path(args.cache)
    out.mkdir(parents=True, exist_ok=True)
    fetcher = Fetcher(cache, args.refresh)

    first = fetcher.get(BASE + LIST_PATH.format(page=""))
    pages = last_listing_page(first)
    items, seen = [], set()
    for p in range(1, pages + 1):
        page = first if p == 1 else fetcher.get(BASE + LIST_PATH.format(page="_%d" % p))
        found = listing_items(page)
        log("listing page %2d: %d entries" % (p, len(found)))
        for it in found:
            if it["path"] in seen:
                continue
            seen.add(it["path"])
            it["page"] = p
            items.append(it)
    log("listing entries: %d unique" % len(items))

    cases, notices, unparsed = [], [], []
    for i, it in enumerate(items, 1):
        url = BASE + it["path"]
        page = fetcher.get(url)
        body = article_body(page)
        if body is None:
            unparsed.append(it["path"])
            continue
        lines = html_to_lines(body)
        meta = fetcher.log.get(url, {})
        # The listing titles are the index: every case entry is named
        # 指导(性)案例NNN号：…, every other entry is a batch notice (some of
        # which reprint the whole batch inline, sections and all).
        is_case = bool(CASE_LABEL_RE.match(it["title"]))
        if is_case:
            rec = parse_case(lines, it)
            if not any(name.endswith("要点") for name in rec["section_order"]):
                raise SystemExit("case %s has no 要点 section: %s" % (rec["case_no"], url))
            rec["source_url"] = url
            rec["page_sha256"] = meta.get("sha256", "")
            rec["fetched_at"] = meta.get("fetched_at", "")
            rec["publish_date"] = it["list_date"]
            rec["full_text"] = compose_full_text(rec)
            rec["full_text_sha256"] = sha256_text(rec["full_text"])
            rec["full_text_chars"] = len(rec["full_text"])
            cases.append(rec)
        else:
            rec = parse_notice(lines, it)
            rec["source_url"] = url
            rec["page_sha256"] = meta.get("sha256", "")
            rec["fetched_at"] = meta.get("fetched_at", "")
            notices.append(rec)
        if i % 50 == 0:
            log("  parsed %d/%d" % (i, len(items)))

    # Cases the listing announced but never published: read them from the
    # mirror pages named in SUPPLEMENTS, same parse path as every case page.
    have = {r["case_no"] for r in cases}
    for sup in SUPPLEMENTS:
        if sup["case_no"] in have:
            continue
        page = fetcher.get(sup["url"])
        body = mirror_article(page)
        if body is None:
            raise SystemExit("supplement page has no article: %s" % sup["url"])
        meta = fetcher.log.get(sup["url"], {})
        item = {"title": sup["title"], "path": sup["url"],
                "list_date": sup["list_date"]}
        rec = parse_case(html_to_lines(body), item)
        if rec["case_no"] != sup["case_no"]:
            raise SystemExit("supplement %s parsed as %s: %s"
                             % (sup["case_no"], rec["case_no"], sup["url"]))
        rec["source_url"] = sup["url"]
        rec["page_sha256"] = meta.get("sha256", "")
        rec["fetched_at"] = meta.get("fetched_at", "")
        rec["publish_date"] = sup["list_date"]
        rec["full_text"] = compose_full_text(rec)
        rec["full_text_sha256"] = sha256_text(rec["full_text"])
        rec["full_text_chars"] = len(rec["full_text"])
        cases.append(rec)
        log("supplemented %s from %s" % (rec["case_no_label"], sup["url"]))
    fetcher.save_log()

    cases.sort(key=lambda r: r["case_no"])
    batch_of = {}
    for n in notices:
        if n.get("batch") is None:
            continue
        for lo, hi in n["case_ranges"]:
            for no in range(lo, hi + 1):
                batch_of.setdefault(no, {"batch": n["batch"], "doc_no": n["doc_no"],
                                         "notice_url": n["source_url"],
                                         "notice_date": n["publish_date"]})
    for rec in cases:
        info = batch_of.get(rec["case_no"])
        if info:
            rec["batch"] = info

    numbers = [r["case_no"] for r in cases]
    missing = sorted(set(range(min(numbers), max(numbers) + 1)) - set(numbers))
    dupes = sorted({n for n in numbers if numbers.count(n) > 1})
    if dupes:
        raise SystemExit("duplicate case numbers: %s" % dupes)
    section_counts = {}
    for rec in cases:
        for name in rec["section_order"]:
            section_counts[name] = section_counts.get(name, 0) + 1

    jsonl = out / "cases.jsonl"
    with open(jsonl, "w", encoding="utf-8") as fh:
        for rec in cases:
            fh.write(json.dumps(rec, ensure_ascii=False, sort_keys=True) + "\n")
    csv_path = out / "cases.csv"
    with open(csv_path, "w", encoding="utf-8", newline="") as fh:
        writer = csv.DictWriter(fh, fieldnames=CSV_COLUMNS, quoting=csv.QUOTE_MINIMAL)
        writer.writeheader()
        for rec in cases:
            writer.writerow(csv_row(rec))
    notices_path = out / "notices.jsonl"
    with open(notices_path, "w", encoding="utf-8") as fh:
        for rec in sorted(notices, key=lambda r: r["publish_date"]):
            fh.write(json.dumps(rec, ensure_ascii=False, sort_keys=True) + "\n")

    files = {}
    for path, records in ((jsonl, len(cases)), (csv_path, len(cases)),
                          (notices_path, len(notices))):
        files[path.name] = {"sha256": sha256_file(path), "bytes": path.stat().st_size,
                            "records": records}
    fetched = [r["fetched_at"] for r in cases + notices if r.get("fetched_at")]

    manifest = {
        "dataset": "guiding-cases",
        "title": "最高人民法院指导性案例（指导案例1号 至 指导性案例%d号）" % max(numbers),
        "version": args.version,
        "generated_at": now_iso(),
        "generator": "tools/crawl_guiding_cases.py",
        "source": {
            "publisher": "最高人民法院",
            "site": BASE,
            "listing_url": BASE + LIST_PATH.format(page=""),
            "listing_pages": pages,
            "listing_entries": len(items),
            "fetched_first": min(fetched) if fetched else "",
            "fetched_last": max(fetched) if fetched else "",
        },
        "counts": {
            "cases": len(cases),
            "notices": len(notices),
            "case_numbers_min": min(numbers),
            "case_numbers_max": max(numbers),
            "case_numbers_missing": missing,
            "with_batch_attribution": sum(1 for r in cases if "batch" in r),
            "sections": section_counts,
            "full_text_chars_total": sum(r["full_text_chars"] for r in cases),
        },
        "files": files,
        "fields": FIELD_DOC,
        "notes": [
            "指导案例45号 is announced by 第10批 (指导案例45-52号) but the site's 指导案例 "
            "listing publishes no page for it, and no xiangqing page exists for it; its "
            "record is taken verbatim from the 洛阳中院 mirror named in the crawler's "
            "SUPPLEMENTS, so the dataset holds %d of 1-%d."
            % (len(cases), max(numbers)),
            "%d cases publish no 裁判结果 section, so 案号 is empty for them."
            % sum(1 for r in cases if not r["judgment_case_numbers"]),
            "%d 执行 cases publish 执行实施要点 where the others publish 裁判要点."
            % section_counts.get("执行实施要点", 0),
            "法院, 所属地区, 审理程序, 当事人 and 案由 are left empty: a guiding-case page "
            "publishes none of them as a field. Nothing is inferred into a filterable column.",
            "batch attribution covers %d of the %d cases: %d of the %d notices in "
            "this listing name case-number ranges (the 第1批 notice names its cases "
            "by title only, and the 第2批 notice is not in this listing); batches "
            "after 第16批 were published here without a notice."
            % (sum(1 for r in cases if "batch" in r), len(cases),
               sum(1 for n in notices if n["case_ranges"]), len(notices)),
            "全文 is plain text, so a search over it needs index_with_full_text = "
            "true; at %d kB of text that costs nothing."
            % (sum(r["full_text_chars"] for r in cases) // 1000),
        ],
        "case_json_fields": [
            "case_no", "case_no_label", "title", "title_full", "publish_note",
            "publish_date", "batch", "keywords", "case_group",
            "sections", "section_order", "judgment_case_numbers",
            "courts_mentioned", "full_text", "full_text_sha256", "full_text_chars",
            "source_url", "page_sha256", "fetched_at",
        ],
        "redistribution": {
            "basis": "official judicial document of the Supreme People's Court, published "
                     "for courts to refer to in similar cases; the publisher anonymises the "
                     "parties itself, so a record names no private individual",
            "cite": "cite the 指导案例 number and the source_url stored with every record",
            "checked_at": now_iso(),
        },
    }
    (out / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    log("cases %d (numbers %d-%d, missing %s), notices %d, unparsed %d"
        % (len(cases), min(numbers), max(numbers), missing or "none", len(notices), len(unparsed)))
    log("sections seen: %s" % json.dumps(section_counts, ensure_ascii=False, sort_keys=True))
    if unparsed:
        log("unparsed: %s" % unparsed[:10])
    log("wrote %s" % out)


if __name__ == "__main__":
    main()
