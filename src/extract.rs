//! Derived (L1) fields extracted from an L0 record: typed dates, parsed case
//! numbers, procedure split, law references, text profile.
//!
//! Pure functions only — no I/O — so they can be unit-tested against the
//! shapes that actually occur in the corpus. `regex` is not a dependency of
//! this crate, so the parsers below are hand-written char scanners.

use bincode::{Decode, Encode};
use jiff::civil::{Date, date};
use serde::{Deserialize, Serialize};

use crate::model::CaseV2;

/// Compact per-record derived facts. Sentinels (0 / -1) mean "unknown" so the
/// bincode payload stays small; the v1 code stored every date as text.
#[derive(Debug, Clone, Default, Encode, Decode, Serialize, Deserialize)]
pub struct Derived {
    /// days since 1970-01-01, -1 when 裁判日期 is unparseable
    pub judgment_days: i64,
    /// days since 1970-01-01, -1 when 公开日期 is unparseable
    pub public_days: i64,
    /// year taken out of 案号, 0 when absent (99.8% of rows have one)
    pub case_year: u16,
    /// running number of 案号, 0 when absent
    pub case_seq: u32,
    /// numeric 案件类型编码 (1 民事 / 2 执行 / 3 行政 / 4 刑事 / 5 管辖 / 8 非诉保全审查)
    pub case_type_code: u16,
    /// 审理程序 split into group + kind, e.g. ("民事", "一审")
    pub procedure_group: String,
    pub procedure_kind: String,
    pub party_count: u32,
    pub law_count: u32,
    pub text_len: u32,
    pub has_text: bool,
    /// true when 全文 contains markup, i.e. it must not be rendered raw
    pub has_html: bool,
    /// 64-bit fingerprint of 全文 for cheap equality / dedup checks
    pub text_hash: u64,
    /// court short code inside 案号, e.g. 青羊民初 in （2012）青羊民初字第5385号
    pub court_code: String,
}

/// Days since 1970-01-01, or -1. All 裁判日期 values in the sampled corpus are
/// valid `YYYY-MM-DD`, so this mostly guards against future data.
pub fn date_days(s: &str) -> i64 {
    match s.trim().parse::<Date>() {
        Ok(d) => match d.since(date(1970, 1, 1)) {
            Ok(td) => td.get_days() as i64,
            Err(_) => -1,
        },
        Err(_) => -1,
    }
}

/// Split 案号 into (year, court+type code, sequence).
///
/// Real shapes seen in the corpus:
/// - `（2012）青羊民初字第5385号` → `(2012, "青羊民初", 5385)`
/// - `(2019)京0105民初12345号`   → `(2019, "京0105民初", 12345)`
/// - `（2020）湘0822执64号`       → `(2020, "湘0822执", 64)`
///
/// Parentheses are full-width or ASCII. The sequence is the last digit run,
/// preferring one directly followed by `号`, which is what makes the numeric
/// court codes (`京0105`) survive instead of being mistaken for the sequence.
pub fn parse_case_number(s: &str) -> (u16, String, u32) {
    let v: Vec<char> = s.trim().chars().collect();
    let n = v.len();

    // 1) ( YYYY )
    let mut i = 0usize;
    while i < n && v[i] != '(' && v[i] != '（' {
        i += 1;
    }
    if i >= n {
        return (0, String::new(), 0);
    }
    i += 1;
    let mut digits = String::new();
    while i < n && v[i].is_ascii_digit() {
        digits.push(v[i]);
        i += 1;
    }
    if digits.len() != 4 {
        return (0, String::new(), 0);
    }
    let year: u16 = digits.parse().unwrap_or(0);
    while i < n && (v[i] == ')' || v[i] == '）') {
        i += 1;
    }
    if i >= n {
        return (year, String::new(), 0);
    }
    let tail = &v[i..];

    // 2) last digit run (preferring the one followed by 号) is the sequence
    let mut best: Option<(usize, usize)> = None;
    let mut j = 0usize;
    while j < tail.len() {
        if tail[j].is_ascii_digit() {
            let start = j;
            while j < tail.len() && tail[j].is_ascii_digit() {
                j += 1;
            }
            let is_seq = j >= tail.len() || tail[j] == '号';
            if is_seq || best.is_none() {
                best = Some((start, j - start));
            }
        } else {
            j += 1;
        }
    }

    match best {
        None => (year, tail.iter().collect::<String>().trim().to_string(), 0),
        Some((start, len)) => {
            let code: String = tail[..start].iter().collect();
            let seq: String = tail[start..start + len].iter().collect();
            // `青羊民初字第` → `青羊民初`, `湘0822执字` → `湘0822执`
            (
                year,
                code.trim().trim_end_matches(['第', '字', '号']).to_string(),
                seq.parse().unwrap_or(0),
            )
        }
    }
}

/// 审理程序 is usually "group + kind" (`民事一审`, `执行实施`), sometimes only
/// one half (`一审`, `特别程序`, `刑罚变更`), and occasionally two whitespace
/// separated tokens (`民事 一审`). Longest known kind suffix wins.
const PROCEDURE_KINDS: &[&str] = &[
    "再审审查与审判监督",
    "非诉执行审查",
    "死刑复核",
    "国家赔偿",
    "其他",
    "一审",
    "二审",
    "再审",
    "复议",
    "实施",
    "审查",
    "监督",
    "清算",
    "重整",
    "和解",
    "破产",
    "执行",
    "程序",
];

/// Case-type / procedure groups, longest first so `国家赔偿` is not cut short.
const PROCEDURE_GROUPS: &[&str] = &[
    "国家赔偿",
    "司法救助",
    "强制清算",
    "知识产权",
    "民事",
    "刑事",
    "行政",
    "执行",
    "赔偿",
    "管辖",
    "区际",
];

/// Group a document belongs to, preferring a prefix already present in
/// 审理程序 (`刑事一审` → 刑事) and falling back to 案件类型 (`民事案件` → 民事).
fn procedure_group_of(procedure: &str, case_type: &str) -> String {
    PROCEDURE_GROUPS
        .iter()
        .find(|g| procedure.starts_with(*g))
        .or_else(|| PROCEDURE_GROUPS.iter().find(|g| case_type.starts_with(*g)))
        .map(|g| g.to_string())
        .unwrap_or_default()
}

/// Split 审理程序 into `(group, kind)`; the group never relies on the
/// procedure column alone, so single-half values still get a group
/// (`一审` + 案件类型 `民事案件` → 民事 / 一审).
pub fn split_procedure(s: &str, case_type: &str) -> (String, String) {
    let parts: Vec<&str> = s.split_whitespace().collect();
    let Some(first) = parts.first() else {
        return (String::new(), String::new());
    };
    if parts.len() > 1 {
        return (first.to_string(), parts[1..].join(""));
    }
    for kind in PROCEDURE_KINDS {
        if let Some(group) = first.strip_suffix(kind) {
            let group = group.trim();
            let group = if group.is_empty() {
                procedure_group_of(first, case_type)
            } else {
                group.to_string()
            };
            return (group, kind.to_string());
        }
    }
    (procedure_group_of(first, case_type), first.to_string())
}

/// Law names referenced in 法律依据: `《中华人民共和国合同法》：第一百一十条，…；
/// 《中华人民共和国民事诉讼法（2013年）》：第六十四条第一款`. 67.1% of rows
/// carry at least one reference.
pub fn law_names(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    while let Some(rel) = s[start..].find('《') {
        let open = start + rel;
        let body = open + '《'.len_utf8();
        match s[body..].find('》') {
            Some(rel2) => {
                let close = body + rel2;
                let name = s[body..close].trim();
                if !name.is_empty() {
                    out.push(name.to_string());
                }
                start = close + '》'.len_utf8();
            }
            None => break,
        }
    }
    out
}

/// Markup detector for 全文. Verified across all 37 zips (65,177 sampled rows):
/// the corpus carries no markup at all, so this only guards future sources —
/// but it is recorded per record because the HTML rendering path depends on it.
pub fn looks_like_html(s: &str) -> bool {
    s.contains("<p") || s.contains("</") || s.contains("<div") || s.contains("c_header")
}

/// Stable 64-bit fingerprint (FNV-1a); no hashing crate needed.
pub fn text_fingerprint(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

pub fn derive(case: &CaseV2) -> Derived {
    let (case_year, court_code, case_seq) = parse_case_number(&case.case_id);
    let (procedure_group, procedure_kind) = split_procedure(&case.procedure, &case.case_type);
    let laws = law_names(&case.legal_basis);
    let parties = case.parties.trim();
    let party_count = if parties.is_empty() {
        0
    } else {
        parties.split(',').filter(|p| !p.trim().is_empty()).count() as u32
    };
    let text = case.full_text.trim();
    Derived {
        judgment_days: date_days(&case.judgment_date),
        public_days: date_days(&case.public_date),
        case_year,
        case_seq,
        case_type_code: case.case_type_code.trim().parse().unwrap_or(0),
        procedure_group,
        procedure_kind,
        party_count,
        law_count: laws.len() as u32,
        text_len: text.chars().count() as u32,
        has_text: !text.is_empty(),
        has_html: looks_like_html(text),
        text_hash: text_fingerprint(text),
        court_code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_width_case_number() {
        assert_eq!(
            parse_case_number("（2012）青羊民初字第5385号"),
            (2012, "青羊民初".into(), 5385)
        );
    }

    #[test]
    fn parses_numeric_court_code_case_number() {
        assert_eq!(
            parse_case_number("(2019)京0105民初12345号"),
            (2019, "京0105民初".into(), 12345)
        );
        assert_eq!(
            parse_case_number("（2020）湘0822执64号"),
            (2020, "湘0822执".into(), 64)
        );
        assert_eq!(
            parse_case_number("（2018）最高法民终163号"),
            (2018, "最高法民终".into(), 163)
        );
        assert_eq!(
            parse_case_number("（2020）湘0822执字64号"),
            (2020, "湘0822执".into(), 64)
        );
    }

    #[test]
    fn junk_case_number_yields_zeroes() {
        assert_eq!(parse_case_number("无"), (0, String::new(), 0));
        assert_eq!(parse_case_number(""), (0, String::new(), 0));
    }

    #[test]
    fn splits_procedures() {
        assert_eq!(
            split_procedure("民事一审", "民事案件"),
            ("民事".into(), "一审".into())
        );
        assert_eq!(
            split_procedure("执行实施", "执行案件"),
            ("执行".into(), "实施".into())
        );
        assert_eq!(
            split_procedure("特别程序", "民事案件"),
            ("特别".into(), "程序".into())
        );
        assert_eq!(
            split_procedure("民事 一审", ""),
            ("民事".into(), "一审".into())
        );
        // half-only values borrow the group from 案件类型
        assert_eq!(
            split_procedure("一审", "民事案件"),
            ("民事".into(), "一审".into())
        );
        assert_eq!(
            split_procedure("刑罚变更", "刑事案件"),
            ("刑事".into(), "刑罚变更".into())
        );
        assert_eq!(split_procedure("其他", ""), ("".into(), "其他".into()));
        assert_eq!(split_procedure("", ""), ("".into(), "".into()));
    }

    #[test]
    fn extracts_law_names() {
        let s = "《中华人民共和国合同法》：第一百一十条，第六十一条；\
                 《中华人民共和国民事诉讼法（2013年）》：第六十四条第一款";
        assert_eq!(
            law_names(s),
            vec!["中华人民共和国合同法", "中华人民共和国民事诉讼法（2013年）"]
        );
        assert!(law_names("").is_empty());
        assert!(law_names("没有引用").is_empty());
    }

    #[test]
    fn dates_to_days() {
        assert_eq!(date_days("1970-01-01"), 0);
        assert_eq!(date_days("2013-01-31"), 15736);
        assert_eq!(date_days(""), -1);
        assert_eq!(date_days("2013-1-31"), -1);
    }

    #[test]
    fn html_detector_catches_markup_only() {
        assert!(looks_like_html("<p>合同</p>"));
        assert!(looks_like_html("<div class=\"c_header\">x"));
        assert!(!looks_like_html(
            "原告与被告签订《买卖合同》，价款 100 万元。"
        ));
    }

    #[test]
    fn fingerprint_is_stable_and_distinct() {
        assert_eq!(text_fingerprint("abc"), text_fingerprint("abc"));
        assert_ne!(text_fingerprint("abc"), text_fingerprint("abd"));
    }
}
