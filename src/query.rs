//! Structured filtering + aggregation for the schema-v2 index.
//!
//! Filter values come from the query string; a comma separates several values of
//! the same field (OR), different fields are AND'ed. Everything is built out of
//! exact terms on the raw (`*_s`) fields and ranges on the numeric fast fields,
//! so filtering never depends on jieba tokenization.

use serde::Deserialize;
use tantivy::Term;
use tantivy::query::{BooleanQuery, Occur, Query, RangeQuery, TermQuery};
use tantivy::schema::{Field, IndexRecordOption};

use crate::extract::date_days;
use crate::search::StructuredFields;

/// Upper bound on facet buckets: `size=` must not be able to ask for an
/// unbounded term scan.
pub const MAX_FACET_SIZE: usize = 1000;

/// Public facet/filter name -> tantivy field name.
pub const GROUPS: &[(&str, &str)] = &[
    ("court", "court_s"),
    ("region", "region_s"),
    ("cause", "cause_s"),
    ("case_type", "case_type_s"),
    ("case_type_code", "case_type_code_s"),
    ("procedure", "procedure_s"),
    ("procedure_group", "procedure_group_s"),
    ("procedure_kind", "procedure_kind_s"),
    ("source", "source_s"),
    ("law", "law_s"),
    ("court_code", "court_code_s"),
    ("year", "year"),
    ("case_year", "case_year"),
];

/// Structured filters. Every field is optional; `parse` fills it from the query
/// string and reports unusable input (bad dates, unknown groups) as `String`
/// so the HTTP layer can answer 400 instead of silently widening the query.
#[derive(Debug, Default, Clone)]
pub struct Filters {
    pub court: Vec<String>,
    pub region: Vec<String>,
    pub cause: Vec<String>,
    pub case_type: Vec<String>,
    pub case_type_code: Vec<String>,
    pub procedure: Vec<String>,
    pub procedure_group: Vec<String>,
    pub procedure_kind: Vec<String>,
    pub source: Vec<String>,
    pub law: Vec<String>,
    pub court_code: Vec<String>,
    pub year_from: Option<u64>,
    pub year_to: Option<u64>,
    /// 裁判日期 range, as days since 1970-01-01
    pub judgment_from: Option<i64>,
    pub judgment_to: Option<i64>,
    pub public_from: Option<i64>,
    pub public_to: Option<i64>,
    pub has_text: Option<bool>,
    pub has_html: Option<bool>,
    pub min_text_len: Option<u64>,
    pub max_text_len: Option<u64>,
    /// id cursor for deep pagination (`id_gt`)
    pub id_gt: Option<u64>,
}

fn split_values(raw: Option<&String>) -> Vec<String> {
    raw.map(|v| {
        v.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

fn as_num(raw: Option<&String>) -> Option<u64> {
    raw.and_then(|v| v.trim().parse::<u64>().ok())
}

/// `YYYY-MM-DD`, or a bare year (which becomes the first/last day of that year,
/// depending on whether it is a `_from` or a `_to` bound).
fn as_days(key: &str, raw: Option<&String>) -> Result<Option<i64>, String> {
    let Some(raw) = raw.map(|v| v.trim()).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    if raw.len() == 4 && raw.bytes().all(|b| b.is_ascii_digit()) {
        let year: i64 = raw.parse().unwrap();
        let bound = if key.ends_with("_from") {
            "01-01"
        } else {
            "12-31"
        };
        return Ok(Some(date_days(&format!("{year}-{bound}"))));
    }
    let d = date_days(raw);
    if d < 0 {
        return Err(format!(
            "invalid date for {key}: {raw} (expected YYYY-MM-DD)"
        ));
    }
    Ok(Some(d))
}

/// Everything the search endpoints accept, as raw query-string values.
///
/// The legacy parameters (`search`, `offset`, `export`, `search_type`) keep
/// their exact meaning; the rest are the structured layer. Unknown parameters
/// are ignored, so old clients are unaffected.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct SearchParams {
    pub search: Option<String>,
    pub offset: Option<usize>,
    pub export: Option<bool>,
    pub search_type: Option<String>,

    pub court: Option<String>,
    pub region: Option<String>,
    pub cause: Option<String>,
    pub case_type: Option<String>,
    pub case_type_code: Option<String>,
    pub procedure: Option<String>,
    pub procedure_group: Option<String>,
    pub procedure_kind: Option<String>,
    pub source: Option<String>,
    pub law: Option<String>,
    pub court_code: Option<String>,
    pub year_from: Option<String>,
    pub year_to: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub pub_from: Option<String>,
    pub pub_to: Option<String>,
    pub has_text: Option<String>,
    pub has_html: Option<String>,
    pub min_text_len: Option<String>,
    pub max_text_len: Option<String>,
    pub id_gt: Option<String>,

    /// `relevance` (default, byte-identical to the v1 ranking), `date`, `id`
    pub sort: Option<String>,
    /// comma separated facet groups for `group_by`
    pub group_by: Option<String>,
    /// facet bucket count
    pub size: Option<usize>,
}

impl SearchParams {
    pub fn filters(&self) -> Result<Filters, String> {
        Ok(Filters {
            court: split_values(self.court.as_ref()),
            region: split_values(self.region.as_ref()),
            cause: split_values(self.cause.as_ref()),
            case_type: split_values(self.case_type.as_ref()),
            case_type_code: split_values(self.case_type_code.as_ref()),
            procedure: split_values(self.procedure.as_ref()),
            procedure_group: split_values(self.procedure_group.as_ref()),
            procedure_kind: split_values(self.procedure_kind.as_ref()),
            source: split_values(self.source.as_ref()),
            law: split_values(self.law.as_ref()),
            court_code: split_values(self.court_code.as_ref()),
            year_from: as_num(self.year_from.as_ref()),
            year_to: as_num(self.year_to.as_ref()),
            judgment_from: as_days("date_from", self.date_from.as_ref())?,
            judgment_to: as_days("date_to", self.date_to.as_ref())?,
            public_from: as_days("pub_from", self.pub_from.as_ref())?,
            public_to: as_days("pub_to", self.pub_to.as_ref())?,
            has_text: self
                .has_text
                .as_ref()
                .and_then(|v| v.trim().parse::<bool>().ok()),
            has_html: self
                .has_html
                .as_ref()
                .and_then(|v| v.trim().parse::<bool>().ok()),
            min_text_len: as_num(self.min_text_len.as_ref()),
            max_text_len: as_num(self.max_text_len.as_ref()),
            id_gt: as_num(self.id_gt.as_ref()),
        })
    }

    pub fn groups(&self) -> Vec<String> {
        self.group_by
            .as_deref()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn sort(&self) -> Sort {
        match self.sort.as_deref() {
            Some("date") => Sort::Date,
            Some("id") => Sort::Id,
            _ => Sort::Relevance,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Relevance,
    Date,
    Id,
}

impl Filters {
    pub fn is_empty(&self) -> bool {
        self.court.is_empty()
            && self.region.is_empty()
            && self.cause.is_empty()
            && self.case_type.is_empty()
            && self.case_type_code.is_empty()
            && self.procedure.is_empty()
            && self.procedure_group.is_empty()
            && self.procedure_kind.is_empty()
            && self.source.is_empty()
            && self.law.is_empty()
            && self.court_code.is_empty()
            && self.year_from.is_none()
            && self.year_to.is_none()
            && self.judgment_from.is_none()
            && self.judgment_to.is_none()
            && self.public_from.is_none()
            && self.public_to.is_none()
            && self.has_text.is_none()
            && self.has_html.is_none()
            && self.min_text_len.is_none()
            && self.max_text_len.is_none()
            && self.id_gt.is_none()
    }

    /// Exact term(s) for one field: one value → one term, several → OR.
    fn term_query(field: Field, values: &[String]) -> Option<Box<dyn Query>> {
        match values.len() {
            0 => None,
            1 => Some(Box::new(TermQuery::new(
                Term::from_field_text(field, &values[0]),
                IndexRecordOption::Basic,
            ))),
            _ => Some(Box::new(BooleanQuery::new(
                values
                    .iter()
                    .map(|v| {
                        (
                            Occur::Should,
                            Box::new(TermQuery::new(
                                Term::from_field_text(field, v),
                                IndexRecordOption::Basic,
                            )) as Box<dyn Query>,
                        )
                    })
                    .collect(),
            ))),
        }
    }

    fn u64_range(field: Field, from: Option<u64>, to: Option<u64>) -> Option<Box<dyn Query>> {
        if from.is_none() && to.is_none() {
            return None;
        }
        let lo = std::ops::Bound::Included(Term::from_field_u64(field, from.unwrap_or(0)));
        let hi = std::ops::Bound::Included(Term::from_field_u64(field, to.unwrap_or(u64::MAX)));
        Some(Box::new(RangeQuery::new(lo, hi)))
    }

    fn i64_range(field: Field, from: Option<i64>, to: Option<i64>) -> Option<Box<dyn Query>> {
        if from.is_none() && to.is_none() {
            return None;
        }
        // -1 is the "unknown date" sentinel, so a real range starts above it.
        let lo = std::ops::Bound::Included(Term::from_field_i64(field, from.unwrap_or(0)));
        let hi = std::ops::Bound::Included(Term::from_field_i64(field, to.unwrap_or(i64::MAX)));
        Some(Box::new(RangeQuery::new(lo, hi)))
    }

    /// One clause per active filter; the caller AND's them with the keyword
    /// query (`Occur::Must`).
    pub fn clauses(&self, f: &StructuredFields) -> Vec<Box<dyn Query>> {
        let mut out: Vec<Box<dyn Query>> = Vec::new();
        let text = [
            (f.court, &self.court),
            (f.region, &self.region),
            (f.cause, &self.cause),
            (f.case_type, &self.case_type),
            (f.case_type_code, &self.case_type_code),
            (f.procedure, &self.procedure),
            (f.procedure_group, &self.procedure_group),
            (f.procedure_kind, &self.procedure_kind),
            (f.source, &self.source),
            (f.law, &self.law),
            (f.court_code, &self.court_code),
        ];
        for (field, values) in text {
            if let Some(q) = Self::term_query(field, values) {
                out.push(q);
            }
        }

        if let Some(q) = Self::u64_range(f.year, self.year_from, self.year_to) {
            out.push(q);
        }
        if let Some(q) = Self::i64_range(f.judgment_days, self.judgment_from, self.judgment_to) {
            out.push(q);
        }
        if let Some(q) = Self::i64_range(f.public_days, self.public_from, self.public_to) {
            out.push(q);
        }
        if let Some(has) = self.has_text {
            out.push(Box::new(TermQuery::new(
                Term::from_field_bool(f.has_text, has),
                IndexRecordOption::Basic,
            )));
        }
        if let Some(has) = self.has_html {
            out.push(Box::new(TermQuery::new(
                Term::from_field_bool(f.has_html, has),
                IndexRecordOption::Basic,
            )));
        }
        if let Some(q) = Self::u64_range(f.text_len, self.min_text_len, self.max_text_len) {
            out.push(q);
        }
        // Cursor pagination: strictly greater than the last seen id.
        if let Some(id) = self.id_gt {
            out.push(Box::new(RangeQuery::new(
                std::ops::Bound::Excluded(Term::from_field_u64(f.id, id)),
                std::ops::Bound::Unbounded,
            )));
        }
        out
    }
}

/// Public facet name → tantivy field name. Unknown names are rejected by the
/// caller instead of quietly returning nothing.
pub fn resolve_group(name: &str) -> Option<&'static str> {
    GROUPS.iter().find(|(g, _)| *g == name).map(|(_, f)| *f)
}

/// Terms-agg accuracy per field. tantivy has to walk the term dictionary of
/// every segment, and the cost is proportional to `segment_size` × segments:
/// 法律名 has ~30k distinct terms, so an exhaustive scan costs seconds on the full
/// corpus while its top buckets (hundreds of thousands of hits each) are already
/// inside the top 2k of every segment. Low-cardinality fields get the generous
/// budget instead, which is what the facet UI reads.
fn segment_size(field: &str, size: usize) -> usize {
    let floor = match field {
        "law_s" => 2_000,
        _ => 10_000,
    };
    (size * 10).clamp(floor, 50_000)
}

/// `terms` aggregation request for the requested groups.
///
/// Accuracy note: tantivy reads the top `segment_size` terms of each segment and
/// merges, so counts in a multi-segment index can be approximate — the response
/// carries `doc_count_error_upper_bound` and `sum_other_doc_count` so the
/// undercount is visible rather than silent. `segment_size` is deliberately
/// generous (see `segment_size()`): on the full corpus it brings the top buckets
/// of every field to exact counts.
pub fn aggs_json(groups: &[String], size: usize) -> Result<serde_json::Value, String> {
    let mut map = serde_json::Map::new();
    for group in groups {
        let field = match resolve_group(group) {
            Some(f) => f,
            None => return Err(format!("unknown group_by: {group}")),
        };
        map.insert(
            group.clone(),
            serde_json::json!({
                "terms": {
                    "field": field,
                    "size": size,
                    "min_doc_count": 1,
                    "segment_size": segment_size(field, size),
                }
            }),
        );
    }
    Ok(serde_json::Value::Object(map))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(pairs: &[(&str, &str)]) -> SearchParams {
        let mut p = SearchParams::default();
        for (k, v) in pairs {
            let v = Some(v.to_string());
            match *k {
                "court" => p.court = v,
                "year_from" => p.year_from = v,
                "date_from" => p.date_from = v,
                "date_to" => p.date_to = v,
                "group_by" => p.group_by = v,
                "sort" => p.sort = v,
                _ => {}
            }
        }
        p
    }

    #[test]
    fn parses_comma_separated_values() {
        let p = params(&[
            ("court", "成都市中级人民法院,宝应县人民法院"),
            ("year_from", "2013"),
        ]);
        let f = p.filters().unwrap();
        assert_eq!(f.court.len(), 2);
        assert_eq!(f.court[1], "宝应县人民法院");
        assert_eq!(f.year_from, Some(2013));
        assert!(!f.is_empty());
    }

    #[test]
    fn bare_year_bounds_are_year_edges() {
        let p = params(&[("date_from", "2013"), ("date_to", "2013")]);
        let f = p.filters().unwrap();
        assert_eq!(f.judgment_from, Some(date_days("2013-01-01")));
        assert_eq!(f.judgment_to, Some(date_days("2013-12-31")));
    }

    #[test]
    fn rejects_bad_dates() {
        let p = params(&[("date_from", "2013-13-45")]);
        assert!(p.filters().is_err());
    }

    #[test]
    fn empty_when_nothing_given() {
        assert!(SearchParams::default().filters().unwrap().is_empty());
    }

    #[test]
    fn sort_defaults_to_relevance() {
        assert_eq!(SearchParams::default().sort(), Sort::Relevance);
        assert_eq!(params(&[("sort", "date")]).sort(), Sort::Date);
        assert_eq!(params(&[("group_by", "court,year")]).groups().len(), 2);
    }

    #[test]
    fn groups_resolve_and_aggs_build() {
        assert_eq!(resolve_group("court"), Some("court_s"));
        assert_eq!(resolve_group("nope"), None);
        let aggs = aggs_json(&["court".to_string(), "year".to_string()], 10).unwrap();
        assert_eq!(aggs["court"]["terms"]["field"], "court_s");
        assert_eq!(aggs["year"]["terms"]["field"], "year");
        assert!(aggs_json(&["nope".to_string()], 10).is_err());
    }
}
