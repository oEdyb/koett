# Linux UI and history verification — 2026-10-26

Local candidate from `8e8cc356f8828fb806edb9aad179cbe6ddd7750a` on
`feat/linux-ui-history`. No public release, push or PR merge was performed.
macOS execution checks remain deferred at Olle's explicit request.

## Implemented

Native GTK recording/history/settings on Linux; native Swift searchable history
on macOS. Shared SQLite/FTS5 schema v2, 50-snippet keyset pages, selected-text loads,
literal search, Copy/Export, preserved legacy bytes and resilient completed-text
recovery. Linux includes hold/toggle, stable microphone selection, deterministic
replacements and optional MPRIS pause/resume. The Mac startup explicitly schedules
its once-only background import.

## Passed checks

- Rust full suite: 75 passed, 3 intentional benchmark/model probes ignored by the
  normal suite. Formatting, all-target strict Clippy and release builds pass.
- Ubuntu 22.04/GCC11/GTK4.6: native tests, lint, release build, model self-test and
  real GTK/AT-SPI UI checks under isolated Xvfb/Openbox.
- Current GTK/KWin: complete UI checks in private `agent-lab koett`, repeated from
  a freshly started lab and against the installed Ubuntu-built candidate.
- UI checks cover missing-model Retry, 50/15 paging, search/clear/selection,
  exact Copy on Wayland, native Export with exact file contents, microphone ID,
  X11 shortcut capture, replacements/media persistence, reopening settings,
  model recovery to Ready and empty-state visibility. X11 clipboard delivery is
  explicitly skipped by this UI script, not counted as a pass.
- 29 Python build/receipt/inventory/workflow tests, deterministic inventory checks
  with multiple Python hash seeds, actionlint and whitespace checks pass.
- Rust 1.92 MSRV check passes. Windows Rust source check passes with `DOCS_RS=1`
  and host SQLite/pkg-config; this is not Windows native linking or execution.
- Dependency audit: zero known vulnerabilities; upstream unmaintained-package
  advisories in the existing D-Bus stack remain informational.
- Actual locked release storage benchmark, synthetic 100,000 rows: open 0.188 ms,
  recent 50 rows 0.071 ms, common-term search 24.35 ms. These are storage timings,
  not full-app memory or capture-concurrency measurements.

## Runtime evidence and its limits

Earlier development-build checks completed real default microphone capture and
local ASR on niri. A public model WAV passed through actual CPAL virtual microphone
capture, ASR, a trusted Chrome `insertFromPaste` event and exact SQLite equality
in nested niri with a Swedish layout. A private KWin/KDE probe passed the same
capture/ASR/store path and real RemoteDesktop/Clipboard portal paste into a native
GTK text field. Neither probe programmatically assigned the destination text.
A separate 420-second synthetic long-input probe passed its repeated anchors and
isolated short utterance; it was not a seven-minute physical microphone session.

The private KWin shortcut-injection probe did not activate recording. Portal
registration/permission UI is available, but actual KDE hotkey activation remains
unverified; it is not counted as passed. GNOME, another wlroots compositor,
physical shortcut/permission/restart scenarios, terminal delivery and ARM64/native
Windows artifacts still need their platform checks before a broad release claim.

## Candidate and installation

Archive: `/tmp/koett-linux-candidate-final/Koett-Linux-x86_64-8e8cc356.tar.gz`

SHA-256: `39454ba28f0a96b49bd237882163e7fc90e06cf8c88580fe71c660ef79583338`

Executable SHA-256: `78bae91271891412952a616a0d247384104df06df553f58a8c549410409aabe7`
(48,483,416 bytes). The package receipt records a clean source tree. Installed only
under `/tmp/koett-linux-install-proof`; exact-byte comparison and launcher
validation pass. The installed binary passed the full private UI script.
The declared baseline remains glibc 2.35/GTK4.6; this executable's maximum GLIBC
symbol requirement is 2.34. Symbol and printable-string scans found zero eSpeak,
Piper or sherpa TTS signals. Native inputs come from the pinned ASR-only source
build and its receipt. Existing notice/SBOM gaps still block public-release
compliance; this local candidate does not clear those gates.

Small JSON receipts are in `CrossPlatform/evidence/linux-ui-20261026/`.
Native screenshots and full logs remain under `/tmp/koett-installed-ui-final/`,
`/tmp/koett-lab-repro-final2/`, `/tmp/koett-x11-proof14/` and
`/tmp/koett-native-research/`.

## Background-only testing

Codex and Claude global instructions now require background Linux testing.
The UI script defaults to the existing private `agent-lab koett`, rejects other
Wayland sessions, checks Xvfb for explicit X11 inner runs and clears inherited
host niri control. Lab accessibility, clipboard, input, audio and portals remain
private. Its KWin focus helper verifies the compositor's lab cgroup and targets
only the test process. The lab is stopped after verification. No future physical
session checks are authorized by this report; those need explicit permission.

## Deferred Mac and remaining product limits

Swift grammar, SQL/C shim and SQLite transaction probes passed; Swift/XCTest,
release compilation, native plist validation and Mac light/dark/keyboard/export/
recording checks did not run. Source-level checks do not substitute for them.

The portal uses Ctrl+V because it owns CLIPBOARD only. Terminals requiring another
paste chord need manual paste. Sending a key event does not prove consumption.
MPRIS cannot distinguish every same-state user pause or eliminate the final
check/Play race. V1 development history lacks provenance, so prior rows stay
protected and old duplicates may remain; new verified source versions reconcile
without deleting native records, other-source references or original bytes.

The vault project card/log were updated and their YAML parses. The canonical
vault routing checker is unavailable at both its documented Mac path and this
Linux checkout; a complete vault check was not claimed.
