//! The JSON API the desktop shell talks to. No HTML here: the UI is a separate
//! front-end, and keeping rendering out of the engine means the engine never has
//! to think about escaping.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use indexmap::IndexSet;
use serde::Serialize;
use serde_json::Value;
use std::sync::LazyLock;
use tantivy::aggregation::AggregationCollector;
use tantivy::aggregation::agg_req::Aggregations;
use tantivy::collector::{Count, TopDocs};
use tantivy::query::{AllQuery, BooleanQuery, Occur};
use tantivy::schema::Value as _;
use tantivy::{DocAddress, Order, TantivyDocument};
use tracing::info;

use crate::config::CONFIG;
use crate::extract::Derived;
use crate::model::CaseV2;
use crate::query::{SearchParams, Sort, aggs_json};
use crate::{AppState, Searcher};

/// Page size when `export` is not set.
const PAGE_SIZE: usize = 20;
/// Facet buckets per group unless the caller asks for another number.
const DEFAULT_FACET_SIZE: usize = 20;
/// Characters of 全文 returned with each hit.
const PREVIEW_CHARS: usize = 240;

static EXPORT_LIMIT: LazyLock<usize> = LazyLock::new(|| CONFIG.export_limit.unwrap_or(10_000));
static MAX_RESULTS: LazyLock<usize> = LazyLock::new(|| CONFIG.max_results.unwrap_or(50_000));

type ApiError = (StatusCode, String);

fn bad_request(e: impl std::fmt::Display) -> ApiError {
    (StatusCode::BAD_REQUEST, e.to_string())
}

fn internal(e: impl std::fmt::Display) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

/// One search result. `full_text` is deliberately absent: fetch the record.
#[derive(Serialize)]
pub struct Hit {
    pub id: u32,
    /// the source document link/key the row was imported from
    pub url: String,
    pub case_id: String,
    pub case_name: String,
    pub court: String,
    pub region: String,
    pub case_type: String,
    pub procedure: String,
    pub judgment_date: String,
    pub public_date: String,
    pub parties: String,
    pub cause: String,
    pub legal_basis: String,
    pub preview: String,
}

/// What the query was, and how many rows it matched.
#[derive(Serialize)]
pub struct Meta {
    pub offset: usize,
    pub search: String,
    pub search_type: String,
    pub limit: usize,
    pub total: usize,
    pub export: bool,
    /// the index carries the exact-match layer, so filters/facets work
    pub structured: bool,
}

#[derive(Serialize)]
pub struct SearchResponse {
    pub search_meta: Meta,
    pub cases: Vec<Hit>,
    /// terms aggregations, only when `group_by` named at least one group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facets: Option<Value>,
}

/// The whole imported row (source column names) plus the derived facts.
#[derive(Serialize)]
pub struct Record {
    pub id: u32,
    pub record: CaseV2,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derived: Option<Derived>,
}

/// Everything one query produces.
struct Outcome {
    hits: Vec<Hit>,
    meta: Meta,
    facets: Option<Value>,
}

/// Run one query: keyword (if any) AND the exact filters, then sort, page and
/// aggregate. This is the only place the index is touched, so `/api/search`,
/// `/api/facets` and `/api/export.csv` cannot disagree.
fn run(input: &SearchParams, state: &AppState) -> Result<Outcome, ApiError> {
    let started = std::time::Instant::now();
    let offset = input.offset.unwrap_or(0).min(*MAX_RESULTS);
    let export = input.export.unwrap_or(false);
    let limit = if export { *EXPORT_LIMIT } else { PAGE_SIZE };
    let text = input.search.clone().unwrap_or_default();
    // A bad filter value is a client error; widening the query silently would be
    // worse than refusing it.
    let filters = input.filters().map_err(bad_request)?;
    let groups = input.groups();
    let facet_size = input
        .size
        .unwrap_or(DEFAULT_FACET_SIZE)
        .clamp(1, crate::query::MAX_FACET_SIZE);
    let structured = state.searcher.structured.is_some();
    let sort = input.sort();

    let mut ids: IndexSet<u32> = IndexSet::with_capacity(limit.min(1024));
    let mut total = 0usize;
    let mut facets: Option<Value> = None;

    // A filter/facet/sort-only request with no keyword is still a real query.
    let structured_ask = !filters.is_empty() || !groups.is_empty() || input.sort.is_some();
    if !text.trim().is_empty() || structured_ask {
        let searcher = state.searcher.reader.searcher();
        // Half/full width and simplified/traditional are normalized, so either
        // spelling finds the other.
        let normalized = fast2s::convert(&text);
        let keyword: Box<dyn tantivy::query::Query> = if normalized.trim().is_empty() {
            Box::new(AllQuery)
        } else {
            Box::new(
                state
                    .searcher
                    .query_parser
                    .parse_query_lenient(&normalized)
                    .0,
            )
        };
        // A structured filter is added as a must clause next to the keyword
        // query, so what is left keeps the ordinary relevance ordering of the
        // keyword part.
        let query: Box<dyn tantivy::query::Query> =
            match (&state.searcher.structured, filters.is_empty()) {
                (Some(fields), false) => {
                    let mut occurs = vec![(Occur::Must, keyword)];
                    occurs.extend(
                        filters
                            .clauses(fields)
                            .into_iter()
                            .map(|clause| (Occur::Must, clause)),
                    );
                    Box::new(BooleanQuery::new(occurs))
                }
                // Without the exact layer the filters cannot be honoured, and
                // answering the keyword part alone is the honest fallback.
                _ => keyword,
            };

        total = searcher.search(&query, &Count).map_err(internal)?;
        ids = page_of(&searcher, &query, &state.searcher, sort, limit, offset).map_err(internal)?;
        if !groups.is_empty() {
            facets = Some(facets_for(&searcher, &query, &groups, facet_size)?);
        }
    }

    let hits = ids.iter().filter_map(|id| hit_of(state, *id)).collect();
    let meta = Meta {
        offset,
        search: text.clone(),
        search_type: "keyword".to_owned(),
        limit,
        total,
        export,
        structured,
    };
    info!(
        "search {text} export={export} total={total} offset={offset} limit={limit} elapsed={:.3}s",
        started.elapsed().as_secs_f32()
    );
    Ok(Outcome { hits, meta, facets })
}

/// Document ids of one page, in the requested order.
fn page_of(
    searcher: &tantivy::Searcher,
    query: &dyn tantivy::query::Query,
    handles: &Searcher,
    sort: Sort,
    limit: usize,
    offset: usize,
) -> tantivy::Result<IndexSet<u32>> {
    let top = TopDocs::with_limit(limit).and_offset(offset);
    let docs: Vec<DocAddress> = match sort {
        Sort::Relevance => searcher
            .search(query, &top.order_by_score())?
            .into_iter()
            .map(|(_score, addr)| addr)
            .collect(),
        Sort::Date => searcher
            .search(
                query,
                &top.order_by_fast_field::<i64>("judgment_days", Order::Desc),
            )?
            .into_iter()
            .map(|(_days, addr)| addr)
            .collect(),
        // Ascending on purpose: it pairs with the `id_gt` cursor, which is how
        // deep paging and export walk the whole set.
        Sort::Id => searcher
            .search(query, &top.order_by_fast_field::<u64>("id", Order::Asc))?
            .into_iter()
            .map(|(_id, addr)| addr)
            .collect(),
    };

    let mut ids = IndexSet::with_capacity(docs.len());
    for addr in docs {
        let doc: TantivyDocument = searcher.doc(addr)?;
        if let Some(value) = doc.get_first(handles.id).and_then(|v| v.as_u64()) {
            ids.insert(value as u32);
        }
    }
    Ok(ids)
}

/// Terms aggregations over the *same* query, so every bucket count describes the
/// filtered set rather than the whole library.
fn facets_for(
    searcher: &tantivy::Searcher,
    query: &dyn tantivy::query::Query,
    groups: &[String],
    size: usize,
) -> Result<Value, ApiError> {
    let request = aggs_json(groups, size).map_err(bad_request)?;
    let aggs: Aggregations = serde_json::from_value(request).map_err(bad_request)?;
    let collector = AggregationCollector::from_aggs(aggs, Default::default());
    let result = searcher
        .search(query, &collector)
        .map_err(|e| bad_request(e.to_string()))?;
    serde_json::to_value(result).map_err(internal)
}

fn decode_record(state: &AppState, id: u32) -> Option<CaseV2> {
    let bytes = state.records.get(id.to_be_bytes()).ok()??;
    bincode::decode_from_slice(&bytes, bincode::config::standard())
        .ok()
        .map(|(case, _)| case)
}

fn hit_of(state: &AppState, id: u32) -> Option<Hit> {
    let case = decode_record(state, id)?;
    Some(Hit {
        id,
        url: case.doc_url,
        case_id: case.case_id,
        case_name: case.case_name,
        court: case.court,
        region: case.region,
        case_type: case.case_type,
        procedure: case.procedure,
        judgment_date: case.judgment_date,
        public_date: case.public_date,
        parties: case.parties,
        cause: case.cause,
        legal_basis: case.legal_basis,
        preview: case.full_text.trim().chars().take(PREVIEW_CHARS).collect(),
    })
}

pub async fn search(
    Query(input): Query<SearchParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match run(&input, &state) {
        Ok(out) => Json(SearchResponse {
            search_meta: out.meta,
            cases: out.hits,
            facets: out.facets,
        })
        .into_response(),
        Err((code, message)) => {
            (code, Json(serde_json::json!({ "error": message }))).into_response()
        }
    }
}

/// Buckets instead of rows: the same query, no documents fetched.
pub async fn facets_only(
    Query(input): Query<SearchParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match run(&input, &state) {
        Ok(out) => Json(serde_json::json!({
            "search_meta": out.meta,
            "facets": out.facets.unwrap_or(Value::Null),
        }))
        .into_response(),
        Err((code, message)) => {
            (code, Json(serde_json::json!({ "error": message }))).into_response()
        }
    }
}

/// The whole row, 全文 included, exactly as it was imported. Escaping is the
/// front-end's job; the engine never renders markup.
pub async fn record(State(state): State<AppState>, Path(id): Path<u32>) -> impl IntoResponse {
    let Some(case) = decode_record(&state, id) else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    let derived = state
        .derived
        .get(id.to_be_bytes())
        .ok()
        .flatten()
        .and_then(|bytes| bincode::decode_from_slice(&bytes, bincode::config::standard()).ok())
        .map(|(derived, _)| derived);
    Json(Record {
        id,
        record: case,
        derived,
    })
    .into_response()
}

/// At most `export_limit` rows as a CSV download.
pub async fn export_csv(
    Query(params): Query<SearchParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let input = SearchParams {
        export: Some(true),
        ..params
    };
    let out = match run(&input, &state) {
        Ok(out) => out,
        Err((code, message)) => {
            return (code, Json(serde_json::json!({ "error": message }))).into_response();
        }
    };

    let mut writer = csv::Writer::from_writer(Vec::new());
    let _ = writer.write_record([
        "id",
        "原始链接",
        "案号",
        "案件名称",
        "法院",
        "案件类型",
        "审理程序",
        "裁判日期",
        "公开日期",
        "当事人",
        "案由",
        "法律依据",
        "全文",
    ]);
    for hit in &out.hits {
        if let Some(case) = decode_record(&state, hit.id) {
            let _ = writer.write_record([
                &hit.id.to_string(),
                &case.doc_url,
                &case.case_id,
                &case.case_name,
                &case.court,
                &case.case_type,
                &case.procedure,
                &case.judgment_date,
                &case.public_date,
                &case.parties,
                &case.cause,
                &case.legal_basis,
                &case.full_text,
            ]);
        }
    }

    (
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=localcases.csv",
            ),
        ],
        writer.into_inner().unwrap_or_default(),
    )
        .into_response()
}

/// Lets the desktop shell wait until the library and index are actually open.
pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let documents = state
        .searcher
        .reader
        .searcher()
        .search(&AllQuery, &Count)
        .unwrap_or_default();
    Json(serde_json::json!({ "ok": true, "indexed_documents": documents }))
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/api/search", get(search))
        .route("/api/facets", get(facets_only))
        .route("/api/case/{id}", get(record))
        .route("/api/export.csv", get(export_csv))
        .route("/api/health", get(health))
        .with_state(state)
}
