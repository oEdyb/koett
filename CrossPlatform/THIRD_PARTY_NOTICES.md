# Third-party notices

Distribute this file together with the complete `notices/` directory, copied
from `CrossPlatform/packaging/notices/`. Copyright and license texts remain
those of their respective authors. `components.json` identifies source crate
and native notice provenance; it is a source superset across Cargo targets,
not a claim that every crate is present in a Linux binary.

## Linux application and runtimes

Rust dependencies are pinned by `Cargo.lock`. Their original license and
copyright texts are in `notices/rust/`; Rust 1.98.0 toolchain notices are in
`notices/rust-toolchain-1.98.0/`. SQLite's public-domain dedication is retained
in the libsqlite3-sys source notices. License branch choices are recorded in
the source inventory policy; multiple original license texts are retained.
Koett locally patches sherpa-onnx 1.13.5 to omit its unused Rust TTS module,
matching the ASR-only native runtime. Original implementations remain unchanged;
the modification notice is in `notices/rust/sherpa-onnx-1.13.5/`.

The current native sherpa-onnx 1.13.5 build pins source commit
`3dc7c569f31ca2cd4a20ed6f7db780327e6714c5`, with TTS disabled. It links
sherpa-onnx, kaldi-decoder, kaldifst/OpenFst, kaldi-native-fbank, KISS FFT,
simple-sentencepiece and the upstream prebuilt static ONNX Runtime 1.27.1
archive. Original licenses and ONNX Runtime ThirdPartyNotices are under
`notices/native/`. Its exact archive/library hashes, embedded upstream source
revision, matching static build recipe and dependency notice records are in
`notices/native/onnxruntime/provenance.json`. Eigen's MPL source download and
the upstream patches are provided in `notices/native/onnxruntime/SOURCE-ACCESS.md`.
Historical blocked TTS/eSpeak receipts describe earlier
control builds, not the inspected ASR-only binary.

Some private/current-PC packages additionally distribute GTK 4.24.1 and
llama.cpp CPU b11981. These are conditional package contents, not universal
CI dependencies. GTK license and embedded source notices are under
`notices/native/gtk4/`; the package must also retain its exact source archive,
patch, provenance and reproduction helper in `sources/`. GTK is dynamically
linked and may be replaced by a compatible modified build. The LGPL text is
provided in GTK's COPYING. llama.cpp and its included third-party notices are
under `notices/native/llama.cpp/`. Other system libraries resolved from the
user's Linux installation are host dependencies, not bundled libraries.

## Embedded assistant renderer

Linux Ask embeds Marked 18.0.10 (MIT), KaTeX 0.18.4 (MIT), highlight.js 11.12.0
(BSD-3-Clause), and DOMPurify 3.4.14 (Apache-2.0 alternative). Their original
license texts are in `notices/renderer/`. KaTeX's embedded fonts separately
carry SIL Open Font License 1.1 notices, copyright Design Science, Inc.
(2009–2010) and Khan Academy (2014–2018), with reserved KaTeX font names.
The original embedded copyright statements and complete font license are
retained there alongside the font identities.

## Optional downloaded speech models

No model binaries are distributed in the application archive. The installer
can download models explicitly selected by the user, checking the canonical
manifest's hashes. `notices/models.json` retains source URLs, original model
references and artifact identities.

NVIDIA created **Parakeet TDT-CTC 110M**, **Parakeet TDT 0.6B v2**, and
**Parakeet TDT 0.6B v3**. Each is offered under Creative Commons Attribution
4.0 International: https://creativecommons.org/licenses/by/4.0/ . See the
included legal code and NVIDIA model cards under `notices/models/`.
The downloaded sherpa-onnx versions are k2-fsa's ONNX/INT8 conversions of
those NVIDIA models; Koett uses the converted artifacts described by its
manifests. Conversion/quantization changes the model representation. These
credits do not imply NVIDIA or k2-fsa endorsement.

Silero VAD is by Silero Team / snakers4 under MIT; its original copyright
and license are in `notices/native/silero-vad/LICENSE`. The downloaded ONNX
artifact is distributed through the sherpa-onnx model release.

**Nemotron Speech Streaming 0.6B** is an NVIDIA model governed separately by
the NVIDIA Open Model License, not the Parakeet CC BY license. The primary
terms are included in `notices/models/NVIDIA-Open-Model-License.html` and at
https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/ .
The downloaded 560ms INT8 ONNX conversion is published by k2-fsa/sherpa-onnx.
Its exporter identifies copyright 2026 Xiaomi Corp., author Fangjun Kuang;
it exports the transducer branch and dynamically quantizes the weights.
See `notices/models/NEMOTRON-ATTRIBUTION.md` for original model credit,
the pinned exporter, NVIDIA card and the exact unresolved upstream question.
The canonical Nemotron manifest still records a conversion-attribution
clarification gap. Factual converter credits do not resolve that gap.

## Review scope

The machine-readable review and remaining concrete gaps are in
`CrossPlatform/evidence/linux-release-notices-20261030/current-components.json`.
This notice collection does not assert complete release compliance.
