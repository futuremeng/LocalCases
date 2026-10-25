//! v2 conversion: raw CSV rows -> immutable L0 records + derived L1 records.
//!
//! Differences from `convert` (v1), all of them correctness fixes:
//! - ids come from an `ids` partition keyed by the document's `docId`, not from
//!   `fs::read_dir` traversal order, so ids are stable across runs and adding a
//!   zip later never renumbers existing documents;
//! - duplicate `docId`s are detected and skipped instead of silently becoming
//!   separate documents;
//! - `全文` is stored verbatim (v1 wrapped every whitespace token in `<p>…</p>`,
//!   which is why the text layer had to be un-tagged again at query time);
//! - all 15 CSV columns are kept (v1 dropped 所属地区 / 案件类型编码 / 来源);
//! - progress is checkpointed per CSV member inside the same atomic batch as the
//!   data, so an interrupted run resumes exactly where it stopped;
//! - per-member manifest rows and data-quality counters are recorded.
//!
//! Reading is parallel (one thread per CSV member: zip inflate + CSV decode +
//! L1 derivation, fed through bounded channels), but ids are handed out and
//! batches committed by a single writer thread that consumes members and rows in
//! the original traversal order — so a parallel run produces exactly what a
//! serial one did, `compare` checks that byte-for-byte. That split matches
//! where the time actually goes: on the 1.54 GB sample the reader side is ~15 s
//! of wall time while fjall commits are ~44 s, so parallel readers alone buy
//! nothing; the write path (`--wbs-mb` / `--memtable-mb` / `--journal-mb` /
//! `--io-workers`) is what moves the number, 64 s -> 37 s.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use bincode::config::standard;
use bincode::{Decode, Encode};
use fjall::{Config, PartitionCreateOptions};
use localcases::extract;
use localcases::v2::*;
use localcases::{CONFIG, CaseV2, record_partition_options};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(not(target_os = "windows"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

/// Resume checkpoint + manifest entry for one CSV member.
#[derive(Debug, Clone, Default, Encode, Decode)]
struct MemberProgress {
    /// data rows consumed (never ahead of a committed batch)
    rows_done: u64,
    /// ids handed out to this member's kept records, in traversal order
    first_id: u32,
    last_id: u32,
    /// Counters for this member. They live inside the checkpoint, so they are
    /// only ever persisted together with the `rows_done` they describe: a crash
    /// rolls both back, a resume re-adds exactly what it replays.
    qual: Quality,
}

/// Data-quality counters, rolled up per source year for the report.
#[derive(Debug, Clone, Default, Encode, Decode)]
struct Quality {
    /// rows the reader handed us, including undecodable ones
    rows_seen: u64,
    /// rows successfully decoded
    rows: u64,
    csv_errors: u64,
    kept: u64,
    duplicates: u64,
    empty_text: u64,
    html_text: u64,
    bad_judgment_date: u64,
    bad_public_date: u64,
    no_case_year: u64,
    year_mismatch: u64,
    with_law_ref: u64,
    text_chars: u64,
    /// rows whose 原始链接 carries no usable docId at all
    no_doc_id: u64,
}

fn add_quality(a: &mut Quality, b: &Quality) {
    a.rows_seen += b.rows_seen;
    a.rows += b.rows;
    a.csv_errors += b.csv_errors;
    a.kept += b.kept;
    a.duplicates += b.duplicates;
    a.empty_text += b.empty_text;
    a.html_text += b.html_text;
    a.bad_judgment_date += b.bad_judgment_date;
    a.bad_public_date += b.bad_public_date;
    a.no_case_year += b.no_case_year;
    a.year_mismatch += b.year_mismatch;
    a.with_law_ref += b.with_law_ref;
    a.text_chars += b.text_chars;
    a.no_doc_id += b.no_doc_id;
}

/// One CSV member to convert, in the order the writer must consume it.
struct Member {
    zip_path: PathBuf,
    member: String,
    /// `<zip name>/<member name>` — the progress-checkpoint key.
    key: String,
}

/// A CSV row already decoded and L1-derived by a reader thread.
struct Parsed {
    /// 1-based row number inside the member, as the resume checkpoint uses it.
    row_no: u64,
    /// Identity key, already falling back to a positional one when the source
    /// link carries no docId.
    doc_key: String,
    case: CaseV2,
    derived: extract::Derived,
    /// true when 原始链接 carried no docId and `doc_key` is positional
    positional: bool,
}

/// One ordered group of events from a single member. Counters deliberately are
/// *not* aggregated per chunk: the writer advances its checkpoint row by row, so
/// a checkpoint must never describe rows it has not replayed yet. Aggregating
/// here made a resumed run count the rows of a partially processed chunk twice
/// (measured: +79 rows_seen on an interrupted member).
#[derive(Default)]
struct Chunk {
    events: Vec<Event>,
}

/// A row, or a row the CSV reader could not decode. Both consume a row number.
/// The row is boxed: a decoded record is a kilobyte of strings, an error is a
/// counter, and a chunk of errors should not pay for the largest variant.
enum Event {
    Row(Box<Parsed>),
    CsvError { row_no: u64 },
}

/// Rows per channel message, and how many messages a reader may have in flight.
/// Together they bound memory: `workers * CHANNEL_CHUNKS * CHUNK_ROWS` rows.
const CHUNK_ROWS: usize = 256;
const CHANNEL_CHUNKS: usize = 8;

/// The source columns we know, with the header spellings we accept for each.
/// Users import their own corpora, whose headers are rarely the exact 15 Chinese
/// names of the export this tool grew up on: an English or renamed column is
/// matched here, a column that is simply absent stays empty, an unrecognised
/// column is ignored. The order matches `CaseV2`'s field order.
const COLUMNS: [(&str, &[&str]); 15] = [
    (
        "原始链接",
        &["原始链接", "url", "link", "doc_url", "docid", "链接"],
    ),
    ("案号", &["案号", "case_id", "case_number", "caseNo"]),
    ("案件名称", &["案件名称", "case_name", "title", "标题"]),
    ("法院", &["法院", "court", "court_name", "审理法院"]),
    ("所属地区", &["所属地区", "region", "area", "地区"]),
    ("案件类型", &["案件类型", "case_type", "type"]),
    ("案件类型编码", &["案件类型编码", "case_type_code"]),
    ("来源", &["来源", "source"]),
    ("审理程序", &["审理程序", "procedure", "trial_procedure"]),
    ("裁判日期", &["裁判日期", "judgment_date", "date"]),
    ("公开日期", &["公开日期", "public_date", "publish_date"]),
    ("当事人", &["当事人", "parties", "party"]),
    ("案由", &["案由", "cause", "reason"]),
    ("法律依据", &["法律依据", "legal_basis", "laws"]),
    ("全文", &["全文", "full_text", "text", "正文", "content"]),
];

/// Which source column sits at which CSV field, worked out once per member.
struct ColumnMap {
    at: [Option<usize>; 15],
    /// how many known columns the header actually named
    matched: usize,
}

impl ColumnMap {
    fn of(headers: &csv::StringRecord) -> ColumnMap {
        let names: Vec<String> = headers
            .iter()
            .map(|h| h.trim().trim_start_matches('\u{feff}').to_owned())
            .collect();
        let mut at = [None; 15];
        let mut matched = 0;
        for (i, (_, aliases)) in COLUMNS.iter().enumerate() {
            // Exact spelling first, then a case-insensitive alias match, so the
            // Chinese name wins when a file happens to carry both spellings.
            let mut found = None;
            for alias in *aliases {
                found = names.iter().position(|h| h == alias);
                if found.is_some() {
                    break;
                }
                found = names
                    .iter()
                    .position(|h| h.to_lowercase() == alias.to_lowercase());
                if found.is_some() {
                    break;
                }
            }
            if found.is_some() {
                matched += 1;
            }
            at[i] = found;
        }
        ColumnMap { at, matched }
    }

    fn row(&self, record: &csv::StringRecord) -> CaseV2 {
        // Values are kept verbatim: the stored row must be byte-identical to what
        // the source file says, spaces included. Only headers are normalized.
        let take = |i: usize| -> String {
            self.at[i]
                .and_then(|idx| record.get(idx))
                .unwrap_or_default()
                .to_owned()
        };
        CaseV2 {
            doc_url: take(0),
            case_id: take(1),
            case_name: take(2),
            court: take(3),
            region: take(4),
            case_type: take(5),
            case_type_code: take(6),
            source: take(7),
            procedure: take(8),
            judgment_date: take(9),
            public_date: take(10),
            parties: take(11),
            cause: take(12),
            legal_basis: take(13),
            full_text: take(14),
        }
    }
}

/// Decode one CSV member (zip inflate + CSV parse + L1 derivation) and stream
/// the rows to the writer in file order.
///
/// Deliberately keyspace-free: duplicate detection and id assignment stay in the
/// writer, so running several of these in parallel cannot change which row gets
/// which id. `skip_rows` is the checkpointed row count, so a resumed member
/// replays nothing.
fn read_member(
    zip_path: &Path,
    member: &str,
    member_key: &str,
    skip_rows: u64,
    tx: &mpsc::SyncSender<Chunk>,
) {
    let file = fs::File::open(zip_path).unwrap();
    if member.is_empty() {
        // A loose CSV sitting next to the archives: the file *is* the member.
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(BufReader::new(file));
        stream(&mut rdr, member_key, skip_rows, tx);
        return;
    }
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let entry = archive.by_name(member).unwrap();
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(entry);
    stream(&mut rdr, member_key, skip_rows, tx);
}

/// Parse + derive + stream from any CSV source. Members are gigabytes in real
/// corpora, so nothing here is read into memory first.
fn stream<R: io::Read>(
    rdr: &mut csv::Reader<R>,
    member_key: &str,
    skip_rows: u64,
    tx: &mpsc::SyncSender<Chunk>,
) {
    // Headers are resolved once up front, so a member whose columns are not
    // recognised fails loudly instead of quietly yielding a member of empty rows.
    let headers = rdr.headers().unwrap().clone();
    let columns = ColumnMap::of(&headers);
    if columns.matched == 0 {
        let seen: Vec<&str> = headers.iter().collect();
        error!(
            "{member_key}: no recognised column in {seen:?}; expected any of {:?}",
            COLUMNS.map(|(name, _)| name)
        );
        return;
    }

    let mut chunk = Chunk::default();
    let mut row: u64 = 0;
    for record in rdr.records() {
        row += 1;
        if row <= skip_rows {
            continue;
        }
        match record {
            Ok(record) => {
                let case = columns.row(&record);
                // Stable identity first: an already-seen docId keeps its original
                // id and the repeated row is dropped instead of duplicated. Rows
                // with no usable docId get a positional key (unique per source
                // row) — fjall rejects empty keys outright.
                let positional = case.doc_key().is_empty();
                let doc_key = if positional {
                    format!("row/{member_key}#{row}")
                } else {
                    case.doc_key().to_string()
                };
                let derived = extract::derive(&case);
                chunk.events.push(Event::Row(Box::new(Parsed {
                    row_no: row,
                    doc_key,
                    case,
                    derived,
                    positional,
                })));
            }
            Err(err) => {
                warn!("{member_key} row {row}: csv error {err}");
                chunk.events.push(Event::CsvError { row_no: row });
            }
        }
        if chunk.events.len() >= CHUNK_ROWS && tx.send(std::mem::take(&mut chunk)).is_err() {
            // The writer is gone; no point decompressing the rest.
            return;
        }
    }
    if !chunk.events.is_empty() {
        let _ = tx.send(chunk);
    }
}

fn main() {
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        eprintln!(
            "usage: convert [config.toml] [--workers=N] [--batch=N]\n\
             [--wbs-mb=N] [--io-workers=N] [--compact]\n\
             Converts the raw CSV export into the v2 keyspace (L0 records, L1\n\
             derived records, stable ids, manifest, quality counters). Safe to\n\
             re-run: progress is checkpointed per CSV member. `--workers` sets how\n\
             many CSV members are decompressed and derived at once; ids are\n\
             handed out by a single writer in the original traversal order, so the\n\
             result is identical to a serial run. `--batch` is the rows per\n\
             committed batch (= crash-replay unit), `--wbs-mb` the fjall write\n\
             buffer cap, `--io-workers` the flush/compaction threads.\n\
             `--compact` merges the LSM trees afterwards (re-running with just\n\
             `--compact` compacts an already converted keyspace)."
        );
        return;
    }

    let opts = opts();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info,fjall=warn"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    convert(
        CONFIG.raw_data_path.as_deref().unwrap(),
        CONFIG.db.as_str(),
        &opts,
    );
}

/// Tuning knobs for a bulk conversion. The defaults are the safe ones; the
/// flags exist because a full-corpus run costs an hour and the right values
/// depend on the machine it runs on. None of them change the result.
struct Opts {
    /// CSV members decompressed + derived at once
    workers: usize,
    /// rows per committed fjall batch — also the crash-replay unit
    batch_rows: usize,
    /// fjall write-buffer cap in MB: fewer flushes vs. more RAM
    wbs_mb: u64,
    /// fjall flush + compaction threads; `None` keeps the fjall default
    io_workers: Option<usize>,
    /// per-partition memtable size in MB for the record partitions
    memtable_mb: u64,
    /// fjall journal rotation threshold in MB
    journal_mb: u64,
    /// merge the LSM trees once the conversion is done. The bulk-load settings
    /// are chosen for write throughput and leave the tree shallow but wide:
    /// the 1.54 GB sample is 7.2 GB on disk right after a run vs 2.2 GB with
    /// fjall's own settings. A major compaction settles that and pays for the
    /// writes the bulk settings avoided.
    compact: bool,
    /// diagnostic only: plain LSM partitions instead of key-value separation,
    /// to tell blob-log cost from journal cost. Changes the on-disk layout.
    diagnostic_no_kv_sep: bool,
}

fn flag<T: std::str::FromStr>(name: &str) -> Option<T>
where
    T::Err: std::fmt::Debug,
{
    std::env::args().find_map(|a| {
        a.strip_prefix(name)
            .map(|v| v.parse::<T>().unwrap_or_else(|e| panic!("{name}=N: {e:?}")))
    })
}

fn opts() -> Opts {
    Opts {
        workers: flag("--workers=").unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get().min(8))
                .unwrap_or(1)
        }),
        batch_rows: flag("--batch=").unwrap_or(10_000),
        wbs_mb: flag("--wbs-mb=").unwrap_or(2_000),
        memtable_mb: flag("--memtable-mb=").unwrap_or(512),
        journal_mb: flag("--journal-mb=").unwrap_or(16_000),
        io_workers: flag("--io-workers=").or_else(|| {
            // fjall's default is 4 flush + 4 compaction threads, which is what
            // stalls a bulk load: journal rotation at 512 MB plus 128 MB
            // memtables cost ~26 s of the 61 s sample run. Measured on the
            // 1.54 GB sample: 61 s -> 37 s at ~2.5 GB RSS.
            Some(
                std::thread::available_parallelism()
                    .map(|n| n.get().saturating_sub(1).min(12))
                    .unwrap_or(8),
            )
        }),
        compact: std::env::args().any(|a| a == "--compact"),
        diagnostic_no_kv_sep: std::env::args().any(|a| a == "--no-kv-sep"),
    }
}

fn convert(raw_path: &str, db_path: &str, opts: &Opts) {
    let started = std::time::Instant::now();

    let mut config = Config::new(db_path)
        .max_write_buffer_size(opts.wbs_mb * 1_000_000)
        .max_journaling_size(opts.journal_mb * 1_000_000);
    if let Some(n) = opts.io_workers {
        config = config.flush_workers(n).compaction_workers(n);
    }
    let keyspace = config.open().unwrap();
    let record_opts = if opts.diagnostic_no_kv_sep {
        PartitionCreateOptions::default()
    } else {
        record_partition_options()
    };
    let record_opts = record_opts
        .max_memtable_size(u32::try_from(opts.memtable_mb * 1_000_000).unwrap_or(u32::MAX));
    let cases = keyspace
        .open_partition(P_CASES, record_opts.clone())
        .unwrap();
    let fields = keyspace.open_partition(P_FIELDS, record_opts).unwrap();
    let ids = keyspace
        .open_partition(P_IDS, PartitionCreateOptions::default())
        .unwrap();
    let progress = keyspace
        .open_partition(P_PROGRESS, PartitionCreateOptions::default())
        .unwrap();
    let stats = keyspace
        .open_partition(P_STATS, PartitionCreateOptions::default())
        .unwrap();

    // Ids are monotonic, so the highest id stored in `cases` is the resume
    // point. Deriving it from the partition rather than only from `stats`
    // matters: `stats/next_id` is written when a run *finishes*, so a run that
    // is killed mid-way would otherwise restart at 1 and silently overwrite the
    // documents it already wrote.
    let stored_next_id = match stats.get(b"next_id").unwrap() {
        Some(v) => v.as_ref().try_into().ok().map(u32::from_be_bytes),
        None => None,
    };
    let highest_id = match cases.keys().next_back() {
        Some(Ok(k)) => k.as_ref().try_into().ok().map(u32::from_be_bytes),
        Some(Err(err)) => {
            warn!("cannot read the highest id from {P_CASES}: {err}");
            None
        }
        None => None,
    };
    let mut next_id = highest_id.map(|h| h + 1).unwrap_or(1);
    match stored_next_id {
        Some(s) if s == next_id => info!("resuming at id {next_id}"),
        Some(s) => warn!(
            "stats/next_id {s} disagrees with the stored ids (would use {next_id}); \
             using the stored ids to avoid re-issuing them"
        ),
        None if next_id > 1 => warn!("no stats/next_id checkpoint; resuming at id {next_id}"),
        None => info!("starting at id 1"),
    }

    // Deterministic traversal: sorted zip names, sorted member names within each
    // zip, then the loose CSV files a user dropped in. v1 used whatever order the
    // directory happened to return. The plan is built up front so reader threads
    // can run ahead of the writer while the writer still consumes members — and
    // rows — in exactly this order, which is what keeps ids identical to a serial
    // run.
    let mut plan: Vec<Member> = Vec::new();
    let entries: Vec<PathBuf> = {
        let mut all: Vec<PathBuf> = fs::read_dir(raw_path)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        all.sort();
        all
    };
    let mut zips = 0usize;
    let mut loose = 0usize;
    for path in &entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let text = path.to_string_lossy().to_string();
        if text.ends_with(".zip") {
            zips += 1;
            let file = fs::File::open(path).unwrap();
            let mut archive = zip::ZipArchive::new(file).unwrap();
            let mut members: Vec<String> = Vec::new();
            for i in 0..archive.len() {
                let entry = archive.by_index(i).unwrap();
                // `zip` 9.x: `name()` returns `Result<Cow<'_, str>, ZipError>`.
                match entry.name() {
                    Ok(entry) if entry.ends_with(".csv") => members.push(entry.to_string()),
                    Ok(_) => {}
                    Err(err) => warn!("{name}: skipping zip entry {i}: {err}"),
                }
            }
            members.sort();
            for member in members {
                let key = format!("{name}/{member}");
                plan.push(Member {
                    zip_path: path.clone(),
                    member,
                    key,
                });
            }
        } else if text.ends_with(".csv") {
            // A user's own export, straight from a spreadsheet: no archive around
            // it. `member` empty means "the file itself is the member".
            loose += 1;
            plan.push(Member {
                zip_path: path.clone(),
                member: String::new(),
                key: name,
            });
        }
    }
    info!(
        "{} zip archives + {} loose csv, {} csv member(s) to read, {} reader(s), batch {}, {} MB write buffer,          {} MB memtable, {} MB journal, io_workers {:?}",
        zips,
        loose,
        plan.len(),
        opts.workers,
        opts.batch_rows,
        opts.wbs_mb,
        opts.memtable_mb,
        opts.journal_mb,
        opts.io_workers
    );

    // Sliding window: at most `workers` members are decompressed at a time, each
    // feeding a bounded channel, so memory stays flat however many members are
    // left. A reader that dies leaves its channel short; the join below turns
    // that into a hard stop rather than a silently truncated member.
    let slots = opts.workers.min(plan.len()).max(1);
    let mut window: Vec<Option<(mpsc::Receiver<Chunk>, thread::JoinHandle<()>)>> =
        (0..slots).map(|_| None).collect();
    let mut next_spawn = 0usize;

    /// Where the single writer thread spends its time. Worth measuring: the
    /// reader side parallelises for free, the writer side does not.
    #[derive(Default)]
    struct Spent {
        wait: std::time::Duration,
        dedup: std::time::Duration,
        insert: std::time::Duration,
        commit: std::time::Duration,
    }
    let mut spent = Spent::default();

    for (i, planned) in plan.iter().enumerate() {
        while next_spawn < plan.len() && next_spawn < i + slots {
            let p = &plan[next_spawn];
            let skip = match progress.get(p.key.as_bytes()).unwrap() {
                Some(v) => {
                    bincode::decode_from_slice::<MemberProgress, _>(v.as_ref(), standard())
                        .unwrap()
                        .0
                        .rows_done
                }
                None => 0,
            };
            let (tx, rx) = mpsc::sync_channel(CHANNEL_CHUNKS);
            let (zip_path, member, key) = (p.zip_path.clone(), p.member.clone(), p.key.clone());
            let handle = thread::spawn(move || read_member(&zip_path, &member, &key, skip, &tx));
            window[next_spawn % slots] = Some((rx, handle));
            next_spawn += 1;
        }

        let member_key = planned.key.clone();
        let (rx, reader) = window[i % slots].take().unwrap();
        let mut prog = match progress.get(member_key.as_bytes()).unwrap() {
            Some(v) => {
                bincode::decode_from_slice::<MemberProgress, _>(v.as_ref(), standard())
                    .unwrap()
                    .0
            }
            None => MemberProgress::default(),
        };

        let mut batch = keyspace.batch();
        let mut pending: u64 = 0;
        // Keys inserted into the still-open batch are invisible to `ids.get`,
        // so duplicates inside a batch window have to be caught here — the
        // corpus really does contain adjacent identical rows.
        let mut batch_keys: HashSet<String> = HashSet::new();

        // Explicit recv loop, so the time spent waiting for the next chunk can
        // be measured: that share is what the reader threads could win back.
        let mut rx_iter = rx.into_iter();
        loop {
            let t0 = std::time::Instant::now();
            let Some(chunk) = rx_iter.next() else { break };
            spent.wait += t0.elapsed();

            for event in chunk.events {
                let (row_no, doc_key, case, derived, positional) = match event {
                    Event::CsvError { row_no } => {
                        prog.rows_done = row_no;
                        prog.qual.rows_seen += 1;
                        prog.qual.csv_errors += 1;
                        continue;
                    }
                    Event::Row(parsed) => (
                        parsed.row_no,
                        parsed.doc_key,
                        parsed.case,
                        parsed.derived,
                        parsed.positional,
                    ),
                };
                // The checkpoint advances over *consumed* rows, not only kept ones,
                // and the counters advance with it: a row that was already seen
                // and skipped (duplicate, csv error) must not be replayed and
                // counted twice after a crash. Every row up to here is either
                // inserted in this batch or produced no data at all, so the
                // checkpoint can never run ahead of its rows.
                prog.rows_done = row_no;
                prog.qual.rows_seen += 1;
                prog.qual.rows += 1;
                if positional {
                    prog.qual.no_doc_id += 1;
                }
                let t0 = std::time::Instant::now();
                let known = ids.get(doc_key.as_bytes()).unwrap().is_some();
                spent.dedup += t0.elapsed();
                if known || batch_keys.contains(&doc_key) {
                    prog.qual.duplicates += 1;
                    continue;
                }

                // Quality counters describe the documents that are actually kept.
                if derived.has_text {
                    prog.qual.text_chars += derived.text_len as u64;
                } else {
                    prog.qual.empty_text += 1;
                }
                if derived.has_html {
                    prog.qual.html_text += 1;
                }
                if derived.judgment_days < 0 {
                    prog.qual.bad_judgment_date += 1;
                }
                if derived.public_days < 0 {
                    prog.qual.bad_public_date += 1;
                }
                if derived.case_year == 0 {
                    prog.qual.no_case_year += 1;
                } else if let Some(jy) = case
                    .judgment_date
                    .get(..4)
                    .and_then(|s| s.parse::<u16>().ok())
                    && jy != derived.case_year
                {
                    prog.qual.year_mismatch += 1;
                }
                if derived.law_count > 0 {
                    prog.qual.with_law_ref += 1;
                }

                let id = next_id;
                next_id += 1;
                batch.insert(&ids, doc_key.as_bytes(), id.to_be_bytes());
                batch_keys.insert(doc_key.clone());

                batch.insert(
                    &cases,
                    id.to_be_bytes(),
                    bincode::encode_to_vec(&case, standard()).unwrap(),
                );
                let t0 = std::time::Instant::now();
                batch.insert(
                    &fields,
                    id.to_be_bytes(),
                    bincode::encode_to_vec(&derived, standard()).unwrap(),
                );
                spent.insert += t0.elapsed();

                if prog.first_id == 0 {
                    prog.first_id = id;
                }
                prog.last_id = id;
                pending += 1;
                prog.qual.kept += 1;

                if pending >= opts.batch_rows as u64 {
                    // The checkpoint travels in the same atomic batch as the rows
                    // it describes: a crash replays nothing and loses nothing.
                    batch.insert(
                        &progress,
                        member_key.as_bytes(),
                        bincode::encode_to_vec(&prog, standard()).unwrap(),
                    );
                    let t0 = std::time::Instant::now();
                    batch.commit().unwrap();
                    spent.commit += t0.elapsed();
                    pending = 0;
                    // Committed keys are now visible through `ids.get`.
                    batch_keys.clear();
                    batch = keyspace.batch();
                }
            }
        }

        // A reader that panicked leaves its channel short: stop here rather than
        // checkpoint a member that was only half converted. The atomic batches
        // above mean re-running resumes at the last committed row.
        if reader.join().is_err() {
            panic!("reader for {member_key} failed; re-run to resume where it stopped");
        }

        // Flush whatever is left, checkpoint included.
        batch.insert(
            &progress,
            member_key.as_bytes(),
            bincode::encode_to_vec(&prog, standard()).unwrap(),
        );
        batch.commit().unwrap();

        info!(
            "{member_key}: rows {} kept {} dup {} empty_text {} ({}s)",
            prog.rows_done,
            prog.qual.kept,
            prog.qual.duplicates,
            prog.qual.empty_text,
            started.elapsed().as_secs()
        );
    }

    // Roll the per-member checkpoints up into the report. Reading them back from
    // the keyspace instead of an in-memory map keeps the numbers complete even
    // when a run was interrupted and resumed.
    let mut per_year: BTreeMap<String, Quality> = BTreeMap::new();
    let mut total = Quality::default();
    let mut members = 0u64;
    for entry in progress.iter() {
        // fjall 2.x iterates over (key, value) pairs.
        let (key, value) = entry.unwrap();
        let key = String::from_utf8_lossy(key.as_ref()).to_string();
        let prog = bincode::decode_from_slice::<MemberProgress, _>(value.as_ref(), standard())
            .unwrap()
            .0;
        let year = key
            .split('/')
            .next()
            .map(year_of)
            .unwrap_or_else(|| "unknown".to_string());
        add_quality(per_year.entry(year).or_default(), &prog.qual);
        add_quality(&mut total, &prog.qual);
        members += 1;
    }

    let mut batch = keyspace.batch();
    batch.insert(&stats, b"next_id", next_id.to_be_bytes());
    for (year, q) in &per_year {
        batch.insert(
            &stats,
            format!("year/{year}").as_bytes(),
            bincode::encode_to_vec(q, standard()).unwrap(),
        );
    }
    batch.insert(
        &stats,
        b"total",
        bincode::encode_to_vec(&total, standard()).unwrap(),
    );
    batch.commit().unwrap();

    for (year, q) in &per_year {
        info!(
            "year {year}: rows {} kept {} dup {} empty_text {} html {} no_doc_id {} \
             no_case_year {} law_ref {} chars {}",
            q.rows,
            q.kept,
            q.duplicates,
            q.empty_text,
            q.html_text,
            q.no_doc_id,
            q.no_case_year,
            q.with_law_ref,
            q.text_chars
        );
    }
    info!(
        "DONE members {members} rows {} parsed {} csv_errors {} kept {} dup {} no_doc_id {} \
         empty_text {} html {} bad_jdate {} bad_pdate {} no_case_year {} year_mismatch {} \
         law_ref {} chars {} next_id {next_id} in {}s",
        total.rows,
        total.rows_seen,
        total.csv_errors,
        total.kept,
        total.duplicates,
        total.no_doc_id,
        total.empty_text,
        total.html_text,
        total.bad_judgment_date,
        total.bad_public_date,
        total.no_case_year,
        total.year_mismatch,
        total.with_law_ref,
        total.text_chars,
        started.elapsed().as_secs()
    );
    if opts.compact {
        // Runs after everything is written, so the tree ends up as compact as a
        // run with fjall's own (slow) settings. Re-running with only `--compact`
        // is fine: the conversion part is a no-op once every member is done.
        for (name, part) in [
            ("cases", &cases),
            ("fields", &fields),
            ("ids", &ids),
            ("progress", &progress),
            ("stats", &stats),
        ] {
            let t0 = std::time::Instant::now();
            part.major_compact().unwrap();
            info!("major_compact {name}: {:.1}s", t0.elapsed().as_secs_f64());
        }
    }

    let total_s = started.elapsed().as_secs_f64();
    info!(
        "writer split of {total_s:.1}s: wait_for_reader {:.1} dedup_lookup {:.1} insert {:.1} commit {:.1}",
        spent.wait.as_secs_f64(),
        spent.dedup.as_secs_f64(),
        spent.insert.as_secs_f64(),
        spent.commit.as_secs_f64()
    );
}

/// `2013年.zip` / `裁判文书2020年(1).zip` -> `2020`; unparseable -> `unknown`.
fn year_of(zip_name: &str) -> String {
    let digits: String = zip_name.chars().filter(|c| c.is_ascii_digit()).collect();
    match digits.get(..4) {
        Some(y) if y.len() == 4 => y.to_string(),
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::year_of;

    #[test]
    fn extracts_zip_years() {
        assert_eq!(year_of("2013年.zip"), "2013");
        assert_eq!(year_of("裁判文书2020年(1).zip"), "2020");
        assert_eq!(year_of("no-year.zip"), "unknown");
    }
}

#[cfg(test)]
mod column_tests {
    use super::*;

    /// Headers built from their own CSV line, which is also what exercises the
    /// real parser (BOM, padding) rather than a hand-made record.
    fn headers(names: &[&str]) -> csv::StringRecord {
        let line = names.join(",");
        let mut rdr = csv::ReaderBuilder::new().from_reader(line.as_bytes());
        rdr.headers().unwrap().clone()
    }

    fn record(fields: &[&str]) -> csv::StringRecord {
        let line = fields.join(",");
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(line.as_bytes());
        rdr.records().next().unwrap().unwrap()
    }

    #[test]
    fn the_source_export_header_maps_every_column() {
        let m = ColumnMap::of(&headers(&[
            "原始链接",
            "案号",
            "案件名称",
            "法院",
            "所属地区",
            "案件类型",
            "案件类型编码",
            "来源",
            "审理程序",
            "裁判日期",
            "公开日期",
            "当事人",
            "案由",
            "法律依据",
            "全文",
        ]));
        assert_eq!(m.matched, 15);
        // each source column sits at the CSV field with the same index
        for i in 0..15 {
            assert_eq!(m.at[i], Some(i), "column {i}");
        }
    }

    #[test]
    fn english_headers_and_a_missing_column_still_import() {
        let m = ColumnMap::of(&headers(&[
            "url",
            "case_number",
            "title",
            "court",
            "region",
            "case_type",
            "procedure",
            "date",
            "publish_date",
            "party",
            "cause",
            "laws",
            "text",
        ]));
        // 案件类型编码 and 来源 are simply absent, everything else is found.
        assert_eq!(m.matched, 13);
        assert_eq!(m.at[6], None);
        assert_eq!(m.at[7], None);
        let record = record(&[
            "http://x?docId=a",
            "(2019)京0105民初1号",
            "title",
            "court",
            "region",
            "type",
            "一审",
            "2019-06-14",
            "2019-08-01",
            "party",
            "cause",
            "law",
            "text",
        ]);
        let case = m.row(&record);
        assert_eq!(case.case_id, "(2019)京0105民初1号");
        assert_eq!(case.source, "");
        assert_eq!(case.full_text, "text");
    }

    #[test]
    fn a_spreadsheet_bom_and_padded_names_are_tolerated() {
        let m = ColumnMap::of(&headers(&["\u{feff} 案号 ", " 全文", "备注"]));
        assert_eq!(m.matched, 2);
        assert_eq!(m.at[1], Some(0));
        assert_eq!(m.at[14], Some(1));
    }

    #[test]
    fn unrecognised_headers_match_nothing_and_are_reported() {
        let m = ColumnMap::of(&headers(&["foo", "bar", "baz"]));
        assert_eq!(m.matched, 0);
        assert!(m.at.iter().all(|x| x.is_none()));
    }

    #[test]
    fn values_are_kept_verbatim_spaces_included() {
        let m = ColumnMap::of(&headers(&["案号", "全文"]));
        let record = record(&["  （2020）京1执1号 ", "<p>正文</p>"]);
        let case = m.row(&record);
        assert_eq!(case.case_id, "  （2020）京1执1号 ");
        assert_eq!(case.full_text, "<p>正文</p>");
    }
}
