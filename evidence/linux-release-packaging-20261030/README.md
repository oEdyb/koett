# Linux packaging verification, 2026-10-30

Scope: packaging/CI only, source HEAD 0d519a0414b36b4471e82907637f82217564d681
with uncommitted packaging changes. No commit, push, release, host install,
configuration, audio or visible desktop actions. Existing app untouched.

## Interface for parent documentation

Run from CrossPlatform:

```
python3 scripts/package_linux.py --flavor system-runtime --binary target/release/koett --output DIST
python3 scripts/package_linux.py --binary BINARY --output DIST --gtk-runtime GTK --asr-runtime ASR --llama-runtime LLAMA
```

Default flavor remains current-pc; all three private runtime inputs remain
required. ASR input supplies provenance; unused static archives are no longer
copied as runtime dependencies. System flavor rejects private runtime inputs.
Names: Koett-Linux-PLATFORM_MACHINE[-system-runtime]-HEAD8.tar.gz, plus SHA-256.
Optional `--native-build-receipt PATH` identifies the native libraries actually
linked into a supplied binary. System default: native-build/build-receipt.json;
private default: ASR/build-receipt.json. External binary builds must override
the system default when their native build lives elsewhere.
Both archives contain libexec/koett, launcher koett, install.sh, install.py,
preflight.py, files.json, desktop/icon, candidate.json and existing legal docs.
packaging/notices is copied as notices if supplied by the owning worker.
System archives contain SYSTEM-RUNTIME.md, private archives CURRENT-PC.md.

Extract, run `./koett --check-runtime`, install with
`./install.sh --prefix '/isolated path with $dollar'`, then use
`PREFIX/bin/koett menu-help` for JSON. System install runs required-loader/GTK
preflight before any prefix mutation. Default prefix is ~/.local. Python 3.10+
works (removed Python 3.11-only hashlib.file_digest from installer).

Required CI gates now package once through this interface, extract that actual
archive, check required loader dependencies, install/reinstall in a prefix with
spaces/$, run installed launcher JSON menu-help, verify old regular binary
backup and old-process survival, and reject tampering/missing loader deps.
Action pins, audit, dependency review and actual linked-inference self-test are
retained. Native/Rust Linux builds use two jobs; Linux timeout is 60 minutes.
CI upload remains a workflow artifact, not a GitHub Release publication.

## Verified private candidate

See current-pc-proof.json. Frozen binary SHA-256:
8d0bfd67711c9d4999191edc6c8e2d885823dae58e13e616e09953906deae36e

Archive SHA-256:
b878dc3204b6655b68a82e718f6162b934221510b2be6a5d1835dd7f98dbf232

Archive lives under ~/.cache/koett/private-tests/linux-release-packaging-20261030/current-pc-verified/.
This is CachyOS-only, not a portable Linux runtime. No GUI/audio claims.

## Verified Ubuntu 22.04 x86_64 system candidate

See ubuntu22-proof.json. Source build completed in 2m28s in the existing
koett-linux-floor:20261026 image, Rust 1.94.0, --release --locked, native libraries
from read-only koett-linux-native-floor, two jobs/two CPUs/4 GiB. The image's
Rust is 1.94.0; CI remains pinned to 1.98.0. No host packages were installed.
The binary was copied out and frozen before packaging/proofs. This is a source
build with the existing native ASR input, not a fresh native ASR rebuild.

Frozen binary SHA-256:
75074fffebcb91ed4e9b52baf9c354279f60948ba9994836805b26e3ff675aae

Final archive SHA-256:
b12221ce276c3129d78ef3e665561e535c320f3a027bbef464e7b5115aa522e4

Archive: ~/.cache/koett/private-tests/linux-release-packaging-20261030/system-runtime-final/Koett-Linux-x86_64-system-runtime-0d519a04.tar.gz

The archive passed extraction, required-loader/GTK preflight, real installed
launcher menu-help JSON, reinstall, old-process preservation, regular-binary
backup, tamper rejection and missing-loader rejection before prefix creation
inside the Ubuntu image, Python 3.10.12. Native provenance was explicitly supplied
from the floor native volume. This is an isolated prefix on a dependency-equipped
Ubuntu build image, not proof on a pristine desktop install. The build container
was removed; no owned resources remain running. GUI/audio were not exercised.

## Focused gates

Five packaging tests and six workflow-pin tests pass. Ruff 0.17.2, shell syntax,
Python compilation and git diff --check pass. Full Rust suite is parent-owned.
Existing gio desktop tests cover spaces, $, quotes, backslash, backtick and %.

## Documentation checked

https://docs.github.com/en/actions/tutorials/store-and-share-data
checked 2026-10-30. Retained exact action SHAs. Workflow artifact upload and
checksums are distinct from public release publication/authorization.

## Release boundaries

ARM64 native execution is unavailable locally and remains a CI gate. Loader
preflight and menu-help prove neither broad desktop behavior nor audio/input,
portal, model inference or accessibility. GTK 4.6 is the API floor; system GTK
4.24.1 needs upstream accessibility fix 21d3eb7. No accessibility/sandbox
bypass is introduced. Optional llama, Ask keys, WebKitGTK and media setup is
explicit in SYSTEM-RUNTIME.md. Final notice completeness, full CI, desktop
acceptance, parent README changes and release authorization remain parent-owned.
