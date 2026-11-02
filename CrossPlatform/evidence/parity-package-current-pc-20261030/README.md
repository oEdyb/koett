# Current-PC package handoff

Reviewable private candidate built from frozen app 37705a44, not a public release.
No commits, pushes, host installation, installed-app restart, system GTK change,
cloud request, physical audio or user settings/history/key reads were performed.

Package:
`/home/olle/.cache/koett/private-tests/koett-package-currentpc/final/Koett-Linux-x86_64-37705a44.tar.gz`

SHA-256: `405e8f19bf8b68f81afae003b9fde221d153822a0f312fdd4b5560f1f3119bdf`.
Installed launcher:
`/home/olle/.cache/koett/private-tests/koett-package-currentpc/clean install/bin/koett`.
Exact binary/runtime/source hashes and version tree are in receipt.json.

## Changes

Versioned app tree with a single atomic launcher switch; regular old binaries get
content-addressed sibling backups. Existing version trees remain available.
No running process is stopped. Config/data are outside the package and untouched.
Integrity verification precedes deployment. Desktop Exec quoting handles spaces,
quotes, dollar signs, backticks and literal percent signs. Runtime paths are
app-scoped, arguments and activation environment pass through exec unchanged.

GTK was actually rebuilt with its persistent cache prefix, validated by the
existing source/protocol/Meson-option checks. Its library hash changed to
`a6a8876857058a9b78ef681c9f57465a2cc664a8baa9a5123c7660598c51ef5a`.
The package contains GTK library/resources, full source archive, upstream patch,
license, build helper and provenance. ASR-only floor libraries and their build
receipt are bundled. CPU llama.cpp b11981 was re-extracted from the digest-verified
public archive into persistent cache, then bundled with MIT license/provenance.
No model is bundled. Native dependency/licensing review still blocks any public
release; this GTK depends on the current CachyOS host, not Ubuntu 22 portability.

## Verified

- Actual installed launcher in a path with spaces, both native light/dark suites.
- History search/selection/Copy/Export, Settings preview/signal/silence, persistence,
  missing input errors, recording pill/focus/click-through and cleanup.
- Real CPAL private monitor -> public compact-model sample -> ASR/completion/save.
  Public models were copied after manifest checksum verification; no writable
  model verification cache links point to user assets.
- Restart twice: JSON menu/recovery controls, loaded engine, Hold preference,
  Copy Last exact bytes, history row count unchanged. See restart-results.json.
- Packaged GTK loaded in own PID maps and passed 20 visible/retired-root child
  rounds with clean exit. See gtk-root-results.json and loaded-maps.json.
- Packaged llama-server version and real bounded localhost inference with a
  copied checksum-verified S1-mini model. This proves runtime execution only;
  its simplistic completion fixture is not product formatting quality evidence.
- Installer integrity rejects tampering; reinstall is stable; quoted desktop path
  tested. Missing runtime preflight fails truthfully. Required loader check passes.
- Ruff 0.17.2, shell syntax, diff check, one packaging unittest and six existing
  GTK build-helper tests pass. No Rust product source changed; full Rust gates,
  Windows/MSRV source checks and final installation remain parent-owned.

The existing UI harness has a stale assertion for the removed “Managed by desktop”
label. Focused test_linux_package_ui.py substitutes a stronger check for all three
current unassigned labels and leaves its remaining native/capture assertions intact.
Initial failed runs and successful runs remain under the private cache directory.
The lab portal required its desktop entry in the lab's own data/applications;
only that private profile was supplied. No host desktop entry was installed.

## Optional capabilities and limits

preflight.py and CURRENT-PC.md ship actionable setup instructions. Host WebKitGTK
6 is absent; rich Ask is NOT claimed exercised in this candidate. Install the
complete distribution package with helpers/resources/sandbox support and restart
for rich Ask; plain fallback remains the product path. The earlier verified
WebKit archive/signature were persisted under
`~/.cache/koett/runtime-builds/webkitgtk-6.0-2.54.2-1`; no experimental mount wrapper
or sandbox-disable environment is used by the candidate.
Media tools, Secret Service, GStreamer and compositor utilities remain optional
host dependencies; dependency-inventory.json records resolved paths. No provider
or account is required for ordinary local dictation. Product S1 controls, optional
rich Ask, physical-seat behavior and broad distro support remain unverified here.

## Cleanup and next step

Zero private source-output streams remained; owned app/server fixtures exited.
Lab koett-package-currentpc is stopped. Installed ~/.local/bin/koett and system GTK
were not modified. Large artifacts/screenshots/models stay in private cache.
Parent: review source/receipt, run the full integrated gates, then perform the
separately authorized idle-safe host installation. No scope expansion was used.

## Parent review and final candidate

Both independent reviews reproduced invalid desktop-entry escaping for special
prefixes. Parent now encodes the quoted argument and desktop string separately,
using `/usr/bin/env --` so GLib expands a literal percent in an argument rather
than treating it as part of the initial executable lookup. Actual isolated
`gio launch` fixtures pass for spaces, dollar signs, backslashes, double quotes,
backticks and percent signs. Equals signs are rejected before deployment.
Installer integrity/reinstall tests still pass. Media preflight now checks the
exact executable paths used by the product.

The updated package and hashes are recorded in `parent/package.json`; the initial
worker archive above remains historical. The installed private launcher includes
both spaces and a dollar sign. Parent exercised actual product S1-mini preparation,
private CPAL/public-fixture ASR, final/raw history, forced formatter death with
exact raw recovery, Raw persistence and exact Copy Last. See
`parent/product-formatting.json`. This establishes the product execution path,
not formatting quality or meaning preservation. An earlier run wrote its passing
assertions before the enclosing scope returned SIGTERM. The final run explicitly
clears its owned clipboard daemon and exits zero; the earlier exit's cause was
not independently established. All final
owned streams/processes ended and the private lab stopped.

Parent Rust suite:146 passed/10 existing opt-in or benchmark ignores. Packaging
and prior Python gates pass; actual total is recorded at final installation.
The packaged app and GTK bytes are unchanged by the installer-only review fix.
Rich Ask still requires complete host WebKitGTK6 and is not exercised here.
Selected actual light Settings/dark recording captures were inspected and copied
under `parent/`. No public release or appearance approval is implied.

## Host installation

The parent installed the reviewed candidate while the previous app reported
Ready, had no capture/preview stream, and had no Settings dialog open. The old
regular binary remains as a content-addressed sibling backup. After graceful
quit, the new launcher started with `--background` and reached Ready with the
same model. Executable and loaded GTK hashes match the private tested package;
preferences were byte-identical before installation and after startup. See
`parent/host-install.json` and `parent/gates.json` (146 Rust/38 Python passed).
The Niri Pause binding still invokes the installed `koett toggle`; it was read,
not activated. No host microphone/clipboard/input/window test was performed.
System GTK and model choice remain unchanged. Rich Ask's WebKit prerequisite
and broader platform/appearance/live-provider limits above still apply.
