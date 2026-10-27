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

## Recording pill and Settings follow-up

The user-installed candidate above lacked the existing recording-pill contract.
The follow-up adds optional GTK layer-shell feedback and reuses/activates Settings
with the user's Wayland token, including tokens cached by GDK before Rust main.
An actual private Niri keybinding now foregrounds an existing Settings window.
Private Niri/KWin proofs also show capture above a typing target without stealing
focus, live microphone levels, transcribing feedback and cleanup. No host input,
clipboard, microphone or foreground windows were used for these checks.

Current checks: 78 Rust tests pass (3 intentional ignored probes), formatting and
strict all-target Clippy pass; 29 Python tests and Ubuntu22/GTK4.6 release build
pass. Parent reran Rust tests, formatting, Clippy and inventory verification.
Windows source and Rust1.92 checks pass. The baseline executable still requires
at most GLIBC2.34 and has no mandatory gtk4-layer-shell shared-library dependency.
GNOME/X11 and missing-library sessions retain in-window feedback, without an
always-on-top claim. Broader platform/release limits above remain in force.

Native receipts: `/tmp/koett-pill-niri-keybinding-fixed2/result.json`,
`/tmp/koett-pill-niri-keybinding-fixed2/settings.png` and
`/tmp/koett-pill-ui-receipt/results.json`. Both owned labs were stopped afterward.
User executable replacement is atomic and backed up; the running old process is
left alone, so the new UI takes effect after the user quits and reopens Koett.

English quality remains unresolved. A separate verified 600M v2 trial works with
the current runtime, using about 1105MiB peak RSS versus 332MiB for 110M on two
public short fixtures. These samples do not establish better personal accuracy.
No selected/default model was changed. The private trial receipt remains outside
the repository at `~/.cache/koett/model-evaluations/v2-trial-receipt.json`.

Installed follow-up executable SHA-256:
`c43bdb263c8f9c508f57db6adc7c1bcc844b76c73194bce556ac518af34ae63b`
(48,606,736 bytes). Prior executable and install receipt:
`~/.local/state/koett/install-backups/20261026T161938Z/`.
Exact installed-byte comparison passed; the user app was not launched or restarted.

After a later keybinding failure report, read-only inspection found the old
executable still running. With no active microphone stream, Koett was gracefully
restarted using `--background`; its running executable hash matches the installed
follow-up. No host key/input/capture test occurred. Exact Ctrl+Shift+Space with
Swedish layout passed start/stop/ASR in private Niri with both old and new binaries:
`/tmp/koett-keybind-check-physical-20261026/result.json`. The physical-session
failure remains unproven pending user retry. The lab was stopped.

## Selected microphone meter — 2026-10-27

Settings now previews the selected microphone, including unsaved selections,
with a labeled native Input level meter. Opening runs off the UI thread. Preview
keeps only instantaneous levels, not a waveform or transcript; Save/close/switch
releases its capture stream. Invalid inputs show an error without fallback.
Generation checks reject stale levels/errors. Dictation reserves capture priority,
waits up to 500ms for preview release and reports a retryable error if it stalls;
preview pauses while recording and resumes afterward. The wider UI redesign is
outside this change.

81 Rust tests pass (3 existing intentional ignored probes), formatting, strict
Clippy, Windows source check and Rust1.92 check pass. Ubuntu22/GTK4.6 release build
passes with maximum GLIBC2.34. Two fresh independent code reviews found no
high-confidence actionable issues. All input/audio/GUI proofs ran in private
agent-lab with synthetic signals and isolated state, never physical devices.

Implementer native proof: `/tmp/koett-preview-ui-final/results.json`; inspected
screenshot: `/tmp/koett-preview-ui-final/settings-selected-input-signal.png`.
Proof covers signal, silence, unsaved switching, zero/error without fallback,
Save/close cleanup, one stream during real dictation and safe preview resume.
Parent baseline native rerun: `/tmp/koett-preview-floor-native/`.

Parent baseline executable native rerun passed all 19 checks and its real screenshot
was inspected. The lab is stopped. Machine-readable receipt:
`CrossPlatform/evidence/microphone-preview-20261027/native-checks.json`.
Executable SHA-256:
`84823738e620507deca13d344428dde91b2e25e6273baa94253f6229ac01b076`
(48,842,136 bytes). No dependencies changed. Mac execution remains deferred.

Installed atomically at `~/.local/bin/koett`, exact-byte hash verified. Backup:
`~/.local/state/koett/install-backups/20261027T093802Z/koett`.
The user began recording before reload, so PID3195381 was deliberately left
untouched; this feature loads on the next quit/reopen. No host input/capture test
was performed, and Klippt remained running.
