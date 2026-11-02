# Native and renderer notice addendum — 2026-10-30

This extends the earlier review; the historical package hashes in README.md
and current-components.json still identify the previously inspected package,
not an artifact rebuilt after these notice changes.

## ONNX Runtime: concrete provenance and notice scope

- Downloaded the exact upstream Linux x64 static 1.27.1 archive (23,603,000
  bytes), SHA256 `6b4df7fc46d3367b6be73fdea80dee323b9dc9eaa8dc50136a33d8524e7f06bb`.
  GitHub asset 488086792 independently reports the same digest. The archive
  contains headers and one static library, without license files.
- Its libonnxruntime.a is byte-identical to the active native cache's library:
  109,507,520 bytes, SHA256
  `9dbbd7bff1db5698d04ae6710a28e995a33d0409adc362f8ff2bb0bf7e8fc2ea`.
  The embedded build string identifies Release, version 1.27.1 and commit
  `df2ba1cf8`. Official v1.27.1 resolves to full Microsoft source commit
  `df2ba1cf8108aa63627cf4cdf8f807880b938616`.
- The archive publisher's README links static builds to
  [onnxruntime-build](https://github.com/csukuangfj/onnxruntime-build).
  [Successful Linux glibc217 run 30070551571](https://github.com/csukuangfj/onnxruntime-build/actions/runs/30070551571)
  used source `d225cd6b04034501792fe4ae2183292a3cc41223`. Its public job metadata
  shows Update, Build, Collect and Upload all executed successfully; asset
  creation is 2026-07-24 06:18 UTC, within that run's build period.
- Retained the exact workflow, shell recipe, static target list and bundling
  CMake script in distributable notices. Recipe: manylinux2014 x64,
  devtoolset-11, Release static, unit tests off, protoc 21.12, no GPU provider
  option requested. The builder removes a SOVERSION line. The explicit
  dependency targets include ONNX/ONNX proto, protobuf-lite, re2, Abseil and
  cpuinfo, alongside ONNX Runtime/MLAS/flatbuffers. 872 archive member names
  are retained; no new binary payload is stored in the repository.
- Existing ThirdPartyNotices.txt matches exact upstream source bytes:
  SHA256 `0e07b95f3a8d6230037707c5c4a2b554d12c4cb67369669ac255635528ffcee2`.
  It already includes the named bundled dependency licenses. Added original
  license/copyright files for 13 named binary/header dependency source trees,
  fetched from exact upstream deps.txt archives and checked against its SHA1s.
  The source dependency list is a conservative superset, not proof every
  optional library is linked.
- Concrete source-access omission addressed: Eigen's MPL 2.0 license was
  present but lacked an exact corresponding-source notice. Added an exact
  freely accessible Eigen source archive URL, its pin, both ONNX Runtime
  Eigen patches, and instructions to recreate the patched source. The original
  COPYING files remain included. No Eigen source change by Koett is claimed.

All retained file identities and sources are in
`packaging/notices/release-review-sources.json`; the archive and run receipt
is `packaging/notices/native/onnxruntime/provenance.json`.
This resolves the concrete static recipe/source identification gap for this
archive and supplies the identified source-access notice. It does not prove
an independent reproducible build, signed provenance or every member's full
source correspondence. Public build logs required authentication (HTTP 403);
no missing log content is invented. No additional known missing native license
text was identified for the reviewed CPU configuration.

## Nemotron: credits retained, external clarification remains

Read the pinned sherpa exporter and release workflow, the published converted
model README at Hugging Face revision
`52056fdc070914a48dcd68b31b44d6a6f5b85902`, and the current NVIDIA original
model card (exact revision retained in release-review-sources.json).

The exporter identifies copyright 2026 Xiaomi Corp., author Fangjun Kuang.
It loads NVIDIA's named model, exports its transducer branch with a 560ms
configuration and dynamically quantizes encoder/decoder/joiner to QUInt8.
It writes `model_author=NeMo` and the NVIDIA model URL as metadata. These are
exporter-source observations, not measurements of newly downloaded weights.
The converted README credits NVIDIA and chunk size; it does not state an
immutable original checkpoint or independent conversion model-license terms.

Added factual original-model/converter attribution and the original NVIDIA
terms/card. Preserved the canonical manifest's clarification gap. Exact
external question, also included in the distributable attribution:

> For release artifact SHA256
> 78e2b79fcf7271553a74402a76b771b09ea40117a39566a79f52235b23db6358,
> which immutable NVIDIA checkpoint was converted, and what conversion-specific
> copyright/attribution or license notices, if any, must accompany redistribution
> of the converted weights and included test audio? Confirm the original NVIDIA
> model license applies to that artifact and identify any additional notices
> required by its converter.

No contact was sent and no approval is implied. An application-only package
can include these accurate notices without bundling model weights. This review
cannot declare the Nemotron converted-model download cleared for redistribution.
No default model, manifest, feature availability or release policy was changed.

## Embedded Ask renderer: notice omission addressed

Copied original Marked, KaTeX, highlight.js and DOMPurify licenses/version
notices from tracked AssistantRenderer resources into the release notice tree.
Recorded the Apache-2.0 alternative for DOMPurify, retaining both license texts.

Examined all 20 WOFF2 name tables. Each includes separate SIL OFL 1.1 terms,
Design Science (2009–2010) and Khan Academy (2014–2018) copyrights and reserved
KaTeX font names. The existing KaTeX MIT text did not contain those font terms.
Added the complete OFL text, exact embedded copyright statements, per-font
hashes and original metadata. Cross-checked the standard license text against
[the official SIL text](https://openfontlicense.org/open-font-license-official-text/).
The metadata-derived attribution follows the actual shipped font bytes.

## Focused verification

- `python CrossPlatform/scripts/check_linux_notices.py`: 682 files validated,
  including all component/source provenance hashes and actual renderer font
  identities. The checker is executable.
- `python -m unittest discover -s CrossPlatform/scripts -p test_linux_notices.py`:
  six tests passed. Reindexed removal of OFL or Eigen source patches fails;
  replacing a component notice, changing a font or adding a font also fails.
- `uv tool run --offline ruff check` on both notice scripts: passed.
- Scoped `git diff --check`: passed.

Application/package rebuilding, package content verification and full branch
gates belong to the parent release task. Earlier source inventory gaps and
RustSec maintenance warnings were not suppressed or erased. No universal
compliance or release authorization is asserted by this addendum.

## Final independent-review corrections

Included KISS FFT's exact pinned `LICENSES/BSD-3-Clause`, referenced by its
COPYING file; retained both original files and their provenance. The final
notice tree contains 683 files. Authored ONNX source-access/provenance and
Nemotron attribution records are now hash-bound to the reviewed source index;
reindexing only files.json cannot silently replace those remedies.

Packaging validates canonical notices before creating an archive. Extracted
archive verification checks the canonical notice index, exact file membership
and every notice digest. Missing font terms, altered notice bytes and a replaced
index are rejected. Renderer-only source changes trigger CI so font-byte checks
cannot be bypassed by the workflow path filter. Eight notice tests and seven
packaging tests pass after these corrections; the source inventory is regenerated.
