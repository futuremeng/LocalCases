# Third-party licenses

The binaries in this repository statically link the crates below. The
application code itself is Apache-2.0 (see `LICENSE` and `NOTICE`); the
notices below are the attribution required by those crates' own terms
(MIT/BSD/ISC keep the copyright notice, Apache-2.0 additionally asks for a
copy of its license and any NOTICE file, and for stating that the sources
were modified — the exact license texts ship inside the published crates
and are unchanged).

`desktop/` is a separate workspace with its own lock file: run this tool
from that directory as well before shipping a desktop bundle.

Regenerate: `python3 tools/licenses.py`.

## Summary

| SPDX expression | crates |
| --- | --- |
| MIT OR Apache-2.0 | 133 |
| MIT | 72 |
| MIT/Apache-2.0 | 13 |
| Apache-2.0 OR MIT | 11 |
| Apache-2.0 | 7 |
| Unlicense OR MIT | 6 |
| Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 5 |
| BSD-3-Clause | 5 |
| Unlicense/MIT | 4 |
| Zlib | 3 |
| BSD-2-Clause OR Apache-2.0 OR MIT | 2 |
| MIT OR Apache-2.0 OR LGPL-2.1-or-later | 2 |
| (MIT OR Apache-2.0) AND Unicode-3.0 | 1 |
| 0BSD | 1 |
| 0BSD OR MIT OR Apache-2.0 | 1 |
| Apache-2.0 / MIT | 1 |
| Apache-2.0 / MIT / MPL-2.0 | 1 |
| Apache-2.0 OR BSL-1.0 | 1 |
| Apache-2.0 OR GPL-2.0-only | 1 |
| Apache-2.0/MIT | 1 |
| BSL-1.0 | 1 |
| MIT AND BSD-3-Clause | 1 |
| MIT OR Zlib OR Apache-2.0 | 1 |
| zlib-acknowledgement OR MIT | 1 |

## All crates

| crate | version | license |
| --- | --- | --- |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| adler32 | 1.2.0 | Zlib |
| ahash | 0.7.8 | MIT OR Apache-2.0 |
| aho-corasick | 1.1.5 | Unlicense OR MIT |
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 |
| arc-swap | 1.9.3 | MIT OR Apache-2.0 |
| async-compression | 0.4.51 | MIT OR Apache-2.0 |
| async-trait | 0.1.92 | MIT OR Apache-2.0 |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT |
| autocfg | 1.5.1 | Apache-2.0 OR MIT |
| axum | 0.8.9 | MIT |
| axum-core | 0.5.6 | MIT |
| base64 | 0.23.1 | MIT OR Apache-2.0 |
| basic-toml | 0.1.10 | MIT OR Apache-2.0 |
| bincode | 1.3.3 | MIT |
| bincode | 2.0.1 | MIT |
| bincode_derive | 2.0.1 | MIT |
| bitflags | 1.3.2 | MIT/Apache-2.0 |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| bitpacking | 0.9.3 | MIT |
| bon | 3.10.3 | MIT OR Apache-2.0 |
| bon-macros | 3.10.3 | MIT OR Apache-2.0 |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 |
| bytecount | 0.6.9 | Apache-2.0/MIT |
| byteorder | 1.5.0 | Unlicense OR MIT |
| bytes | 1.12.1 | MIT |
| byteview | 0.6.1 | MIT OR Apache-2.0 |
| cc | 1.10.0 | MIT OR Apache-2.0 |
| census | 0.4.2 | MIT |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 |
| cfg_aliases | 0.2.2 | MIT |
| compare | 0.0.6 | MIT/Apache-2.0 |
| compression-codecs | 0.4.46 | MIT OR Apache-2.0 |
| compression-core | 0.4.34 | MIT OR Apache-2.0 |
| crc32fast | 1.5.3 | MIT OR Apache-2.0 |
| crossbeam-channel | 0.5.17 | MIT OR Apache-2.0 |
| crossbeam-deque | 0.8.8 | MIT OR Apache-2.0 |
| crossbeam-epoch | 0.9.21 | MIT OR Apache-2.0 |
| crossbeam-skiplist | 0.1.3 | MIT OR Apache-2.0 |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 |
| crunchy | 0.2.4 | MIT |
| csv | 1.4.0 | Unlicense/MIT |
| csv-core | 0.1.13 | Unlicense/MIT |
| darling | 0.24.1 | MIT |
| darling_core | 0.24.1 | MIT |
| darling_macro | 0.24.1 | MIT |
| dary_heap | 0.3.9 | MIT OR Apache-2.0 |
| dashmap | 6.2.1 | MIT |
| datasketches | 0.5.0 | Apache-2.0 |
| defmt | 1.2.0 | MIT OR Apache-2.0 |
| defmt-macros | 1.2.0 | MIT OR Apache-2.0 |
| defmt-parser | 1.1.0 | MIT OR Apache-2.0 |
| deranged | 0.5.9 | MIT OR Apache-2.0 |
| double-ended-peekable | 0.1.0 | MIT |
| downcast-rs | 2.0.2 | MIT OR Apache-2.0 |
| either | 1.19.0 | MIT OR Apache-2.0 |
| enum_dispatch | 0.3.13 | MIT OR Apache-2.0 |
| equivalent | 1.0.3 | Apache-2.0 OR MIT |
| erased-serde | 0.4.11 | MIT OR Apache-2.0 |
| errno | 0.3.14 | MIT OR Apache-2.0 |
| fast2s | 0.3.1 | MIT |
| fastdivide | 0.4.2 | zlib-acknowledgement OR MIT |
| fastrand | 2.5.0 | Apache-2.0 OR MIT |
| find-msvc-tools | 0.1.15 | MIT OR Apache-2.0 |
| fjall | 2.11.2 | MIT OR Apache-2.0 |
| flate2 | 1.1.10 | MIT OR Apache-2.0 |
| fnv | 1.0.7 | Apache-2.0 / MIT |
| foldhash | 0.2.0 | Zlib |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 |
| frostem | 1.20260821.8 | BSD-3-Clause |
| fs4 | 0.13.1 | MIT OR Apache-2.0 |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 |
| futures-core | 0.3.34 | MIT OR Apache-2.0 |
| futures-macro | 0.3.34 | MIT OR Apache-2.0 |
| futures-sink | 0.3.34 | MIT OR Apache-2.0 |
| futures-task | 0.3.34 | MIT OR Apache-2.0 |
| futures-util | 0.3.34 | MIT OR Apache-2.0 |
| getrandom | 0.2.17 | MIT OR Apache-2.0 |
| getrandom | 0.3.4 | MIT OR Apache-2.0 |
| getrandom | 0.4.3 | MIT OR Apache-2.0 |
| guardian | 1.3.0 | MIT OR Apache-2.0 |
| h2 | 0.4.20 | MIT |
| hashbrown | 0.12.3 | MIT OR Apache-2.0 |
| hashbrown | 0.14.5 | MIT OR Apache-2.0 |
| hashbrown | 0.16.1 | MIT OR Apache-2.0 |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| htmlescape | 0.3.1 | Apache-2.0 / MIT / MPL-2.0 |
| http | 1.5.0 | MIT OR Apache-2.0 |
| http-body | 1.1.0 | MIT |
| http-body-util | 0.1.5 | MIT |
| httparse | 1.10.1 | MIT OR Apache-2.0 |
| httpdate | 1.0.3 | MIT OR Apache-2.0 |
| hyper | 1.12.0 | MIT |
| hyper-util | 0.1.21 | MIT |
| ident_case | 1.0.1 | MIT/Apache-2.0 |
| include-flate | 0.3.4 | Apache-2.0 |
| include-flate-codegen | 0.3.4 | Apache-2.0 |
| include-flate-compress | 0.3.4 | Apache-2.0 |
| indexmap | 2.14.2 | Apache-2.0 OR MIT |
| interval-heap | 0.0.5 | MIT/Apache-2.0 |
| inventory | 0.3.25 | MIT OR Apache-2.0 |
| itertools | 0.14.0 | MIT OR Apache-2.0 |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| jieba-macros | 0.11.0 | MIT |
| jieba-rs | 0.11.0 | MIT |
| jiff | 0.2.38 | Unlicense OR MIT |
| jiff-core | 0.1.1 | Unlicense OR MIT |
| jiff-static | 0.2.38 | Unlicense OR MIT |
| jobserver | 0.1.35 | MIT OR Apache-2.0 |
| js-sys | 0.3.106 | MIT OR Apache-2.0 |
| lazy_static | 1.5.1 | MIT OR Apache-2.0 |
| levenshtein_automata | 0.2.1 | MIT |
| libc | 0.2.191 | MIT OR Apache-2.0 |
| libflate | 2.3.2 | MIT |
| libflate_lz77 | 2.3.0 | MIT |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| lock_api | 0.4.14 | MIT OR Apache-2.0 |
| log | 0.4.34 | MIT OR Apache-2.0 |
| lru | 0.18.5 | MIT |
| lsm-tree | 2.10.4 | MIT OR Apache-2.0 |
| lz4_flex | 0.14.0 | MIT |
| matchers | 0.2.0 | MIT |
| matchit | 0.8.4 | MIT AND BSD-3-Clause |
| measure_time | 0.9.0 | MIT |
| memchr | 2.8.3 | Unlicense OR MIT |
| memmap2 | 0.9.12 | MIT OR Apache-2.0 |
| mime | 0.3.17 | MIT OR Apache-2.0 |
| minimal-lexical | 0.2.1 | MIT/Apache-2.0 |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| mio | 1.2.4 | MIT |
| murmurhash32 | 0.4.0 | MIT |
| nix | 0.31.3 | MIT |
| no_std_io2 | 0.9.4 | Apache-2.0 OR MIT |
| nom | 7.1.3 | MIT |
| nu-ansi-term | 0.50.3 | MIT |
| num-conv | 0.2.2 | MIT OR Apache-2.0 |
| num-traits | 0.2.19 | MIT OR Apache-2.0 |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| oneshot | 0.1.13 | MIT OR Apache-2.0 |
| ordered-float | 5.5.0 | MIT |
| ownedbytes | 0.9.0 | MIT |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 |
| path-absolutize | 3.1.1 | MIT |
| path-dedot | 3.1.1 | MIT |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 |
| portable-atomic | 1.15.0 | Apache-2.0 OR MIT |
| portable-atomic-util | 0.2.8 | Apache-2.0 OR MIT |
| powerfmt | 0.2.1 | MIT OR Apache-2.0 |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 |
| prettyplease | 0.3.0 | MIT OR Apache-2.0 |
| proc-macro-error-attr3 | 3.1.1 | MIT OR Apache-2.0 |
| proc-macro-error3 | 3.1.1 | MIT OR Apache-2.0 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| quick_cache | 0.6.24 | MIT |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| r-efi | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| rand | 0.9.5 | MIT OR Apache-2.0 |
| rand_chacha | 0.9.0 | MIT OR Apache-2.0 |
| rand_core | 0.9.5 | MIT OR Apache-2.0 |
| rayon | 1.12.0 | MIT OR Apache-2.0 |
| rayon-core | 1.13.0 | MIT OR Apache-2.0 |
| redox_syscall | 0.5.18 | MIT |
| regex | 1.13.1 | MIT OR Apache-2.0 |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 |
| rle-decode-fast | 1.0.3 | MIT OR Apache-2.0 |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT |
| rustix | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| rustversion | 1.0.23 | MIT OR Apache-2.0 |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 |
| self_cell | 1.3.0 | Apache-2.0 OR GPL-2.0-only |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde-tuple-vec-map | 1.0.1 | MIT |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.154 | MIT OR Apache-2.0 |
| serde_path_to_error | 0.1.20 | MIT OR Apache-2.0 |
| serde_urlencoded | 0.7.1 | MIT/Apache-2.0 |
| sharded-slab | 0.1.7 | MIT |
| shlex | 2.0.1 | MIT OR Apache-2.0 |
| simd-adler32 | 0.3.10 | MIT |
| sketches-ddsketch | 0.4.1 | Apache-2.0 |
| slab | 0.4.12 | MIT |
| smallvec | 1.16.4 | MIT OR Apache-2.0 |
| socket2 | 0.6.5 | MIT OR Apache-2.0 |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 |
| std-semaphore | 0.1.0 | MIT/Apache-2.0 |
| stop-words | 0.10.1 | MIT OR Apache-2.0 |
| strsim | 0.11.1 | MIT |
| syn | 2.0.119 | MIT OR Apache-2.0 |
| syn | 3.0.9 | MIT OR Apache-2.0 |
| sync_wrapper | 1.0.2 | Apache-2.0 |
| tantivy | 0.27.0 | MIT |
| tantivy-bitpacker | 0.27.0 | MIT |
| tantivy-columnar | 0.27.0 | MIT |
| tantivy-common | 0.27.0 | MIT |
| tantivy-fst | 0.5.0 | Unlicense/MIT |
| tantivy-jieba | 0.21.0 | MIT |
| tantivy-query-grammar | 0.27.0 | MIT |
| tantivy-sstable | 0.27.0 | MIT |
| tantivy-stacker | 0.27.0 | MIT |
| tantivy-tokenizer-api | 0.7.0 | MIT |
| tempfile | 3.27.0 | MIT OR Apache-2.0 |
| thiserror | 2.0.21 | MIT OR Apache-2.0 |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 |
| thread_local | 1.1.10 | MIT OR Apache-2.0 |
| tikv-jemalloc-sys | 0.7.1+5.3.1-0-g81034ce1f1373e37dc865038e1bc8eeecf559ce8 | MIT/Apache-2.0 |
| tikv-jemallocator | 0.7.0 | MIT/Apache-2.0 |
| time | 0.3.55 | MIT OR Apache-2.0 |
| time-core | 0.1.9 | MIT OR Apache-2.0 |
| time-macros | 0.2.32 | MIT OR Apache-2.0 |
| tokio | 1.54.1 | MIT |
| tokio-macros | 2.7.2 | MIT |
| tokio-util | 0.7.20 | MIT |
| tower | 0.5.3 | MIT |
| tower-http | 0.7.1 | MIT |
| tower-layer | 0.3.3 | MIT |
| tower-service | 0.3.3 | MIT |
| tracing | 0.1.44 | MIT |
| tracing-attributes | 0.1.31 | MIT |
| tracing-core | 0.1.36 | MIT |
| tracing-log | 0.2.0 | MIT |
| tracing-subscriber | 0.3.23 | MIT |
| typed-path | 0.13.0 | MIT OR Apache-2.0 |
| typeid | 1.0.3 | MIT OR Apache-2.0 |
| typetag | 0.2.23 | MIT OR Apache-2.0 |
| typetag-impl | 0.2.23 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| unty | 0.0.4 | MIT OR Apache-2.0 |
| unwrap-infallible | 1.0.0 | MIT OR Apache-2.0 |
| utf8-ranges | 1.0.5 | Unlicense/MIT |
| uuid | 1.29.0 | Apache-2.0 OR MIT |
| valuable | 0.1.1 | MIT |
| value-log | 1.9.0 | MIT OR Apache-2.0 |
| varint-rs | 2.2.1 | 0BSD |
| version_check | 0.9.5 | MIT/Apache-2.0 |
| virtue | 0.0.18 | MIT |
| wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasip2 | 1.0.4+wasi-0.2.12 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasm-bindgen | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-macro | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-macro-support | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-shared | 0.2.129 | MIT OR Apache-2.0 |
| winapi | 0.3.9 | MIT/Apache-2.0 |
| winapi-i686-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 |
| windows_aarch64_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_aarch64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_gnu | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnu | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| wit-bindgen | 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| xxhash-rust | 0.8.19 | BSL-1.0 |
| zerocopy | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerocopy-derive | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zip | 9.1.0 | MIT |
| zlib-rs | 0.6.8 | Zlib |
| zmij | 1.0.23 | MIT |
| zopfli | 0.8.4 | Apache-2.0 |
| zstd | 0.13.3 | MIT |
| zstd | 0.14.3 | BSD-3-Clause |
| zstd-safe | 7.3.0 | BSD-3-Clause |
| zstd-safe | 8.2.0 | BSD-3-Clause |
| zstd-sys | 2.1.2+zstd.1.5.7 | BSD-3-Clause |
