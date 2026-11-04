# Linux transcription CLI verification

2026-11-04. First Linux parity slice, based on `origin/main` `e6fe25c`.
No app install, desktop input, model-default change, or history/clipboard access.

## Changes and decisions

Reuse the Mac Python URL/file wrapper and its flags/JSON schema. A separate small
Rust adapter calls the existing pinned model manager and long-audio transcription
engine. Keep v3 as the CLI default; the dictation app retains its own default.
No Windows platform or shared engine implementation changes. Linux scratch files
use its cache directory. Retain the original Mac cache identity.

## Fresh checks

- `python3 -m unittest discover -s scripts -p test_transcribe.py`: 7 passed.
  Covers source validation, downloader isolation, cache invalidation/engine
  separation, Linux file routing/no-cache, process failure and timeout.
- `cargo fmt --check`: passed.
- `cargo test --locked`: 370 passed, 14 ignored in the library suite;
  the separate native streaming ABI test also passed. Ignored tests are not proof.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo build --release --locked`: passed, including the CLI adapter.
- `bash -n scripts/install-transcribe-linux.sh`, `git diff --check`: passed.
- `cargo check --locked --target x86_64-pc-windows-msvc`: failed before checking
  this adapter because bundled SQLite could not find MSVC `lib.exe` on Linux.
  Real Windows verification remains with the Windows workstream.

Rust gates ran with `systemd-run --user --scope -p MemoryMax=12G nice -n 10 env
CARGO_BUILD_JOBS=4`; build output stayed in the worktree's `CrossPlatform/target`.
Models were copied into `target/cli-parity-sol/models`; `KOETT_MODEL_DIR` and
`XDG_CACHE_HOME` isolated the inference checks from user model/data directories.

## Real CLI checks

- Local upstream speech WAV, v3: JSON with nonempty raw text; 7.435 seconds audio,
  0.247 seconds ASR, 1.429 seconds total including model load and conversion.
- `https://www.youtube.com/watch?v=jNQXAC9IVRw`, v2: title `Me at the zoo`,
  source ID `jNQXAC9IVRw`, nonempty raw text; 19.006 seconds audio,
  0.336 seconds ASR, 3.827 seconds elapsed including download and model load.
- Repeat URL invocation: identical text with `cached: true`, elapsed rounded
  to 0.000 seconds. This proves cache reuse, not fresh inference.

Raw speech output remains unverified ASR and contains recognition errors.
The checks do not establish three-hour accuracy, every extractor's availability,
clean-machine installation, or Windows runtime behavior. Existing subprocess
limits/cleanup are reused; no UI lab or global key injection was needed.

Remaining Linux parity rows are unchanged, starting with Wayland Escape and pill
errors, followed by pin context, per-app paste/permissions, acoustic dictionary
bias and spelling around formatting, then the remaining audit rows.
