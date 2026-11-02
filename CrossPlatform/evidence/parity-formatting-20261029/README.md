# Optional Linux formatting — 2026-10-29

Shared branch `feat/linux-ui-history`; previous model-management diff preserved.
No commit, push, install, live preference edit, Swift/default-ASR/schema/vendor
change or dependency addition. All native work used private `agent-lab koett-formatting`.
Fixtures live under `/tmp/koett-format-*`; selected captures are copied here.

## Implementation and acceptance

- `src/formatting.rs`: official Q4_K_M download with pinned revision, byte count,
  SHA-256, staged file, progress, cancellation and bounded HTTP. External
  `llama-server` prerequisite checked before downloading. Local authenticated
  ephemeral-port server, bounded readiness/tokenization/completion, CPU threads 2,
  GPU layers 0, measured N7/M48 speculation, official prompt/control line,
  greedy decoding and thinking disabled. Output rejects missing, empty and
  truncated results. Drop kills and reaps the owned child.
- `src/runtime.rs`: independent setup worker and cancellation, warm server while
  enabled, capture remains available during setup, idle-only configuration,
  persisted choices, optional-startup failure falls back to raw. Preparation
  thread stays alive for Linux parent-death binding until the warm child closes.
  Late cancellation is checked before saving/enabling the candidate.
- `src/linux/formatting_view.rs`, menu/window/theme: persistent transient native
  popup, four labeled rows, 16 inset, 160 control width, compact title/type,
  Raw/S1-mini, four styles, Prose/Lists, General/Email. Progress/errors/Cancel,
  capture/transcription/setup sensitivity, Escape/outside dismissal.
- Existing delivery and raw-history APIs retain paste-before-save, one row per
  dictation and original ASR text when cleanup succeeds. Linux replacements retain
  their existing ordering. Windows gets source-only raw-history propagation.

The native Output control is currently a chooser, whereas Mac uses a segmented
control. Idle popup is approximately 300 wide; GTK system metrics make it taller
than the Mac's nominal 190. This is compact source-referenced implementation,
**not visual approval or screenshot parity**. Whole app/menu/model-dialog visual
completion is not claimed. Searchable Linux History remains intentionally.

## Exact gates

All commands run from repository root, with
`SHERPA_ONNX_LIB_DIR=/tmp/koett-native-host-abi0/lib` for host Rust builds.

- `cargo fmt --check --manifest-path CrossPlatform/Cargo.toml`: `fmt.log`.
- `cargo +1.92.0 test --locked --manifest-path CrossPlatform/Cargo.toml`:
  `test.log`, **103 passed / 3 existing ignored**.
- `cargo clippy --locked --all-targets --manifest-path CrossPlatform/Cargo.toml -- -D warnings`:
  `clippy.log` (installed default Rust 1.94; 1.92 has no clippy component).
- `cargo build --release --locked --manifest-path CrossPlatform/Cargo.toml`: `build.log`.
- `DOCS_RS=1 LIBSQLITE3_SYS_USE_PKG_CONFIG=1 PKG_CONFIG_ALLOW_CROSS=1 cargo +1.98.0 check --locked --target x86_64-pc-windows-msvc --manifest-path CrossPlatform/Cargo.toml`:
  `windows.log`, source only. Installed MSVC target belongs to 1.98, not default.
- Ruff **0.17.2** and `python3 -m py_compile` on the four affected native harnesses:
  `ruff.log`. No GUI unittest discovery.
- `git diff --check`: clean.

## Native and real-model evidence

- `light/`, `dark/`: actual private native popup at normal/narrow sizes, raw,
  progress/cancel/error and disabled recording state. Inspected after capture.
  The loading capture briefly pauses only its owned server child, then resumes it.
  Record remains enabled during formatting setup; `@format-*` protocol messages
  are consumed before the recording/status UI.
- `e2e/`, `raw-final.json`, `native-e2e.log`: public sherpa ASR `test_wavs/0.wav`
  through private PipeWire into actual ASR and S1-mini. One successful dictation
  produces exactly one row with raw+final. Killing only its owned llama child,
  then recording the same public sample, produces one further raw-only row with
  exact original ASR text. Raw persists after disabling. No host audio/clipboard.
- `real-inference.jsonl`: actual official pinned model on CPU, four styles,
  Lists and Email, plus three synthetic meaning probes. Number/date/negation
  probe and two meaning checks passed their literal requirements. The fictional
  name probe **failed**: “maple or maybe birch” became “birch”. This is measured
  evidence against calling cleanup meaning-safe. The model remains optional.
- `artifacts.json`: official evaluated runtime b11981 CPU archive URL/digest and
  official model pin. Both downloads verified locally; runtime stays isolated
  under `/tmp/koett-format-eval`, never installed or set as a live default.
- `model-checks.json`, `model-inference.json`, `native-models.log`, `models/`:
  parent's updated `test_linux_models.py` ran privately, including failed read-only
  preference save, distinct real compact/v2 inference, cancel, busy refusal,
  Settings merge, and **active v2 after restart through Menu**. Harness click
  lookup now requires an actionable accessible node; same-name labels cannot
  swallow restart navigation. No acceptance assertion was removed.
- `startup-cancel.log`, `startup/`: optional v2 startup Cancel surfaces the
  cancellation error, with no capture streams. No giant network-loss fixture.
- `native-fixture.py`, `native-e2e-fixture.py`, `startup-cancel-fixture.py` preserve
  generated native probes. They reuse the existing model harness preamble/helpers;
  pass the same `--lab`, `--binary`, unique `/tmp --output`, `--theme` arguments.
  Normal UI fixture runs with evaluation runtime directory on PATH so real model
  download can begin and cancel; e2e links only the verified evaluation model into
  isolated fixture cache. Native model full rerun used `--renderer cairo`.

## Remaining gates and parity

No real Mac reference capture exists; `docs/mac-parity-reference-2026-10-29.md`
is source-derived. UI Atlas is absent on this PC and offline UI index is absent;
no web widget/framework replacement was used. Mac source remains primary.

Parent retains independent reviews, GTK 4.6/runtime floor, installed acceptance,
physical devices/compositors, visual approval and publication. Lab lacks installed
app registration, so portal shortcuts/paste warn and fallback; those warnings are
not formatting failures or evidence of installed integration. No Mac/Windows
native execution was done. Runtime installation/distribution remains a packaging
prerequisite; no credentials or cloud calls were needed for formatting.

Full remaining Ask, media, TTS, menu/setup/permission/recovery/shortcut, experimental
engine and packaging parity stays in `docs/linux-parity-inventory.md`. No inactive
Ask/media/TTS controls were added.
