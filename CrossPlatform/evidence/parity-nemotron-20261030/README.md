# Nemotron Linux streaming increment

No installation, live preference changes, private audio, provider calls, commits or pushes.
All desktop/audio activity used agent-lab `koett-nemotron` (two CPU-equivalent quota,
6 GiB aggregate lab memory limit). This is not an unrestricted hardware benchmark.

## Implementation

Explicit experimental third model choice; 110M remains default. Uses pinned
sherpa-onnx 1.13.5 native OnlineRecognizer and the official English 560 ms ONNX
release. One warm recognizer, fresh stream per recording, ordered CPAL mono chunks
at the hardware rate (native sherpa resampling), drain during capture, endpoint
reset disabled, 300 ms tail plus InputFinished and final drain. No partial-text UI.

Parakeet v2 is verified and kept warm in the same worker for ASR recovery. Ask
uses the offline recording route; media stays on the offline Parakeet worker.
Model preparation retains the previous worker until load and preference save
succeed. Cancellation retains the previous model. Recording is refused during
setup; model choices are disabled during recording/transcription.

Capture queue is bounded to 64 chunks (2048 samples each). Overflow marks the
capture incomplete, stops streaming submission, preserves collected audio, and
forces offline recovery with a visible notice. Disconnect similarly preserves
known samples. Capture error no longer discards the collected recording.
Worker shutdown cancels the active session and joins; cancellation wakes idle
record receivers every 25 ms. Offline fallback keeps shared ownership of the
captured samples, including worker send/receive failure. Both ASR failures retain
a collision-safe 0600 mono PCM16 WAV at the original capture sample rate, with
its path in the visible error and a Failed recordings menu action. Captured
samples otherwise stay in memory and are dropped after ASR; successful ASR leaves
no recovery WAV, including subsequent delivery/storage failures. Existing
formatting, paste-before-storage and single final history event remain downstream
of ASR; those failures have no path back into the ASR worker.

## Model provenance and license boundary

Archive: https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemotron-speech-streaming-en-0.6b-560ms-int8-2026-04-25.tar.bz2

Verified locally: 463945051 bytes; SHA-256
`78e2b79fcf7271553a74402a76b771b09ea40117a39566a79f52235b23db6358`.
Per-file sizes/hashes are in `../../model-manifests/nemotron-streaming-en-560ms-int8.json`;
computed only after archive verification. The same installer also verifies the
existing pinned Silero artifact. Raw archive/extraction: `/tmp/koett-nemotron-eval`.

- Official model and 560 ms export: https://k2-fsa.github.io/sherpa/onnx/nemo/nemotron-streaming.html
- Exact API example: https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.5/c-api-examples/streaming-nemotron-c-api.c
- NVIDIA card: https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b
- Governing license: https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/
- Mac conversion: https://huggingface.co/FluidInference/nemotron-speech-streaming-en-0.6b-coreml

NVIDIA's card specifies NVIDIA Open Model License. FluidInference's metadata
also specifies that license, while its prose footer still says Apache 2.0.
That conflict does not establish Apache licensing for these weights. The official
ONNX archive contains a short NVIDIA-source/chunk-size README, without a full
license/NOTICE or exact NVIDIA revision receipt. Preserve attribution and resolve
redistribution notices/checkpoint revision before packaging. FluidInference's
CoreML conversion is not used on Linux. Same model family/configuration is proven;
checkpoint identity or equivalent CoreML quantization is not claimed.

## Evidence and boundaries

Public WAV direct native probe: repeated fresh streams, silence, 100 ms input,
53-second concatenated input, and cancellation passed. No endpoint text was lost
in the repeated long sample. Direct public output matches the words in the
published sherpa example; our output additionally inserts a comma. This is one
fixture comparison, not a WER evaluation.

Native release app: explicit selection, two separate real CPAL captures of the
public release WAV, distinct Nemotron model identity, exactly one history row
per capture, idle streams released, no success WAV, and saved-selection restart
passed in light and dark. Initial light stop-to-history: 342/292 ms; dark:
331/284 ms. These include history polling, not precise stop-to-ASR timing.
Warm Nemotron + v2 + GTK observed VmRSS/VmHWM: light 2146576 KiB, dark 1910816 KiB.
Direct debug model load was 1302 ms in the first isolated short probe. Later long
probe ran with concurrent lab work; its timings are not a comparable benchmark.

Injected stream/finalization errors recovered via real warm Parakeet v2. A stopped
backup worker after injected finalization failure retained a readable WAV.
Unit tests verify collision safety, 0600, original rate, readable sample count,
and captured samples surviving streaming queue overflow. Native model save-failure,
cancel retention, busy state, recovery-menu keyboard and unobscured captures
passed in both final light/dark native harnesses (see checks/logs).

TTS native private-WSS cancel fixture passed after the parent fix, without manually
clearing speech_notice. No live Cartesia call. Screenshot inspection is engineering
evidence, not user appearance approval.

Remaining acceptance: live microphone unplug/overflow fault injection, end-to-end
both-ASR inference fault UI and recovery-folder launch, delivery/history failure
under active Nemotron, S1 formatting + Nemotron native integration, offline Ask/media
native regression under the selected Nemotron worker, and distribution/license
closure. Existing source and other increment receipts cover these separate flows;
they are not new Nemotron acceptance results. Parent owns independent review and
final full-suite integration gates.


## Final gates

- `cargo test --locked`: 123 passed / 9 intentional fixture ignores; fixture
  tests listed above were also run explicitly. Shared worker/audio changes
  justified the final full Rust run after targeted checks.
- Targeted transcription: 13 passed / 4 opt-in fixtures; audio: 7 passed;
  model verification: 21 passed / 1 opt-in fixture (includes tamper/cancel tests).
- `cargo clippy --locked --all-targets -- -D warnings`: pass.
- `cargo fmt --check`, locked release build: pass.
- `cargo +1.92.0 check --locked --all-targets`: pass.
- Windows **source check**, `cargo +1.98.0 check --locked --target
  x86_64-pc-windows-msvc` with `DOCS_RS=1 LIBSQLITE3_SYS_USE_PKG_CONFIG=1
  PKG_CONFIG_ALLOW_CROSS=1`: pass. Stable has no Windows target installed;
  used the already-installed 1.98 target without installing anything. No
  Windows link/run claim. Parent's Linux-only speech guard is preserved.
- Ruff 0.17.2 and Python compile, affected Nemotron/speech harnesses: pass.
- Final private PipeWire source-outputs and sink-inputs were empty; lab stopped.

Native CPAL runs encountered unavailable compositor paste delivery, yet each
successful ASR still saved once and retained no WAV. This covers delivery failure
under Nemotron; an injected history write failure remains separate acceptance.

Final selected captures were visually inspected: light/dark 380px model chooser,
active identity, retained-engine preference error, recovery menu keyboard focus,
and cancelled speech without Speaking. No user appearance approval is claimed.
The small chooser scrolls to lower status/download content; this is observable
in the captures, not evidence of Mac-identical geometry.
