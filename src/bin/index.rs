//! v2 indexing: fjall_v2 (L0 + L1) -> tantivy schema v2.
//!
//! Builds the full-text fields exactly like schema v1 (same names, same jieba
//! analyzer, same boosts) *plus* raw+fast structured fields, so keyword result
//! counts stay comparable with the deployed index while filters and aggregations
//! become possible. Writes into a fresh directory (`index_path` in the config);
//! the deployed `search_index/` is never opened by this binary.
//!
//! Sourcing is parallel: ids are dense and gapless (`check` asserts it), so
//! the keyspace is cut into key ranges that readers walk independently. Both
//! partitions are keyed by the same id, so each reader zips `cases` with
//! `fields` instead of paying one point lookup per document.

use bincode::config::standard;
use fjall::Config;
use localcases::extract::{self, Derived};
use localcases::v2::*;
use localcases::{CONFIG, CaseV2, Search, record_partition_options};
use std::cmp::Ordering;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::mpsc::sync_channel;
use tantivy::TantivyDocument;
use tantivy::schema::Schema;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(not(target_os = "windows"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

type Field = tantivy::schema::Field;

/// Field handles the indexer needs.
struct Handles {
    id: Field,
    case_id: Field,
    case_name: Field,
    court: Field,
    case_type: Field,
    procedure: Field,
    judgment_date: Field,
    public_date: Field,
    parties: Field,
    cause: Field,
    legal_basis: Field,
    full_text: Field,
    year: Field,
    month: Field,
    day: Field,
    court_s: Field,
    region_s: Field,
    cause_s: Field,
    case_type_s: Field,
    case_type_code_s: Field,
    procedure_s: Field,
    procedure_group_s: Field,
    procedure_kind_s: Field,
    source_s: Field,
    law_s: Field,
    court_code_s: Field,
    judgment_days: Field,
    public_days: Field,
    case_year: Field,
    case_seq: Field,
    party_count: Field,
    law_count: Field,
    text_len: Field,
    has_text: Field,
    has_html: Field,
}

fn handles(schema: &Schema) -> Handles {
    let f = |name: &str| schema.get_field(name).expect("schema v2 field missing");
    Handles {
        id: f("id"),
        case_id: f("case_id"),
        case_name: f("case_name"),
        court: f("court"),
        case_type: f("case_type"),
        procedure: f("procedure"),
        judgment_date: f("judgment_date"),
        public_date: f("public_date"),
        parties: f("parties"),
        cause: f("cause"),
        legal_basis: f("legal_basis"),
        full_text: f("full_text"),
        year: f("year"),
        month: f("month"),
        day: f("day"),
        court_s: f("court_s"),
        region_s: f("region_s"),
        cause_s: f("cause_s"),
        case_type_s: f("case_type_s"),
        case_type_code_s: f("case_type_code_s"),
        procedure_s: f("procedure_s"),
        procedure_group_s: f("procedure_group_s"),
        procedure_kind_s: f("procedure_kind_s"),
        source_s: f("source_s"),
        law_s: f("law_s"),
        court_code_s: f("court_code_s"),
        judgment_days: f("judgment_days"),
        public_days: f("public_days"),
        case_year: f("case_year"),
        case_seq: f("case_seq"),
        party_count: f("party_count"),
        law_count: f("law_count"),
        text_len: f("text_len"),
        has_text: f("has_text"),
        has_html: f("has_html"),
    }
}

/// Build one document. Same field-by-field treatment for every reader thread,
/// and identical to what the serial version of this binary used to do.
fn build_doc(h: &Handles, id: u32, case: &CaseV2, derived: &Derived) -> TantivyDocument {
    let mut doc = TantivyDocument::default();
    doc.add_u64(h.id, id as u64);

    // ---- full-text layer: identical treatment to schema v1 ----
    // No HTML stripping here: 全文 is stored verbatim in the v2 keyspace.
    for (field, value) in [
        (h.case_id, case.case_id.as_str()),
        (h.case_name, case.case_name.as_str()),
        (h.court, case.court.as_str()),
        (h.case_type, case.case_type.as_str()),
        (h.procedure, case.procedure.as_str()),
        (h.judgment_date, case.judgment_date.as_str()),
        (h.public_date, case.public_date.as_str()),
        (h.parties, case.parties.as_str()),
        (h.cause, case.cause.as_str()),
        (h.legal_basis, case.legal_basis.as_str()),
    ] {
        if !value.is_empty() {
            doc.add_text(field, value);
        }
    }
    if CONFIG.index_with_full_text && derived.has_text {
        doc.add_text(h.full_text, case.full_text.trim());
    }

    // ---- v1-compatible numeric date parts ----
    let parts: Vec<&str> = case.judgment_date.split('-').collect();
    if let Some(y) = parts.first().and_then(|s| s.parse::<u64>().ok()) {
        doc.add_u64(h.year, y);
    }
    if let Some(m) = parts.get(1).and_then(|s| s.parse::<u64>().ok()) {
        doc.add_u64(h.month, m);
    }
    if let Some(d) = parts.get(2).and_then(|s| s.parse::<u64>().ok()) {
        doc.add_u64(h.day, d);
    }

    // ---- structured layer: exact terms + fast fields ----
    for (field, raw) in [
        (h.court_s, case.court.as_str()),
        (h.region_s, case.region.as_str()),
        (h.cause_s, case.cause.as_str()),
        (h.case_type_s, case.case_type.as_str()),
        (h.case_type_code_s, case.case_type_code.as_str()),
        (h.procedure_s, case.procedure.as_str()),
        (h.procedure_group_s, derived.procedure_group.as_str()),
        (h.procedure_kind_s, derived.procedure_kind.as_str()),
        (h.source_s, case.source.as_str()),
        (h.court_code_s, derived.court_code.as_str()),
    ] {
        // Index time trim keeps every stored term reachable through the query
        // string — the filter parser trims what it receives, so a value stored
        // with a stray leading newline (the corpus has a few, e.g. a 案件类型
        // cell that swallowed a line break) could never be filtered for, and
        // would show up as a mangled facet key. The keyspace keeps it verbatim.
        let value = raw.trim();
        if !value.is_empty() {
            doc.add_text(field, value);
        }
    }
    for law in extract::law_names(&case.legal_basis) {
        let law = law.trim();
        if !law.is_empty() {
            doc.add_text(h.law_s, law);
        }
    }

    // Days-since-epoch are written even when unknown (-1) so the unknown
    // bucket stays visible in histograms instead of silently vanishing.
    doc.add_i64(h.judgment_days, derived.judgment_days);
    doc.add_i64(h.public_days, derived.public_days);
    if derived.case_year != 0 {
        doc.add_u64(h.case_year, derived.case_year as u64);
    }
    if derived.case_seq != 0 {
        doc.add_u64(h.case_seq, derived.case_seq as u64);
    }
    doc.add_u64(h.party_count, derived.party_count as u64);
    doc.add_u64(h.law_count, derived.law_count as u64);
    doc.add_u64(h.text_len, derived.text_len as u64);
    doc.add_bool(h.has_text, derived.has_text);
    doc.add_bool(h.has_html, derived.has_html);
    doc
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!(
            "usage: index [config.toml] [--workers=N]\n\
             Builds the tantivy index (structured fields included) from the v2\n\
             keyspace. `--workers` sets how many threads scan the keyspace in key\n\
             ranges (writer threads are separate and come from the machine's\n\
             parallelism)."
        );
        return;
    }
    let readers = args
        .iter()
        .find_map(|a| a.strip_prefix("--workers="))
        .map(|v| v.parse::<usize>().expect("--workers=N"))
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .min(8)
        })
        .clamp(1, 32);

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            "info,tantivy=warn,html5ever=error",
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let schema = Search::schema();
    let h = Arc::new(handles(&schema));
    let index = Search::index().unwrap();
    // A much larger heap than v1's 50 MB: fewer, bigger segments, far less
    // merging work at 85M documents; ~1 GiB arena per writer thread.
    let writers = std::thread::available_parallelism()
        .map(|n| n.get().clamp(1, 8))
        .unwrap_or(4);
    let mut writer = index
        .writer_with_num_threads(writers, writers * 1_000_000_000)
        .unwrap();

    let keyspace = Config::new(&CONFIG.db)
        .max_write_buffer_size(512_000_000)
        .open()
        .unwrap();
    let cases = keyspace
        .open_partition(P_CASES, record_partition_options())
        .unwrap();
    let fields = keyspace
        .open_partition(P_FIELDS, record_partition_options())
        .unwrap();

    // Ids are dense and gapless (check asserts that), so the keyspace can be
    // cut into equal key ranges and read independently. Keys are big-endian u32,
    // which orders the same as the ids themselves.
    // KvPair is (key, value) — the value is the whole record, so take the first
    let (last_key, _) = cases
        .last_key_value()
        .unwrap()
        .expect("cases partition is empty");
    let max_id = u32::from_be_bytes(last_key.as_ref().try_into().unwrap());
    let ranges = readers * 16;
    let per = (max_id as u64 + 1).div_ceil(ranges as u64).max(1);
    info!(
        "ids 0..={max_id}: {readers} readers over {ranges} key ranges ({per} ids each), {writers} writer threads"
    );

    let next = Arc::new(AtomicUsize::new(0));
    // Bounded so the readers cannot run ahead of the writer and pile documents
    // up in memory; 512 docs per batch keeps the channel overhead low.
    let (tx, rx) = sync_channel::<Vec<TantivyDocument>>(readers * 3);
    let started = std::time::Instant::now();

    let scanners: Vec<_> = (0..readers)
        .map(|_| {
            let (cases, fields, h, tx, next) = (
                cases.clone(),
                fields.clone(),
                Arc::clone(&h),
                tx.clone(),
                Arc::clone(&next),
            );
            std::thread::spawn(move || {
                let mut seen: u64 = 0;
                loop {
                    let i = next.fetch_add(1, AtomicOrdering::Relaxed);
                    let lo = (i as u64) * per;
                    if i >= ranges || lo > max_id as u64 {
                        break;
                    }
                    let hi = (lo + per - 1).min(max_id as u64);
                    let (lo_k, hi_k) = ((lo as u32).to_be_bytes(), (hi as u32).to_be_bytes());

                    // `cases` and `fields` are keyed by the same id, so walk the
                    // second partition in lockstep: one sequential scan beats 85M
                    // point lookups (that was the bottleneck of the serial version).
                    let mut fit = fields.range(lo_k..=hi_k).map(|e| e.unwrap());
                    let mut f = fit.next();
                    let mut batch: Vec<TantivyDocument> = Vec::with_capacity(512);

                    for entry in cases.range(lo_k..=hi_k) {
                        let (k, v) = entry.unwrap();
                        let id = u32::from_be_bytes(k.as_ref().try_into().unwrap());
                        let mut derived: Option<Derived> = None;
                        while let Some((fk, fv)) = f.take() {
                            match fk.as_ref().cmp(k.as_ref()) {
                                Ordering::Equal => {
                                    derived = bincode::decode_from_slice(&fv, standard())
                                        .ok()
                                        .map(|d| d.0);
                                    f = fit.next();
                                    break;
                                }
                                // fields is ahead: this id has no derived record
                                Ordering::Greater => {
                                    f = Some((fk, fv));
                                    break;
                                }
                                // fields is behind: an entry this range does not own
                                Ordering::Less => {}
                            }
                        }
                        let derived = derived.unwrap_or_else(|| {
                            tracing::warn!("id {id}: derived record missing");
                            Derived::default()
                        });
                        let case: CaseV2 = bincode::decode_from_slice(&v, standard()).unwrap().0;
                        batch.push(build_doc(&h, id, &case, &derived));
                        seen += 1;
                        if batch.len() >= 512 && tx.send(std::mem::take(&mut batch)).is_err() {
                            return seen; // writer is gone
                        }
                    }
                    if !batch.is_empty() && tx.send(batch).is_err() {
                        break;
                    }
                }
                seen
            })
        })
        .collect();
    drop(tx);

    let mut count: u64 = 0;
    for batch in rx {
        let before = count;
        for doc in batch {
            writer.add_document(doc).unwrap();
            count += 1;
        }
        if count / 500_000 != before / 500_000 {
            writer.commit().unwrap();
            info!("{count} indexed, {}s", started.elapsed().as_secs());
        }
    }

    // A panicked reader would have closed its sender and ended the loop above,
    // so collect the results before declaring success.
    let scanned: Vec<u64> = scanners
        .into_iter()
        .map(|t| t.join().expect("a reader thread panicked"))
        .collect();
    let by_scan: u64 = scanned.iter().sum();
    if by_scan != count {
        panic!("reader/writer disagreement: scanned {by_scan}, indexed {count}");
    }

    writer.commit().unwrap();
    let reader = index.reader().unwrap();
    info!(
        "Total {count} documents ({readers} readers), {} segments, {}s",
        reader.searcher().segment_readers().len(),
        started.elapsed().as_secs()
    );
}
