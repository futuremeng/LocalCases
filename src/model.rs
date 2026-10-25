//! L0 record: one row of the raw CSV, all 15 columns kept verbatim.
//!
//! Deliberately separate from the legacy `Case` struct: `Case` is what the v1
//! database holds (12 columns, `full_text` wrapped in `<p>…</p>` by convert).
//! Keeping both means the already-built v1 binary can still read `fjall/`
//! while v2 binaries work on a new keyspace.

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// The raw CSV has exactly one header layout across all 37 zips (verified):
/// 原始链接 案号 案件名称 法院 所属地区 案件类型 案件类型编码 来源 审理程序
/// 裁判日期 公开日期 当事人 案由 法律依据 全文
#[derive(Debug, Clone, Default, Encode, Decode, Serialize, Deserialize)]
pub struct CaseV2 {
    #[serde(rename = "原始链接")]
    pub doc_url: String,
    #[serde(rename = "案号")]
    pub case_id: String,
    #[serde(rename = "案件名称")]
    pub case_name: String,
    #[serde(rename = "法院")]
    pub court: String,
    #[serde(rename = "所属地区")]
    pub region: String,
    #[serde(rename = "案件类型")]
    pub case_type: String,
    #[serde(rename = "案件类型编码")]
    pub case_type_code: String,
    #[serde(rename = "来源")]
    pub source: String,
    #[serde(rename = "审理程序")]
    pub procedure: String,
    #[serde(rename = "裁判日期")]
    pub judgment_date: String,
    #[serde(rename = "公开日期")]
    pub public_date: String,
    #[serde(rename = "当事人")]
    pub parties: String,
    #[serde(rename = "案由")]
    pub cause: String,
    #[serde(rename = "法律依据")]
    pub legal_basis: String,
    #[serde(rename = "全文")]
    pub full_text: String,
}

impl CaseV2 {
    /// Stable identity of a document: the `docId=` part of 原始链接.
    /// Falls back to the whole URL when the marker is absent.
    pub fn doc_key(&self) -> &str {
        match self.doc_url.rsplit_once("docId=") {
            Some((_, id)) if !id.is_empty() => id,
            _ => {
                // Some rows carry a bare id or a different parameter name.
                match self.doc_url.rsplit_once('=') {
                    Some((_, id)) if !id.is_empty() => id,
                    _ => self.doc_url.as_str(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_key_prefers_docid_param() {
        let c = CaseV2 {
            doc_url: [
                "https://wenshu.court.gov.cn/website/wenshu/181107ANFZ0BXSK4/index.html?",
                "docId=964fc681687d4e47a0a9ace500096dde",
            ]
            .concat(),
            ..CaseV2::default()
        };
        assert_eq!(c.doc_key(), "964fc681687d4e47a0a9ace500096dde");
    }

    #[test]
    fn doc_key_falls_back_to_last_eq() {
        let c = CaseV2 {
            doc_url: "https://example/x?id=abc".into(),
            ..CaseV2::default()
        };
        assert_eq!(c.doc_key(), "abc");
    }

    #[test]
    fn doc_key_handles_bare_url() {
        let c = CaseV2 {
            doc_url: "https://example/x".into(),
            ..CaseV2::default()
        };
        assert_eq!(c.doc_key(), "https://example/x");
    }
}
