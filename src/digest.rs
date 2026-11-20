//! The identity of a corpus: one SHA-256 over the whole record set.
//!
//! `corpus_digest` names *what a library holds*, not how it is stored: records
//! contribute to the hash in ascending id order, each framed by its big-endian
//! id and a length prefix. So rebuilding the index, re-compacting the trees or
//! inserting the same rows in a different order cannot move it, while one
//! changed byte in one judgment must. That is what lets a consumer pin the
//! corpus snapshot it measured (`/api/health` reports it) and re-run against
//! the same set — §7's rule that the entries are the contract and the file
//! layout is not, applied to one hash.

use fjall::Partition;
use sha2::{Digest as _, Sha256};

/// Key under which the digest lives in the `stats` partition. Like the other
/// checkpoints there, it is written only by a finished run: an interrupted
/// import has not yet decided what the corpus is.
pub const STATS_KEY: &[u8] = b"corpus_digest";

/// SHA-256 over every record in `cases`, in ascending id order: each entry
/// contributes its key, the stored row's length, and the row itself. The length
/// prefix is framing — without it two record sets that concatenate to the same
/// byte string would collide.
pub fn corpus_digest(cases: &Partition) -> String {
    let mut hasher = Sha256::new();
    for entry in cases.iter() {
        let (key, value) = entry.expect("a readable library yields every stored pair");
        hasher.update(key.as_ref());
        hasher.update((value.as_ref().len() as u64).to_le_bytes());
        hasher.update(value.as_ref());
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The digest a finished run recorded in `stats`, if this library has one.
/// Libraries built before the digest existed have none; callers must say so
/// rather than invent one.
pub fn recorded(stats: &Partition) -> Option<String> {
    stats
        .get(STATS_KEY)
        .ok()?
        .and_then(|v| String::from_utf8(v.as_ref().to_vec()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-partition library holding `rows`, flushed to disk: the digest must
    /// describe the records, so flushing and reopening are part of the test,
    /// not an exception to it.
    fn build(tag: &str, rows: &[(u32, &[u8])]) -> (fjall::Keyspace, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("localcases-digest-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        let keyspace = fjall::Config::new(&dir).open().unwrap();
        let cases = keyspace
            .open_partition("cases", fjall::PartitionCreateOptions::default())
            .unwrap();
        for (id, row) in rows {
            cases.insert(id.to_be_bytes(), *row).unwrap();
        }
        keyspace.force_flush().unwrap();
        (keyspace, dir)
    }

    fn digest_of(keyspace: &fjall::Keyspace) -> String {
        let cases = keyspace
            .open_partition("cases", fjall::PartitionCreateOptions::default())
            .unwrap();
        corpus_digest(&cases)
    }

    #[test]
    fn insertion_order_does_not_matter() {
        let rows: [(u32, &[u8]); 4] = [(1, b"alpha"), (2, b"beta"), (3, b"gamma"), (4, b"delta")];
        let (a, dir_a) = build("order-a", &rows);
        let (b, dir_b) = build(
            "order-b",
            &[(4, b"delta"), (2, b"beta"), (1, b"alpha"), (3, b"gamma")],
        );
        assert_eq!(digest_of(&a), digest_of(&b));
        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
    }

    #[test]
    fn one_changed_byte_moves_the_digest() {
        let (a, dir_a) = build("sensitive-a", &[(1, b"alpha"), (2, b"beta")]);
        let (b, dir_b) = build("sensitive-b", &[(1, b"alphA"), (2, b"beta")]);
        assert_ne!(digest_of(&a), digest_of(&b));
        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
    }

    #[test]
    fn framing_separates_the_rows_it_hashes() {
        // Without the length prefix both libraries concatenate to "abc" — the
        // framing is exactly what makes them different digests.
        let (a, dir_a) = build("framing-a", &[(1, b"ab"), (2, b"c")]);
        let (b, dir_b) = build("framing-b", &[(1, b"a"), (2, b"bc")]);
        assert_ne!(digest_of(&a), digest_of(&b));
        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
    }

    #[test]
    fn reopening_the_same_library_keeps_the_digest() {
        // Dropping the keyspace and opening the same directory again is a
        // storage-layout change with the records untouched.
        let dir = std::env::temp_dir().join("localcases-digest-reopen");
        let _ = std::fs::remove_dir_all(&dir);
        let first = fjall::Config::new(&dir).open().unwrap();
        {
            let cases = first
                .open_partition("cases", fjall::PartitionCreateOptions::default())
                .unwrap();
            cases.insert(1u32.to_be_bytes(), b"row one").unwrap();
            cases.insert(2u32.to_be_bytes(), b"row two").unwrap();
        }
        first.force_flush().unwrap();
        let before = digest_of(&first);
        drop(first);
        let again = fjall::Config::new(&dir).open().unwrap();
        assert_eq!(before, digest_of(&again));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_library_is_the_empty_hash() {
        // The digest of no records is defined: SHA-256 over nothing.
        let (a, dir_a) = build("empty", &[]);
        assert_eq!(
            digest_of(&a),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let _ = std::fs::remove_dir_all(&dir_a);
    }
}
