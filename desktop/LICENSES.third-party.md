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
| MIT OR Apache-2.0 | 210 |
| MIT | 138 |
| (no license field) | 77 |
| Apache-2.0 OR MIT | 29 |
| Unicode-3.0 | 18 |
| MIT/Apache-2.0 | 15 |
| Apache-2.0 | 8 |
| BSD-3-Clause | 7 |
| Unlicense OR MIT | 6 |
| Unlicense/MIT | 6 |
| Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 5 |
| MPL-2.0 | 5 |
| Apache-2.0/MIT | 3 |
| Zlib | 3 |
| BSD-2-Clause OR Apache-2.0 OR MIT | 2 |
| MIT OR Apache-2.0 OR LGPL-2.1-or-later | 2 |
| MIT OR Apache-2.0 OR Zlib | 2 |
| MIT OR Zlib OR Apache-2.0 | 2 |
| (MIT OR Apache-2.0) AND Unicode-3.0 | 1 |
| 0BSD | 1 |
| 0BSD OR MIT OR Apache-2.0 | 1 |
| Apache-2.0 / MIT | 1 |
| Apache-2.0 / MIT / MPL-2.0 | 1 |
| Apache-2.0 AND MIT | 1 |
| Apache-2.0 OR BSL-1.0 | 1 |
| Apache-2.0 OR GPL-2.0-only | 1 |
| Apache-2.0 WITH LLVM-exception | 1 |
| BSD-3-Clause AND MIT | 1 |
| BSD-3-Clause/MIT | 1 |
| BSL-1.0 | 1 |
| CC0-1.0 OR MIT-0 OR Apache-2.0 | 1 |
| ISC | 1 |
| MIT AND BSD-3-Clause | 1 |
| Zlib OR Apache-2.0 OR MIT | 1 |
| zlib-acknowledgement OR MIT | 1 |

## All crates

| crate | version | license |
| --- | --- | --- |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| adler32 | 1.2.0 | Zlib |
| ahash | 0.7.8 | MIT OR Apache-2.0 |
| aho-corasick | 1.1.5 | Unlicense OR MIT |
| alloc-no-stdlib | 3.0.0 | BSD-3-Clause |
| alloc-stdlib | 0.3.0 | BSD-3-Clause |
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 |
| android_system_properties | 0.1.6 | (no license field) |
| anyhow | 1.0.104 | MIT OR Apache-2.0 |
| arc-swap | 1.9.3 | MIT OR Apache-2.0 |
| async-compression | 0.4.51 | MIT OR Apache-2.0 |
| async-trait | 0.1.92 | MIT OR Apache-2.0 |
| atk | 0.18.2 | MIT |
| atk-sys | 0.18.2 | MIT |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT |
| autocfg | 1.5.1 | Apache-2.0 OR MIT |
| axum | 0.8.9 | MIT |
| axum-core | 0.5.6 | MIT |
| base64 | 0.21.7 | (no license field) |
| base64 | 0.23.1 | MIT OR Apache-2.0 |
| basic-toml | 0.1.10 | MIT OR Apache-2.0 |
| bincode | 1.3.3 | MIT |
| bincode | 2.0.1 | MIT |
| bincode_derive | 2.0.1 | MIT |
| bit-set | 0.8.0 | Apache-2.0 OR MIT |
| bit-vec | 0.8.0 | Apache-2.0 OR MIT |
| bitflags | 1.3.2 | MIT/Apache-2.0 |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| bitpacking | 0.9.3 | MIT |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 |
| block2 | 0.6.2 | (no license field) |
| bon | 3.10.3 | MIT OR Apache-2.0 |
| bon-macros | 3.10.3 | MIT OR Apache-2.0 |
| brotli | 9.1.0 | BSD-3-Clause AND MIT |
| brotli-decompressor | 6.2.0 | BSD-3-Clause/MIT |
| bs58 | 0.5.1 | MIT/Apache-2.0 |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 |
| bytecount | 0.6.9 | Apache-2.0/MIT |
| bytemuck | 1.25.2 | (no license field) |
| byteorder | 1.5.0 | Unlicense OR MIT |
| bytes | 1.12.1 | MIT |
| byteview | 0.6.1 | MIT OR Apache-2.0 |
| cairo-rs | 0.18.5 | MIT |
| cairo-sys-rs | 0.18.2 | MIT |
| camino | 1.2.6 | MIT OR Apache-2.0 |
| cargo-platform | 0.1.9 | MIT OR Apache-2.0 |
| cargo_metadata | 0.19.2 | MIT |
| cargo_toml | 1.0.1 | Apache-2.0 OR MIT |
| cc | 1.10.0 | MIT OR Apache-2.0 |
| census | 0.4.2 | MIT |
| cesu8 | 1.1.0 | (no license field) |
| cfb | 0.14.0 | MIT |
| cfg-expr | 0.15.8 | MIT OR Apache-2.0 |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 |
| cfg_aliases | 0.2.2 | MIT |
| chrono | 0.4.45 | MIT OR Apache-2.0 |
| combine | 4.6.9 | (no license field) |
| compare | 0.0.6 | MIT/Apache-2.0 |
| compression-codecs | 0.4.46 | MIT OR Apache-2.0 |
| compression-core | 0.4.34 | MIT OR Apache-2.0 |
| cookie | 0.18.2 | MIT OR Apache-2.0 |
| core-foundation | 0.10.1 | MIT OR Apache-2.0 |
| core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 |
| core-graphics | 0.25.0 | (no license field) |
| core-graphics-types | 0.2.0 | (no license field) |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 |
| crc32fast | 1.5.3 | MIT OR Apache-2.0 |
| crossbeam-channel | 0.5.17 | MIT OR Apache-2.0 |
| crossbeam-deque | 0.8.8 | MIT OR Apache-2.0 |
| crossbeam-epoch | 0.9.21 | MIT OR Apache-2.0 |
| crossbeam-skiplist | 0.1.3 | MIT OR Apache-2.0 |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 |
| crunchy | 0.2.4 | MIT |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 |
| cssparser | 0.37.0 | MPL-2.0 |
| cssparser-macros | 0.7.1 | MPL-2.0 |
| csv | 1.4.0 | Unlicense/MIT |
| csv-core | 0.1.13 | Unlicense/MIT |
| ctor | 1.0.13 | Apache-2.0 OR MIT |
| darling | 0.24.1 | MIT |
| darling_core | 0.24.1 | MIT |
| darling_macro | 0.24.1 | MIT |
| dary_heap | 0.3.9 | MIT OR Apache-2.0 |
| dashmap | 6.2.1 | MIT |
| datasketches | 0.5.0 | Apache-2.0 |
| dbus | 0.9.12 | Apache-2.0/MIT |
| defmt | 1.2.0 | MIT OR Apache-2.0 |
| defmt-macros | 1.2.0 | MIT OR Apache-2.0 |
| defmt-parser | 1.1.0 | MIT OR Apache-2.0 |
| deranged | 0.5.9 | MIT OR Apache-2.0 |
| derive_more | 2.1.1 | MIT |
| derive_more-impl | 2.1.1 | MIT |
| digest | 0.10.7 | MIT OR Apache-2.0 |
| dirs | 7.0.0 | MIT OR Apache-2.0 |
| dirs-sys | 0.5.0 | MIT OR Apache-2.0 |
| dispatch2 | 0.3.1 | (no license field) |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 |
| dlopen2 | 0.8.2 | MIT |
| dlopen2_derive | 0.4.3 | MIT |
| dom_query | 0.28.0 | MIT |
| double-ended-peekable | 0.1.0 | MIT |
| downcast-rs | 2.0.2 | MIT OR Apache-2.0 |
| dpi | 0.1.2 | Apache-2.0 AND MIT |
| dtoa | 1.0.11 | MIT OR Apache-2.0 |
| dtoa-short | 0.3.5 | MPL-2.0 |
| dunce | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 |
| dyn-clone | 1.0.20 | MIT OR Apache-2.0 |
| either | 1.19.0 | MIT OR Apache-2.0 |
| embed-resource | 3.0.12 | MIT |
| embed_plist | 1.2.2 | (no license field) |
| enum_dispatch | 0.3.13 | MIT OR Apache-2.0 |
| equivalent | 1.0.3 | Apache-2.0 OR MIT |
| erased-serde | 0.4.11 | MIT OR Apache-2.0 |
| errno | 0.3.14 | MIT OR Apache-2.0 |
| fast2s | 0.3.1 | MIT |
| fastdivide | 0.4.2 | zlib-acknowledgement OR MIT |
| fastrand | 2.5.0 | Apache-2.0 OR MIT |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 |
| field-offset | 0.3.6 | MIT OR Apache-2.0 |
| find-msvc-tools | 0.1.15 | MIT OR Apache-2.0 |
| fjall | 2.11.2 | MIT OR Apache-2.0 |
| flate2 | 1.1.10 | MIT OR Apache-2.0 |
| fnv | 1.0.7 | Apache-2.0 / MIT |
| foldhash | 0.2.0 | Zlib |
| foreign-types | 0.5.0 | (no license field) |
| foreign-types-macros | 0.2.4 | (no license field) |
| foreign-types-shared | 0.3.1 | (no license field) |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 |
| frostem | 1.20260821.8 | BSD-3-Clause |
| fs4 | 0.13.1 | MIT OR Apache-2.0 |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 |
| futures-core | 0.3.34 | MIT OR Apache-2.0 |
| futures-executor | 0.3.34 | MIT OR Apache-2.0 |
| futures-io | 0.3.34 | MIT OR Apache-2.0 |
| futures-macro | 0.3.34 | MIT OR Apache-2.0 |
| futures-sink | 0.3.34 | MIT OR Apache-2.0 |
| futures-task | 0.3.34 | MIT OR Apache-2.0 |
| futures-util | 0.3.34 | MIT OR Apache-2.0 |
| gdk | 0.18.2 | MIT |
| gdk-pixbuf | 0.18.5 | MIT |
| gdk-pixbuf-sys | 0.18.0 | MIT |
| gdk-sys | 0.18.2 | MIT |
| gdkwayland-sys | 0.18.2 | MIT |
| gdkx11 | 0.18.2 | MIT |
| gdkx11-sys | 0.18.2 | MIT |
| generic-array | 0.14.7 | MIT |
| getrandom | 0.2.17 | MIT OR Apache-2.0 |
| getrandom | 0.3.4 | MIT OR Apache-2.0 |
| getrandom | 0.4.3 | MIT OR Apache-2.0 |
| gio | 0.18.4 | MIT |
| gio-sys | 0.18.1 | MIT |
| glib | 0.18.5 | MIT |
| glib-macros | 0.18.5 | MIT |
| glib-sys | 0.18.1 | MIT |
| glob | 0.3.4 | MIT OR Apache-2.0 |
| gobject-sys | 0.18.0 | MIT |
| gtk | 0.18.2 | MIT |
| gtk-sys | 0.18.2 | MIT |
| gtk3-macros | 0.18.2 | MIT |
| guardian | 1.3.0 | MIT OR Apache-2.0 |
| h2 | 0.4.20 | MIT |
| hashbrown | 0.12.3 | MIT OR Apache-2.0 |
| hashbrown | 0.14.5 | MIT OR Apache-2.0 |
| hashbrown | 0.16.1 | MIT OR Apache-2.0 |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| heck | 0.4.1 | MIT OR Apache-2.0 |
| heck | 0.5.0 | MIT OR Apache-2.0 |
| hex | 0.4.3 | MIT OR Apache-2.0 |
| html5ever | 0.39.0 | MIT OR Apache-2.0 |
| htmlescape | 0.3.1 | Apache-2.0 / MIT / MPL-2.0 |
| http | 1.5.0 | MIT OR Apache-2.0 |
| http-body | 1.1.0 | MIT |
| http-body-util | 0.1.5 | MIT |
| httparse | 1.10.1 | MIT OR Apache-2.0 |
| httpdate | 1.0.3 | MIT OR Apache-2.0 |
| hyper | 1.12.0 | MIT |
| hyper-util | 0.1.21 | MIT |
| iana-time-zone | 0.1.65 | MIT OR Apache-2.0 |
| iana-time-zone-haiku | 0.1.2 | (no license field) |
| ico | 0.6.0 | MIT |
| icu_collections | 2.4.0 | Unicode-3.0 |
| icu_locale_core | 2.4.0 | Unicode-3.0 |
| icu_normalizer | 2.4.0 | Unicode-3.0 |
| icu_normalizer_data | 2.4.0 | Unicode-3.0 |
| icu_properties | 2.4.0 | Unicode-3.0 |
| icu_properties_data | 2.4.0 | Unicode-3.0 |
| icu_provider | 2.4.0 | Unicode-3.0 |
| ident_case | 1.0.1 | MIT/Apache-2.0 |
| idna | 1.1.0 | MIT OR Apache-2.0 |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT |
| include-flate | 0.3.4 | Apache-2.0 |
| include-flate-codegen | 0.3.4 | Apache-2.0 |
| include-flate-compress | 0.3.4 | Apache-2.0 |
| indexmap | 1.9.3 | Apache-2.0 OR MIT |
| indexmap | 2.14.2 | Apache-2.0 OR MIT |
| infer | 0.22.0 | MIT |
| interval-heap | 0.0.5 | MIT/Apache-2.0 |
| inventory | 0.3.25 | MIT OR Apache-2.0 |
| ipnet | 2.12.2 | MIT OR Apache-2.0 |
| itertools | 0.14.0 | MIT OR Apache-2.0 |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| javascriptcore-rs | 1.1.2 | MIT |
| javascriptcore-rs-sys | 1.1.1 | MIT |
| jieba-macros | 0.11.0 | MIT |
| jieba-rs | 0.11.0 | MIT |
| jiff | 0.2.38 | Unlicense OR MIT |
| jiff-core | 0.1.1 | Unlicense OR MIT |
| jiff-static | 0.2.38 | Unlicense OR MIT |
| jiff-tzdb | 0.1.9 | (no license field) |
| jiff-tzdb-platform | 0.1.3 | (no license field) |
| jni | 0.21.1 | (no license field) |
| jni-sys | 0.3.1 | (no license field) |
| jni-sys | 0.4.1 | (no license field) |
| jni-sys-macros | 0.4.1 | (no license field) |
| jobserver | 0.1.35 | MIT OR Apache-2.0 |
| js-sys | 0.3.106 | MIT OR Apache-2.0 |
| json-patch | 4.2.0 | MIT/Apache-2.0 |
| jsonptr | 0.7.1 | MIT OR Apache-2.0 |
| keyboard-types | 0.8.3 | MIT OR Apache-2.0 |
| lazy_static | 1.5.1 | MIT OR Apache-2.0 |
| levenshtein_automata | 0.2.1 | MIT |
| libappindicator | 0.9.0 | Apache-2.0 OR MIT |
| libappindicator-sys | 0.9.0 | Apache-2.0 OR MIT |
| libc | 0.2.191 | MIT OR Apache-2.0 |
| libdbus-sys | 0.2.7 | Apache-2.0/MIT |
| libflate | 2.3.2 | MIT |
| libflate_lz77 | 2.3.0 | MIT |
| libloading | 0.7.4 | ISC |
| libredox | 0.1.25 | MIT |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| litemap | 0.8.3 | Unicode-3.0 |
| lock_api | 0.4.14 | MIT OR Apache-2.0 |
| log | 0.4.34 | MIT OR Apache-2.0 |
| lru | 0.18.5 | MIT |
| lsm-tree | 2.10.4 | MIT OR Apache-2.0 |
| lz4_flex | 0.14.0 | MIT |
| markup5ever | 0.39.0 | MIT OR Apache-2.0 |
| matchers | 0.2.0 | MIT |
| matchit | 0.8.4 | MIT AND BSD-3-Clause |
| measure_time | 0.9.0 | MIT |
| memchr | 2.8.3 | Unlicense OR MIT |
| memmap2 | 0.9.12 | MIT OR Apache-2.0 |
| memoffset | 0.9.1 | MIT |
| mime | 0.3.17 | MIT OR Apache-2.0 |
| minimal-lexical | 0.2.1 | MIT/Apache-2.0 |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| mio | 1.2.4 | MIT |
| muda | 0.20.0 | Apache-2.0 OR MIT |
| murmurhash32 | 0.4.0 | MIT |
| ndk | 0.9.0 | (no license field) |
| ndk-context | 0.1.1 | (no license field) |
| ndk-sys | 0.6.0+11769913 | (no license field) |
| new_debug_unreachable | 1.0.6 | MIT |
| nix | 0.31.3 | MIT |
| no_std_io2 | 0.9.4 | Apache-2.0 OR MIT |
| nom | 7.1.3 | MIT |
| nu-ansi-term | 0.50.3 | MIT |
| num-conv | 0.2.2 | MIT OR Apache-2.0 |
| num-traits | 0.2.19 | MIT OR Apache-2.0 |
| num_enum | 0.7.6 | (no license field) |
| num_enum_derive | 0.7.6 | (no license field) |
| objc2 | 0.6.5 | (no license field) |
| objc2-app-kit | 0.3.2 | (no license field) |
| objc2-cloud-kit | 0.3.2 | (no license field) |
| objc2-core-data | 0.3.2 | (no license field) |
| objc2-core-foundation | 0.3.2 | (no license field) |
| objc2-core-graphics | 0.3.2 | (no license field) |
| objc2-core-image | 0.3.2 | (no license field) |
| objc2-core-location | 0.3.2 | (no license field) |
| objc2-core-text | 0.3.2 | (no license field) |
| objc2-encode | 4.1.0 | (no license field) |
| objc2-exception-helper | 0.1.1 | (no license field) |
| objc2-foundation | 0.3.2 | (no license field) |
| objc2-io-surface | 0.3.2 | (no license field) |
| objc2-quartz-core | 0.3.2 | (no license field) |
| objc2-ui-kit | 0.3.2 | (no license field) |
| objc2-user-notifications | 0.3.2 | (no license field) |
| objc2-web-kit | 0.3.2 | (no license field) |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| oneshot | 0.1.13 | MIT OR Apache-2.0 |
| option-ext | 0.2.0 | MPL-2.0 |
| ordered-float | 5.5.0 | MIT |
| ownedbytes | 0.9.0 | MIT |
| pango | 0.18.3 | MIT |
| pango-sys | 0.18.0 | MIT |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 |
| path-absolutize | 3.1.1 | MIT |
| path-dedot | 3.1.1 | MIT |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 |
| phf | 0.13.1 | MIT |
| phf_codegen | 0.13.1 | MIT |
| phf_generator | 0.13.1 | MIT |
| phf_macros | 0.13.1 | MIT |
| phf_shared | 0.13.1 | MIT |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 |
| plist | 1.10.3 | MIT |
| png | 0.18.1 | MIT OR Apache-2.0 |
| portable-atomic | 1.15.0 | Apache-2.0 OR MIT |
| portable-atomic-util | 0.2.8 | Apache-2.0 OR MIT |
| potential_utf | 0.1.6 | Unicode-3.0 |
| powerfmt | 0.2.1 | MIT OR Apache-2.0 |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 |
| precomputed-hash | 0.1.1 | MIT |
| prettyplease | 0.3.0 | MIT OR Apache-2.0 |
| proc-macro-crate | 1.3.1 | MIT OR Apache-2.0 |
| proc-macro-crate | 2.0.2 | MIT OR Apache-2.0 |
| proc-macro-crate | 3.5.0 | (no license field) |
| proc-macro-error | 1.0.4 | MIT OR Apache-2.0 |
| proc-macro-error-attr | 1.0.4 | MIT OR Apache-2.0 |
| proc-macro-error-attr3 | 3.1.1 | MIT OR Apache-2.0 |
| proc-macro-error3 | 3.1.1 | MIT OR Apache-2.0 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| quick-xml | 0.42.0 | MIT |
| quick_cache | 0.6.24 | MIT |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| r-efi | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| rand | 0.9.5 | MIT OR Apache-2.0 |
| rand_chacha | 0.9.0 | MIT OR Apache-2.0 |
| rand_core | 0.9.5 | MIT OR Apache-2.0 |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib |
| rayon | 1.12.0 | MIT OR Apache-2.0 |
| rayon-core | 1.13.0 | MIT OR Apache-2.0 |
| redox_syscall | 0.5.18 | MIT |
| redox_users | 0.5.3 | MIT |
| ref-cast | 1.0.27 | MIT OR Apache-2.0 |
| ref-cast-impl | 1.0.27 | MIT OR Apache-2.0 |
| regex | 1.13.1 | MIT OR Apache-2.0 |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 |
| reqwest | 0.13.5 | (no license field) |
| rle-decode-fast | 1.0.3 | MIT OR Apache-2.0 |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 |
| rustix | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| rustversion | 1.0.23 | MIT OR Apache-2.0 |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 |
| same-file | 1.0.6 | Unlicense/MIT |
| schemars | 0.8.22 | MIT |
| schemars | 0.9.0 | MIT |
| schemars | 1.2.3 | MIT |
| schemars_derive | 0.8.22 | MIT |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 |
| selectors | 0.38.0 | MPL-2.0 |
| self_cell | 1.3.0 | Apache-2.0 OR GPL-2.0-only |
| semver | 1.0.28 | MIT OR Apache-2.0 |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde-tuple-vec-map | 1.0.1 | MIT |
| serde-untagged | 0.1.9 | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive_internals | 0.29.1 | MIT OR Apache-2.0 |
| serde_json | 1.0.154 | MIT OR Apache-2.0 |
| serde_path_to_error | 0.1.20 | MIT OR Apache-2.0 |
| serde_repr | 0.1.21 | MIT OR Apache-2.0 |
| serde_spanned | 0.6.9 | MIT OR Apache-2.0 |
| serde_spanned | 1.1.2 | MIT OR Apache-2.0 |
| serde_urlencoded | 0.7.1 | MIT/Apache-2.0 |
| serde_with | 3.24.0 | MIT OR Apache-2.0 |
| serde_with_macros | 3.24.0 | MIT OR Apache-2.0 |
| serialize-to-javascript | 0.1.2 | MIT OR Apache-2.0 |
| serialize-to-javascript-impl | 0.1.2 | MIT OR Apache-2.0 |
| servo_arc | 0.4.3 | MIT OR Apache-2.0 |
| sha2 | 0.10.9 | MIT OR Apache-2.0 |
| sharded-slab | 0.1.7 | MIT |
| shlex | 2.0.1 | MIT OR Apache-2.0 |
| simd-adler32 | 0.3.10 | MIT |
| siphasher | 1.0.4 | MIT OR Apache-2.0 |
| sketches-ddsketch | 0.4.2 | Apache-2.0 |
| slab | 0.4.12 | MIT |
| smallvec | 1.16.4 | MIT OR Apache-2.0 |
| socket2 | 0.6.5 | MIT OR Apache-2.0 |
| softbuffer | 0.4.8 | (no license field) |
| soup3 | 0.5.0 | MIT |
| soup3-sys | 0.5.0 | MIT |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 |
| static_vcruntime | 3.0.0 | MIT OR Apache-2.0 OR Zlib |
| std-semaphore | 0.1.0 | MIT/Apache-2.0 |
| stop-words | 0.10.1 | MIT OR Apache-2.0 |
| string_cache | 0.9.0 | MIT OR Apache-2.0 |
| string_cache_codegen | 0.6.1 | MIT OR Apache-2.0 |
| strsim | 0.11.1 | MIT |
| swift-rs | 1.0.9 | (no license field) |
| syn | 1.0.109 | MIT OR Apache-2.0 |
| syn | 2.0.119 | MIT OR Apache-2.0 |
| syn | 3.0.9 | MIT OR Apache-2.0 |
| sync_wrapper | 1.0.2 | Apache-2.0 |
| synstructure | 0.14.0 | MIT |
| system-deps | 6.2.2 | MIT OR Apache-2.0 |
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
| tao | 0.37.2 | Apache-2.0 |
| tao-macros | 0.1.4 | (no license field) |
| target-lexicon | 0.12.16 | Apache-2.0 WITH LLVM-exception |
| tauri | 2.12.5 | Apache-2.0 OR MIT |
| tauri-build | 2.7.4 | Apache-2.0 OR MIT |
| tauri-codegen | 2.7.4 | Apache-2.0 OR MIT |
| tauri-macros | 2.7.4 | Apache-2.0 OR MIT |
| tauri-runtime | 2.12.2 | Apache-2.0 OR MIT |
| tauri-runtime-wry | 2.12.3 | Apache-2.0 OR MIT |
| tauri-utils | 2.10.2 | Apache-2.0 OR MIT |
| tauri-winres | 0.3.7 | MIT |
| tempfile | 3.27.0 | MIT OR Apache-2.0 |
| tendril | 0.5.1 | MIT OR Apache-2.0 |
| thiserror | 1.0.69 | MIT OR Apache-2.0 |
| thiserror | 2.0.21 | MIT OR Apache-2.0 |
| thiserror-impl | 1.0.69 | MIT OR Apache-2.0 |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 |
| thread_local | 1.1.10 | MIT OR Apache-2.0 |
| tikv-jemalloc-sys | 0.7.1+5.3.1-0-g81034ce1f1373e37dc865038e1bc8eeecf559ce8 | MIT/Apache-2.0 |
| tikv-jemallocator | 0.7.0 | MIT/Apache-2.0 |
| time | 0.3.55 | MIT OR Apache-2.0 |
| time-core | 0.1.9 | MIT OR Apache-2.0 |
| time-macros | 0.2.32 | MIT OR Apache-2.0 |
| tinystr | 0.8.5 | Unicode-3.0 |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT |
| tokio | 1.54.1 | MIT |
| tokio-macros | 2.7.2 | MIT |
| tokio-util | 0.7.20 | MIT |
| toml | 0.8.2 | MIT OR Apache-2.0 |
| toml | 1.1.8+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_datetime | 0.6.3 | MIT OR Apache-2.0 |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_edit | 0.19.15 | MIT OR Apache-2.0 |
| toml_edit | 0.20.2 | MIT OR Apache-2.0 |
| toml_edit | 0.25.17+spec-1.1.0 | (no license field) |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_writer | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 |
| tower | 0.5.3 | MIT |
| tower-http | 0.6.11 | MIT |
| tower-http | 0.7.1 | MIT |
| tower-layer | 0.3.3 | MIT |
| tower-service | 0.3.3 | MIT |
| tracing | 0.1.44 | MIT |
| tracing-attributes | 0.1.31 | MIT |
| tracing-core | 0.1.36 | MIT |
| tracing-log | 0.2.0 | MIT |
| tracing-subscriber | 0.3.23 | MIT |
| tray-icon | 0.25.1 | MIT OR Apache-2.0 |
| try-lock | 0.2.5 | MIT |
| typed-path | 0.13.0 | MIT OR Apache-2.0 |
| typeid | 1.0.3 | MIT OR Apache-2.0 |
| typenum | 1.20.1 | MIT OR Apache-2.0 |
| typetag | 0.2.23 | MIT OR Apache-2.0 |
| typetag-impl | 0.2.23 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 |
| unty | 0.0.4 | MIT OR Apache-2.0 |
| unwrap-infallible | 1.0.0 | MIT OR Apache-2.0 |
| url | 2.5.8 | MIT OR Apache-2.0 |
| urlpattern | 0.6.0 | MIT |
| utf8-ranges | 1.0.5 | Unlicense/MIT |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT |
| uuid | 1.29.0 | Apache-2.0 OR MIT |
| valuable | 0.1.1 | MIT |
| value-log | 1.9.0 | MIT OR Apache-2.0 |
| varint-rs | 2.2.1 | 0BSD |
| version-compare | 0.2.1 | MIT |
| version_check | 0.9.5 | MIT/Apache-2.0 |
| virtue | 0.0.18 | MIT |
| vswhom | 0.1.0 | (no license field) |
| vswhom-sys | 0.1.3 | (no license field) |
| walkdir | 2.5.0 | Unlicense/MIT |
| want | 0.3.2 | MIT |
| wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasip2 | 1.0.4+wasi-0.2.12 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasm-bindgen | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-futures | 0.4.79 | MIT OR Apache-2.0 |
| wasm-bindgen-macro | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-macro-support | 0.2.129 | MIT OR Apache-2.0 |
| wasm-bindgen-shared | 0.2.129 | MIT OR Apache-2.0 |
| wasm-streams | 0.5.0 | (no license field) |
| web-sys | 0.3.106 | MIT OR Apache-2.0 |
| web-time | 1.1.0 | MIT OR Apache-2.0 |
| web_atoms | 0.2.6 | MIT OR Apache-2.0 |
| webkit2gtk | 2.0.2 | MIT |
| webkit2gtk-sys | 2.0.2 | MIT |
| webview2-com | 0.39.1 | (no license field) |
| webview2-com-macros | 0.8.1 | (no license field) |
| webview2-com-sys | 0.39.1 | (no license field) |
| winapi | 0.3.9 | MIT/Apache-2.0 |
| winapi-i686-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 |
| winapi-util | 0.1.11 | (no license field) |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 |
| window-vibrancy | 0.8.1 | (no license field) |
| windows | 0.62.2 | (no license field) |
| windows-collections | 0.3.2 | (no license field) |
| windows-core | 0.62.2 | (no license field) |
| windows-future | 0.3.2 | (no license field) |
| windows-implement | 0.60.2 | (no license field) |
| windows-interface | 0.59.3 | (no license field) |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-numerics | 0.3.1 | (no license field) |
| windows-result | 0.4.1 | (no license field) |
| windows-strings | 0.5.1 | (no license field) |
| windows-sys | 0.45.0 | (no license field) |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| windows-targets | 0.42.2 | (no license field) |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 |
| windows-threading | 0.2.1 | (no license field) |
| windows-version | 0.1.7 | (no license field) |
| windows_aarch64_gnullvm | 0.42.2 | (no license field) |
| windows_aarch64_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_aarch64_msvc | 0.42.2 | (no license field) |
| windows_aarch64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_gnu | 0.42.2 | (no license field) |
| windows_i686_gnu | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_msvc | 0.42.2 | (no license field) |
| windows_i686_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnu | 0.42.2 | (no license field) |
| windows_x86_64_gnu | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnullvm | 0.42.2 | (no license field) |
| windows_x86_64_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.42.2 | (no license field) |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| winnow | 0.5.40 | MIT |
| winnow | 1.0.4 | MIT |
| winreg | 0.56.0 | (no license field) |
| wit-bindgen | 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| writeable | 0.6.4 | Unicode-3.0 |
| wry | 0.57.1 | Apache-2.0 OR MIT |
| x11 | 2.21.0 | MIT |
| x11-dl | 2.21.0 | MIT |
| xxhash-rust | 0.8.19 | BSL-1.0 |
| yoke | 0.8.4 | Unicode-3.0 |
| yoke-derive | 0.8.4 | Unicode-3.0 |
| zerocopy | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerocopy-derive | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerofrom | 0.1.8 | Unicode-3.0 |
| zerofrom-derive | 0.1.8 | Unicode-3.0 |
| zerotrie | 0.2.5 | Unicode-3.0 |
| zerovec | 0.11.9 | Unicode-3.0 |
| zerovec-derive | 0.11.7 | Unicode-3.0 |
| zip | 9.1.0 | MIT |
| zlib-rs | 0.6.8 | Zlib |
| zmij | 1.0.23 | MIT |
| zopfli | 0.8.4 | Apache-2.0 |
| zstd | 0.13.3 | MIT |
| zstd | 0.14.3 | BSD-3-Clause |
| zstd-safe | 7.3.0 | BSD-3-Clause |
| zstd-safe | 8.2.0 | BSD-3-Clause |
| zstd-sys | 2.1.2+zstd.1.5.7 | BSD-3-Clause |
