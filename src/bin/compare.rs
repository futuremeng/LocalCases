//! Byte-for-byte comparison of two v2 keyspaces.
//!
//! The parallel converter must produce *exactly* what the serial one produced:
//! same ids for the same rows, same L0/L1 records, same counters. Records are
//! bincode-encoded, so comparing bytes is both cheap and strict. Exits non-zero
//! on any difference and prints the first few offenders.
//!
//!   usage: compare <keyspace A> <keyspace B>

use fjall::Config;
use localcases::v2::*;
use std::process::exit;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Walks both partitions in key order. `values` = false compares keys only, which
/// is how the checkpoint partition is checked after a semantics change: its
/// payload (`rows_done` = last consumed row) is allowed to differ while the ids,
/// records and report counters must not.
fn compare_partitions(a: &fjall::Partition, b: &fjall::Partition, name: &str, values: bool) -> u64 {
    let mut ia = a.iter();
    let mut ib = b.iter();
    let mut same: u64 = 0;
    let mut keys_diff = 0u64;
    let mut vals_diff = 0u64;
    loop {
        let (x, y) = match (ia.next(), ib.next()) {
            (None, None) => break,
            (None, Some(_)) => {
                error!("{name}: {name} A is shorter than B (after {same} entries)");
                keys_diff += 1;
                break;
            }
            (Some(_), None) => {
                error!("{name}: A is longer than B (after {same} entries)");
                keys_diff += 1;
                break;
            }
            (Some(x), Some(y)) => (x.unwrap(), y.unwrap()),
        };
        let (ka, va) = x;
        let (kb, vb) = y;
        if ka.as_ref() != kb.as_ref() {
            keys_diff += 1;
            if keys_diff <= 3 {
                error!(
                    "{name}: key differs: {:?} vs {:?}",
                    String::from_utf8_lossy(ka.as_ref()),
                    String::from_utf8_lossy(kb.as_ref())
                );
            }
        } else if values && va.as_ref() != vb.as_ref() {
            vals_diff += 1;
            if vals_diff <= 3 {
                error!(
                    "{name}: value differs for key {:?} ({} vs {} bytes)",
                    String::from_utf8_lossy(ka.as_ref()),
                    va.len(),
                    vb.len()
                );
            }
        } else {
            same += 1;
        }
    }
    let len_a = a.len().unwrap();
    let len_b = b.len().unwrap();
    if len_a != len_b {
        error!("{name}: partition lengths differ: {len_a} vs {len_b}");
        keys_diff += 1;
    }
    info!(
        "{name}: {same} identical entries, {keys_diff} key/length problems, {vals_diff} value problems"
    );
    keys_diff + vals_diff
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") || args.len() < 3 {
        eprintln!(
            "usage: compare <keyspace A> <keyspace B> [--ignore-progress-values]\n\
             Exits non-zero if the two keyspaces hold different ids, records or\n\
             counters. With --ignore-progress-values the checkpoint partition is\n\
             compared by key only (its payload format changed on 2026-10-25)."
        );
        exit(2);
    }
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info,fjall=warn"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let a = Config::new(&args[1]).open().unwrap();
    let b = Config::new(&args[2]).open().unwrap();

    let ignore_progress_values = args.iter().any(|a| a == "--ignore-progress-values");
    let mut problems = 0u64;
    for name in [P_CASES, P_FIELDS, P_IDS, P_PROGRESS, P_STATS] {
        let pa = a.open_partition(name, Default::default()).unwrap();
        let pb = b.open_partition(name, Default::default()).unwrap();
        let values = !(name == P_PROGRESS && ignore_progress_values);
        problems += compare_partitions(&pa, &pb, name, values);
    }

    if problems == 0 {
        info!("keyspaces are identical, entry for entry");
    } else {
        error!("{problems} problem(s) between {} and {}", args[1], args[2]);
        exit(1);
    }
}
