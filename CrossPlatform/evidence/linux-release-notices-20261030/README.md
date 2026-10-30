# Linux notice review — 2026-10-30

Inspected source HEAD `0d519a0414b36b4471e82907637f82217564d681`, dirty shared
tree, and current-PC private package with application SHA-256
`8d0bfd67711c9d4999191edc6c8e2d885823dae58e13e616e09953906deae36e`.
`current-components.json` records package hash, Cargo.lock identity, Linux
resolved crate membership, native CMake source URLs/hashes, ELF direct dynamic
requirements and remaining review gaps. `package-files.json` records all
inspected package bytes, including GTK source archive and patch identities.
No package, runtime archive, model, recording or credential is stored here.

## Checks

- Verified official cargo-audit 0.22.2 release download digest against GitHub
  release metadata after confirming the crate version/checksum at crates.io.
  `cargo-audit audit --json` read the locked Cargo.lock without changes.
  RustSec database commit `ee1a49a2769a24a596afc1c20cb33748d2769586`:
  zero vulnerabilities, three unmaintained warnings, no ignores.
- RUSTSEC-2026-0370 (`zvariant` 5.15.0), 0371 (`zbus_names` 4.3.4), 0372
  (`zvariant_derive` 5.15.0) are maintenance advisories, not reported
  vulnerabilities. Primary RustSec pages are retained here. Runtime names/wire
  types reach the Linux application through zbus 5.19.0 from ashpd 0.13.13 and
  ksni 0.3.6; derive is a build-time proc macro. Migration is zbus >=6 with
  names/wire/derive integrated, requiring upstream ashpd/ksni compatibility
  work. No manifest changes or blanket advisory ignore applied.
- Official gitleaks release digest verified. Scanned all local Git refs,
  tracked source working tree, and inspected package text files. Directory
  scans limited to 2 MB per file; binary payloads are outside this scan claim.
  All 11 combined hits verified against actual source JSON lines as SHA-256
  digests. Only paths/classes/locations are retained in secret-scan.json.
- Eight focused inventory tests pass; canonical notice bytes validate; two
  consecutive generator/check runs agree; scoped git diff --check passes.

## Package worker handoff

Copy the entire `CrossPlatform/packaging/notices/` directory as `notices/` next
to `THIRD_PARTY_NOTICES.md`. Preserve relative file paths and bytes, including
files.json, components.json, models.json, original copyrights and license texts.
For packages bundling GTK, also preserve the exact sources/ archive, patch,
provenance and reproduction helper inspected in package-files.json. Retain
llama.cpp's own runtime LICENSE and all distributed library files.
Generic system-runtime CI packages must identify host GTK separately; do not
claim that the current-PC runtime receipt describes those artifacts.

## Still unresolved

The source inventory retains 33 gaps across platforms; it now covers 359
components, rather than silently erasing old gaps. Zero Linux resolved Rust
crate text gaps remain. Source notices are a conservative superset and do not
prove every linked transitive native component is covered.

The static ONNX Runtime archive is upstream prebuilt. Its exact embedded
component/build source correspondence still needs review against the upstream
archive build recipe; upstream MIT and ThirdPartyNotices have been collected.
Nemotron's canonical manifest explicitly requires upstream conversion
attribution clarification; its NVIDIA terms do not resolve that conversion
receipt. Parakeet 110M/v2/v3 attribution and CC legal code are provided
separately. No model bytes are bundled. These are review facts, not a legal
conclusion or complete release-compliance assertion.
