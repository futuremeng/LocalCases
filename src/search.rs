use std::collections::HashSet;

use tantivy::{
    Index, IndexReader, ReloadPolicy,
    directory::MmapDirectory,
    query::QueryParser,
    schema::{
        FAST, Field, INDEXED, IndexRecordOption, NumericOptions, STORED, Schema, TextFieldIndexing,
        TextOptions,
    },
    tokenizer::{RAW_TOKENIZER_NAME, RemoveLongFilter, StopWordFilter, TextAnalyzer},
};

use crate::CONFIG;

/// Name of the analyzer every text field is indexed and queried with.
const ANALYZER: &str = "jieba";
/// Longer tokens are not words (markup runs, base64, garbage), drop them.
const MAX_TOKEN_LEN: usize = 40;

/// Fields tokenized with jieba, for keyword search.
const TEXT_FIELDS: [&str; 11] = [
    "case_id",
    "case_name",
    "court",
    "case_type",
    "procedure",
    "judgment_date",
    "public_date",
    "parties",
    "cause",
    "legal_basis",
    "full_text",
];

/// Exact-value twins of some source columns: the raw tokenizer keeps the whole
/// value as a single term, so `court_s` answers "this court, exactly" and can be
/// aggregated. They sit next to the tokenized fields on purpose, which is why
/// adding them never changed keyword-search results.
const EXACT_FIELDS: [&str; 11] = [
    "court_s",
    "region_s",
    "cause_s",
    "case_type_s",
    "case_type_code_s",
    "procedure_s",
    "procedure_group_s",
    "procedure_kind_s",
    "source_s",
    "law_s",
    "court_code_s",
];

/// Runtime handles needed to query one library.
pub struct Searcher {
    pub reader: IndexReader,
    pub query_parser: QueryParser,
    /// the stored `id` field: hits are documents, records are keyed by id
    pub id: Field,
    /// Filtering/aggregation handles, `None` for an index without them.
    pub structured: Option<StructuredFields>,
}

/// Field handles of the exact-match layer.
pub struct StructuredFields {
    /// document id: fast field, so cursor paging and sorting skip the store
    pub id: Field,
    pub court: Field,
    pub region: Field,
    pub cause: Field,
    pub case_type: Field,
    pub case_type_code: Field,
    pub procedure: Field,
    pub procedure_group: Field,
    pub procedure_kind: Field,
    pub source: Field,
    pub law: Field,
    pub court_code: Field,

    pub year: Field,
    pub month: Field,
    pub day: Field,
    /// day number since 1970-01-01; -1 when unknown (kept, so it is countable)
    pub judgment_days: Field,
    pub public_days: Field,
    pub case_year: Field,
    pub case_seq: Field,
    pub party_count: Field,
    pub law_count: Field,
    pub text_len: Field,
    pub has_text: Field,
    /// the source 全文 contains markup the UI must escape rather than render
    pub has_html: Field,
}

fn build_stop_words() -> HashSet<String> {
    let mut words: HashSet<String> = include_str!("../stopwords.txt")
        .lines()
        .map(str::trim)
        .filter(|w| !w.is_empty() && !w.starts_with('#'))
        .map(str::to_owned)
        .collect();
    words.extend(
        stop_words::get(stop_words::Language::Chinese)
            .iter()
            .map(|w| w.to_string()),
    );
    words
}

pub struct Search;

impl Search {
    pub fn schema() -> Schema {
        let mut b = Schema::builder();

        let tokenized = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(ANALYZER)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        );
        // Positions are kept so phrase queries work inside long field values.

        // Stored so a hit resolves without a library lookup; fast so cursor
        // paging and id sorting are pure column access.
        b.add_u64_field("id", STORED | FAST);
        for name in TEXT_FIELDS {
            b.add_text_field(name, tokenized.clone());
        }

        let num = NumericOptions::default().set_indexed().set_fast();
        b.add_u64_field("year", num.clone());
        b.add_u64_field("month", num.clone());
        b.add_u64_field("day", num.clone());

        let exact = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer(RAW_TOKENIZER_NAME)
                    .set_index_option(IndexRecordOption::Basic),
            )
            .set_fast(RAW_TOKENIZER_NAME);
        for name in EXACT_FIELDS {
            b.add_text_field(name, exact.clone());
        }

        let signed = NumericOptions::default().set_indexed().set_fast();
        b.add_i64_field("judgment_days", signed.clone());
        b.add_i64_field("public_days", signed.clone());
        b.add_u64_field("case_year", num.clone());
        b.add_u64_field("case_seq", num.clone());
        b.add_u64_field("party_count", num.clone());
        b.add_u64_field("law_count", num.clone());
        b.add_u64_field("text_len", num);
        b.add_bool_field("has_text", FAST | INDEXED);
        b.add_bool_field("has_html", FAST | INDEXED);

        b.build()
    }

    /// Open (or create) the index at `index_path` and register the analyzer.
    /// Indexing and querying must agree on the tokenizer chain, so this is the
    /// only place it is built.
    pub fn index() -> tantivy::Result<Index> {
        let path = std::path::Path::new(CONFIG.index_path.as_str());
        std::fs::create_dir_all(path)?;
        let index = tantivy::Index::open_or_create(MmapDirectory::open(path)?, Self::schema())?;

        let analyzer = TextAnalyzer::builder(tantivy_jieba::JiebaTokenizer::new())
            .filter(StopWordFilter::remove(build_stop_words()))
            .filter(RemoveLongFilter::limit(MAX_TOKEN_LEN))
            .build();
        index.tokenizers().register(ANALYZER, analyzer);

        Ok(index)
    }

    /// Query-time view: the fields a bare keyword is looked up in, with the
    /// identifiers weighted above the prose.
    pub fn searcher() -> tantivy::Result<Searcher> {
        let schema = Self::schema();
        let field = |name: &str| schema.get_field(name);

        let id = field("id")?;
        let case_id = field("case_id")?;
        let case_name = field("case_name")?;

        let mut defaults = Vec::with_capacity(14);
        for name in [
            "case_id",
            "case_name",
            "court",
            "case_type",
            "cause",
            "legal_basis",
            "parties",
            "procedure",
            "judgment_date",
            "year",
            "month",
            "day",
            "public_date",
        ] {
            defaults.push(field(name)?);
        }
        if CONFIG.index_with_full_text {
            defaults.push(field("full_text")?);
        }

        let index = Self::index()?;
        let mut query_parser = QueryParser::for_index(&index, defaults);
        // "原告 被告" means both, not either; BM25 still decides the order.
        query_parser.set_conjunction_by_default();
        // A case number typed into the box should find that case first.
        query_parser.set_field_boost(case_id, 9.);
        query_parser.set_field_boost(case_name, 3.);

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        Ok(Searcher {
            reader,
            query_parser,
            id,
            structured: StructuredFields::from_schema(&schema),
        })
    }
}

impl StructuredFields {
    /// Resolve the handles, or `None` when the index predates the exact layer —
    /// half-resolving would silently turn filters into no-ops.
    pub fn from_schema(schema: &Schema) -> Option<StructuredFields> {
        let get = |name: &str| schema.get_field(name).ok();
        let missing = EXACT_FIELDS
            .iter()
            .copied()
            .chain(["judgment_days", "public_days", "case_year", "case_seq"])
            .any(|name| get(name).is_none());
        if missing {
            return None;
        }
        Some(StructuredFields {
            id: get("id")?,
            court: get("court_s")?,
            region: get("region_s")?,
            cause: get("cause_s")?,
            case_type: get("case_type_s")?,
            case_type_code: get("case_type_code_s")?,
            procedure: get("procedure_s")?,
            procedure_group: get("procedure_group_s")?,
            procedure_kind: get("procedure_kind_s")?,
            source: get("source_s")?,
            law: get("law_s")?,
            court_code: get("court_code_s")?,
            year: get("year")?,
            month: get("month")?,
            day: get("day")?,
            judgment_days: get("judgment_days")?,
            public_days: get("public_days")?,
            case_year: get("case_year")?,
            case_seq: get("case_seq")?,
            party_count: get("party_count")?,
            law_count: get("law_count")?,
            text_len: get("text_len")?,
            has_text: get("has_text")?,
            has_html: get("has_html")?,
        })
    }
}
