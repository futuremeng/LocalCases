//! Integrity check for a v2 keyspace, run after `convert`.
//!
//! Answers the questions a conversion log cannot answer on its own:
//! - does every stored document have exactly one id, with no gaps and no id
//!   handed out twice (the failure mode of a resumed run);
//! - do the `cases` / `fields` / `ids` partitions agree on the document count;
//! - is the derived layer (L1) still reproducible from the raw layer (L0);
//! - does a document's stable `doc_key` still point back at its id.
//!
//!     cargo run --release --bin check -- config_v2_full.toml
use std::collections::BTreeMap;

use bincode::config::standard;
use fjall::{Config, PartitionCreateOptions};
use localcases::extract;
use localcases::v2::*;
use localcases::{CONFIG, CaseV2};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() {
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        eprintln!(
            "usage: check [config.toml]\n\
             Checks id uniqueness, partition agreement, derived-layer\n\
             reproducibility and doc_key round-trips."
        );
        return;
    }

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info,fjall=warn"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let keyspace = Config::new(CONFIG.db.as_str()).open().unwrap();
    // Opened with the importer's options: fjall creates a missing partition, and
    // an empty default-layout one would then hide the fact that the library was
    // never built.
    let cases = keyspace
        .open_partition(P_CASES, localcases::record_partition_options())
        .unwrap();
    let fields = keyspace
        .open_partition(P_FIELDS, localcases::record_partition_options())
        .unwrap();
    let ids = keyspace
        .open_partition(P_IDS, PartitionCreateOptions::default())
        .unwrap();
    let stats = keyspace
        .open_partition(P_STATS, PartitionCreateOptions::default())
        .unwrap();

    // ---- 1. count documents, find the highest id --------------------------
    let started = std::time::Instant::now();
    let mut n_cases = 0u64;
    let mut max_id = 0u32;
    for key in cases.keys() {
        let key = key.unwrap();
        if let Ok(bytes) = <[u8; 4]>::try_from(key.as_ref()) {
            max_id = max_id.max(u32::from_be_bytes(bytes));
        } else {
            warn!("unexpected id key width {}", key.as_ref().len());
        }
        n_cases += 1;
    }
    info!(
        "cases: {n_cases} documents, highest id {max_id} ({}s)",
        started.elapsed().as_secs()
    );

    // ---- 2. every id used exactly once, no holes --------------------------
    let started = std::time::Instant::now();
    let mut seen = vec![false; max_id as usize + 1];
    let mut n_ids = 0u64;
    let mut duplicate_ids = 0u64;
    let mut out_of_range = 0u64;
    for value in ids.values() {
        let value = value.unwrap();
        n_ids += 1;
        match <[u8; 4]>::try_from(value.as_ref()) {
            Ok(bytes) => {
                let id = u32::from_be_bytes(bytes) as usize;
                if id == 0 || id > max_id as usize {
                    out_of_range += 1;
                } else if seen[id] {
                    duplicate_ids += 1;
                } else {
                    seen[id] = true;
                }
            }
            Err(_) => out_of_range += 1,
        }
    }
    let missing_ids = seen[1..].iter().filter(|s| !**s).count() as u64;
    info!(
        "ids: {n_ids} keys, duplicate ids {duplicate_ids}, ids outside 1..={max_id} \
         {out_of_range}, ids never issued {missing_ids} ({}s)",
        started.elapsed().as_secs()
    );

    /// What the passes above measured, handed to the last pass as one value
    /// instead of five positional arguments.
    struct Tally {
        n_cases: u64,
        max_id: u32,
        n_ids: u64,
        duplicate_ids: u64,
        missing_ids: u64,
    }

    // ---- 3. partitions agree on the document count ------------------------
    let n_fields = fields.keys().count() as u64;
    info!("fields: {n_fields} records");

    check_rest(
        &cases,
        &fields,
        &ids,
        &stats,
        Tally {
            n_cases,
            max_id,
            n_ids,
            duplicate_ids,
            missing_ids,
        },
    );

    /// Derived-layer reproducibility, doc_key round-trips and the final verdict.
    ///
    /// The derived/doc_key checks run on every `STRIDE`-th document: exact numbers
    /// for the sampled documents, while keeping the whole check inside a few minutes
    /// even for 85 million rows.
    fn check_rest(
        cases: &fjall::Partition,
        fields: &fjall::Partition,
        ids: &fjall::Partition,
        stats: &fjall::Partition,
        tally: Tally,
    ) {
        let Tally {
            n_cases,
            max_id,
            n_ids,
            duplicate_ids,
            missing_ids,
        } = tally;
        const STRIDE: u64 = 100_000;
        let started = std::time::Instant::now();
        let mut sampled = 0u64;
        let mut missing_cases = 0u64;
        let mut missing_fields = 0u64;
        let mut key_mismatch = 0u64;
        let mut no_doc_key = 0u64;
        let mut mismatches: BTreeMap<&'static str, u64> = BTreeMap::new();

        for (rank, key) in cases.keys().enumerate() {
            let key = key.unwrap();
            if !(rank as u64).is_multiple_of(STRIDE) {
                continue;
            }
            let Ok(id) = <[u8; 4]>::try_from(key.as_ref()) else {
                continue;
            };
            sampled += 1;

            // Keys are scanned, values are fetched only for the sampled ids.
            let Some(case_bytes) = cases.get(id).unwrap() else {
                missing_cases += 1;
                continue;
            };
            let case: CaseV2 = bincode::decode_from_slice(case_bytes.as_ref(), standard())
                .unwrap()
                .0;
            let stored = match fields.get(id).unwrap() {
                Some(v) => {
                    bincode::decode_from_slice::<extract::Derived, _>(v.as_ref(), standard())
                        .unwrap()
                        .0
                }
                None => {
                    missing_fields += 1;
                    continue;
                }
            };
            let fresh: extract::Derived = extract::derive(&case);
            let mut compare = |name: &'static str, ok: bool| {
                if !ok {
                    *mismatches.entry(name).or_default() += 1;
                }
            };
            compare("judgment_days", stored.judgment_days == fresh.judgment_days);
            compare("public_days", stored.public_days == fresh.public_days);
            compare("case_year", stored.case_year == fresh.case_year);
            compare("court_code", stored.court_code == fresh.court_code);
            compare("case_seq", stored.case_seq == fresh.case_seq);
            compare(
                "procedure_group",
                stored.procedure_group == fresh.procedure_group,
            );
            compare(
                "procedure_kind",
                stored.procedure_kind == fresh.procedure_kind,
            );
            compare("party_count", stored.party_count == fresh.party_count);
            compare("law_count", stored.law_count == fresh.law_count);
            compare("text_len", stored.text_len == fresh.text_len);
            compare("has_text", stored.has_text == fresh.has_text);
            compare("has_html", stored.has_html == fresh.has_html);
            compare("text_hash", stored.text_hash == fresh.text_hash);

            let key = case.doc_key();
            if key.is_empty() {
                no_doc_key += 1;
            } else {
                match ids.get(key.as_bytes()).unwrap() {
                    Some(v) if v.as_ref() == id.as_slice() => {}
                    Some(_) | None => key_mismatch += 1,
                }
            }
        }
        let derived_mismatch: u64 = mismatches.values().sum();
        info!(
            "sampled {sampled} documents: derived mismatches {derived_mismatch} {mismatches:?}, \
         missing cases records {missing_cases}, missing fields records {missing_fields}, \
         doc_key round-trip failures {key_mismatch}, without doc_key {no_doc_key} ({}s)",
            started.elapsed().as_secs()
        );

        let next_id = stats
            .get(b"next_id")
            .unwrap()
            .map(|v| u32::from_be_bytes(<[u8; 4]>::try_from(v.as_ref()).unwrap_or([0; 4])));

        let mut failures = 0;
        let mut check = |name: &str, ok: bool, detail: String| {
            info!("{}  {name}  {detail}", if ok { "PASS" } else { "FAIL" });
            if !ok {
                failures += 1;
            }
        };
        check(
            "no id issued twice",
            duplicate_ids == 0,
            format!("{duplicate_ids}"),
        );
        check(
            "no holes in the id space",
            missing_ids == 0,
            format!("{missing_ids}"),
        );
        check(
            "ids partition matches cases partition",
            n_ids == n_cases && n_ids == max_id as u64,
            format!("{n_ids} ids, {n_cases} cases, highest id {max_id}"),
        );
        check(
            "fields partition matches cases partition",
            fields.keys().count() as u64 == n_cases,
            String::new(),
        );
        check(
            "next_id checkpoint matches the stored ids",
            next_id.is_none_or(|n| n == max_id + 1),
            format!("{next_id:?} vs highest id {max_id}"),
        );
        check(
            "every sampled id resolves to a raw record",
            missing_cases == 0,
            format!("{missing_cases} missing of {sampled} sampled"),
        );
        check(
            "derived layer reproducible from the raw layer",
            derived_mismatch == 0 && missing_fields == 0,
            format!("{derived_mismatch} mismatches, {missing_fields} missing of {sampled} sampled"),
        );
        check(
            "doc_key still resolves to the same id",
            key_mismatch == 0,
            format!("{key_mismatch} of {} sampled", sampled - no_doc_key),
        );

        info!("{n_cases} documents checked: {failures} failure(s)");
        if failures > 0 {
            std::process::exit(1);
        }
    }
}
