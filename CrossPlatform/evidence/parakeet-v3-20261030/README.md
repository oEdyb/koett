# Parakeet v3 integration — 2026-10-30

Linux has four selectable models through Models, GTK menu, tray and agent CLI:
110M (unchanged default), English v2, multilingual v3 and experimental Nemotron.
The shared enum serializes v3 as `parakeet_v3`. Model identity, directories,
transactional installation, verified cache repair, asynchronous progress,
cancellation and persistence use the existing shared runtime. Nemotron backup
remains v2. No Swift or dependency changes.

NVIDIA source and license: https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3
(CC BY 4.0). The model card specifies automatic detection of 25 European
languages including English and Swedish. ONNX int8 conversion/distribution:
https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/nemo-transducer-models.html.
GitHub asset 283097583 independently downloaded to persistent
`~/.cache/koett/model-evaluations/parakeet-v3/model.tar.bz2`: 487170055 bytes,
SHA-256 `5793d0fd397c5778d2cf2126994d58e9d56b1be7c04d13c7a15bb1b4eafb16bf`.
Current primary asset metadata is `upstream-asset.json`. `verified-manifest.json`
records independently hashed required model files and four bundled public sample
WAVs. Silero was separately downloaded, byte/hash verified and remains MIT.
No model/audio binaries are included here. English fixture is k2's bundled
`test_wavs/en.wav` (JFK “Ask not…” excerpt); no Swedish fixture/accuracy claim.

Pinned sherpa-onnx 1.13.5 source commit
`3dc7c569f31ca2cd4a20ed6f7db780327e6714c5`,
`pinned-offline-transducer-nemo-model.cc` detects TDT via encoder URL metadata
(lines 224–225) and configures joiner duration outputs. Actual unchanged native
libraries loaded v3 and decoded English. `inference.log` is real CPU inference;
`chunk-reset.log` proves repeat decode, >=65-second bounded chunk submission,
and exact short decode after the long worker session. These are compatibility
smokes, not comparative accuracy or unrestricted performance measurements.

Primary final native receipt: `accepted/results.json`, `accepted/transcript.json`
and `native-accepted.log`. Final selected-detail screenshot was inspected.

Private `koett-v3` lab, frozen executable hashes in `binaries.sha256`:
- Light and dark GTK menu action started a real v3 network download, showed
  nonzero accessible progress within window bounds and enabled cancellation.
- Cancel retained active compact and its preference; the existing v2 harness
  and footer/layout assertions remain unchanged.
- Each then completed real download/install/verification/load, selected v3,
  retained warm v3 after an injected settings-save failure, and restarted with
  saved `parakeet_v3` and actual engine identity
  `sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8`.
- Dark private CPAL capture transcribed English, stored exactly one row carrying
  v3 identity and removed successful recording WAVs. Paste portal unavailable
  in the private lab. Copy is checked separately against a clipboard sentinel.
- Initial dark Copy capture had stale selected detail text and its `copy_exact`
  field is not accepted evidence. Final harness starts with empty history,
  verifies the selected new detail and proves agent `copy-last` replaces a
  private clipboard sentinel with the exact ASR text. Native header/footer Copy
  probes did not replace the sentinel in this lab (failed logs retained).
  This remains an explicit native Copy verification limitation; no unrelated
  clipboard implementation changes were made. Repeated final capture optionally
  copies verified public files after proving network cancellation; initial
  light/dark runs completed full network installation.


Focused gates: 22 model tests passed, 1 pre-existing ignored; 6 menu tests passed.
Strict all-target Clippy, formatting, release build, Rust 1.92 all-target source
check, Windows Rust 1.98 all-target source check with DOCS_RS/pkg-config overrides,
Ruff 0.17.2 and Python compile passed (logs alongside). No Windows execution,
Mac execution, full suite, heldout review, packaging or installation claim.
Frozen native binary hashes and the later final release hashes are recorded
separately. Only Rust test additions occurred between those builds; production
model/menu/runtime code was unchanged.

Shared repair mechanism is covered by existing focused model tests; no native
v3 corruption/re-download fault injection was performed. No physical input,
host preferences/history/clipboard/keys, host app restart or default change.
Parent owns final reviews, full gates, packaging and safe installation.

Final private source-output and sink-input lists were empty; lab `koett-v3`
was stopped after the accepted receipt. No owned app/downloader/audio stream remains.

## Parent review and integrated gates

Two fresh held-out code reviews returned no actionable findings. Parent inspected
the actual v3-active native capture. Full Rust suite:148 passed/10 existing
opt-in or benchmark ignores; Python unit gates:38 passed. Strict all-target
Clippy, release, MSRV1.92, Windows1.98 all-target source, affected Ruff and diff
checks pass. Native Copy and Swedish accuracy limits above remain unchanged.
