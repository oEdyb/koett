# Rust history storage

`HistoryStore` includes the canonical Swift resource SQL (schema version 2).
The database is `History.sqlite3` alongside the legacy `Transcripts.md`; new
writes use only SQLite. `open_default()` opens SQLite without reading legacy
files. Call `store.import_default_legacy()` explicitly once on a background
startup worker; page/get workers must never invoke import. Files use mode 0600 on Unix, with mode 0700 for newly
created data directories. SQLite uses DELETE journaling and FULL synchronization.

Upstream verification (2026-10-26): rusqlite 0.41.0 is MIT and requires Rust
1.89, below this crate's Rust 1.92 requirement. Its `bundled` feature selects
libsqlite3-sys's bundled SQLite; the resolved libsqlite3-sys 0.39.1 build script
explicitly sets `SQLITE_ENABLE_FTS5`. The bundled engine needs no system SQLite.

- https://github.com/rusqlite/rusqlite/blob/v0.41.0/Cargo.toml
- https://github.com/rusqlite/rusqlite/blob/v0.41.0/libsqlite3-sys/build.rs
- https://github.com/rusqlite/rusqlite/blob/v0.41.0/src/transaction.rs

## Legacy safety

Import hashes input incrementally, preserves original bytes in chunks no larger
than 256 KiB, and never modifies the source. Completed content hashes skip repeat
parsing. Entries with Mac markers keep their decoded stable IDs. Retry after
interruption rolls back atomically; a changed record with the same ID is an error.
Unmarked single-record Rust and old Mac files are recognized, including the
original Mac preamble. Repeated identical marked recordings remain distinct.

Unmarked archives containing multiple valid timestamp headings are structurally
ambiguous: the old format has no escaping or record length, so a dictated heading
can be identical to an entry delimiter. Valid UTF-8 in ambiguous or oversized
blocks is searchable as `Imported archive` chunks, with `created_at_ms = 0`
(Unknown date), no raw/model/date claims about individual dictations, and IDs
based on exact chunk content plus source byte offset. UTF-8 scalars are completed
at chunk boundaries; indexed chunk text reconstructs the original valid text.
Marked entries can contain ordinary Markdown or timestamp headings. Invalid
UTF-8 stays in raw fragments for review, without lossy decoding.

Import completion keys are versioned so a source previously imported only as
preserved fragments is indexed on its next explicit import. Original files and
all original bytes remain intact. Retrying the same source adds no duplicate
records, while identical chunks at different offsets remain distinct.

Source ownership uses `legacy-path:<canonical absolute UTF-8 path>` and version
keys use `searchable-archive-v3:<SHA-256>`. `history_sources` points each source to
its complete verified version; `history_import_records` retains the exact IDs
owned by each version. Parsing, second-pass digest verification, completion,
source activation and removal of obsolete searchable rows share one transaction.
Only rows owned by the previous version and absent from all active source versions
are removed. Raw fragments and historical completion/ownership records remain.
Returning to a previous digest reparses if its searchable rows were removed.
Public `save` records native ownership in `history_native_records`, including
when the same ID already exists; importing never claims native ownership.

Schema v1 recorded no native/source provenance. Its existing rows are protected
as native during the SQLite-only schema upgrade, rather than guessing which
recording or other source may own them. Explicit import establishes v3 ownership
for subsequent versions. Pre-upgrade duplicate rows may therefore remain; the
upgrade deliberately makes no heuristic deletion of historical recordings.

## Verification

From `CrossPlatform`, with the parent's verified ASR-only libraries staged:

```sh
cargo test --locked --lib history::tests
cargo test --locked --lib history::tests::benchmark_100k -- --ignored --nocapture
```

2026-10-26 Linux synthetic 100,000-row benchmark, warmed filesystem cache:

| Build | Open | Recent 50 | Common-term FTS search (50 rows) | Peak process RSS |
|---|---:|---:|---:|---:|
| Actual locked crate, debug | 0.274 ms | 0.133 ms | 75.6 ms | 28,728 KiB |
| Isolated actual history modules, release | 0.165 ms | 0.054 ms | 23.8 ms | 12,040 KiB |

The release harness uses the same rusqlite 0.41.0 / bundled libsqlite3-sys 0.39.1
but independently resolved supporting dependencies. RSS was measured using Python
`resource.getrusage(RUSAGE_CHILDREN)` around direct test-binary execution, excluding
Cargo/compiler processes and including fixture creation. These are synthetic
storage measurements, not microphone/capture-concurrency evidence.
