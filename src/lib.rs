use fjall::{Keyspace, KvSeparationOptions, PartitionCreateOptions, PartitionHandle};
use std::sync::Arc;

pub use config::CONFIG;
pub use model::CaseV2;
pub use search::{Search, Searcher};

pub mod api;
pub mod config;
pub mod extract;
pub mod model;
pub mod proxy;
pub mod query;
pub mod search;
pub mod server;

/// The fjall partitions a LocalCases library is made of. These strings are
/// on-disk format: changing one means migrating existing libraries.
pub mod v2 {
    /// id -> the imported row: all 15 source columns, 全文 kept verbatim
    pub const P_CASES: &str = "cases";
    /// id -> derived facts (dates as day numbers, court codes, procedure groups,
    /// statute list, text statistics)
    pub const P_FIELDS: &str = "fields";
    /// source document key -> id; identity, duplicate detection, id reuse
    pub const P_IDS: &str = "ids";
    /// per-CSV progress records, so an interrupted import resumes where it stopped
    pub const P_PROGRESS: &str = "progress";
    /// counters behind the import report (rows, kept, duplicates, empty 全文, …)
    pub const P_STATS: &str = "stats";
}

/// The service state. `library` is declared last on purpose: Rust drops
/// struct fields in declaration order, and fjall tears a keyspace's file-handle
/// table (plus its background threads) down when the last `Keyspace` clone goes
/// away. A state that held only partition handles aborted on the first read
/// that needed a segment file, with `should acquire file handle` from lsm-tree.
#[derive(Clone)]
pub struct AppState {
    /// `P_CASES`
    pub records: PartitionHandle,
    /// `P_FIELDS`
    pub derived: PartitionHandle,
    pub searcher: Arc<Searcher>,
    /// Keeps the partitions usable; see the note above.
    pub library: Keyspace,
}

/// Create options for the two large partitions. A judgment row runs to tens of
/// kilobytes, so anything above a kilobyte is written to the value log and the
/// LSM tree is left holding keys plus small pointers — much less tree data to
/// rewrite every time compaction runs. The 1 KiB cut-off is above almost all of
/// the derived records, so the small fields a facet or detail page wants stay
/// next to the key instead of costing a second read.
pub fn record_partition_options() -> PartitionCreateOptions {
    PartitionCreateOptions::default()
        .max_memtable_size(192_000_000)
        .with_kv_separation(
            KvSeparationOptions::default()
                .separation_threshold(1_024)
                .file_target_size(128_000_000),
        )
}
