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

## Pause shortcut transfer — 2026-10-27

At Olle's request, the Niri shortcut is now `Pause repeat=false` ->
`~/.local/bin/koett toggle`, replacing Ctrl+Shift+Space. The actual previous
Voxtype handler was `voxtype-mouse.service`, a separate KEY_PAUSE listener;
Voxtype's built-in listener was already disabled. After confirming Voxtype idle,
the separate listener was disabled/stopped; daemon, binary and configuration stay
available. Niri config was backed up before a validated atomic update:
`~/.config/niri/config.kdl.before-koett-pause-20261027T100727Z`.

The installed executable passed actual private Niri Pause key119 start/stop,
private audio input and local ASR/store. No host input/audio test occurred. Lab
stopped. Receipt: `CrossPlatform/evidence/pause-shortcut-20261027/checks.json`.
Reversal: restore that saved Niri config, validate, then re-enable the separate
listener with `systemctl --user enable --now voxtype-mouse.service` when idle.

## Consistent paste selection — 2026-10-27

One persisted Settings choice now selects Ctrl+V or Ctrl+Shift+V for all Linux
output backends. The fallback no longer changes to Shift+Insert or overwrites
PRIMARY. Existing settings default to Ctrl+V. This is an explicit choice, not
automatic terminal detection: GUI and terminal defaults differ. Save/reopen
preserves the control, and invalid stored shortcut values fail validation.
History/cache recovery and original target guards remain in place.

Real private CPAL->ASR->clipboard->GTK and kitty consumption passed on both
RemoteDesktop/Clipboard portal and Niri/wtype paths, including Swedish/Japanese
Unicode, ordinary typing and Enter. Changing focus to Settings prevents insertion,
leaves the terminal unchanged and retains clipboard/final history text.
Receipts are under `CrossPlatform/evidence/paste-consistency-20261027/`.

82 Rust tests pass (3 existing intentional ignored probes), strict Clippy,
formatting, Windows source check, 29 Python tests and Rust1.92 source check pass.
Ubuntu22/GTK4.6 release build passes, with maximum required GLIBC2.34. Parent
verified native Settings selection/Save/reopen on that executable and inspected
`/tmp/koett-paste-floor-native8/settings-paste-reopened.png`. It uses actual native
popup actions and scrollbar interaction; unsupported GTK AT-SPI focus/scroll
methods and missing rendered-value label nodes were test harness assumptions,
not reasons to weaken persistence checks. A reviewed frozen Niri window-list
poll in the new test was corrected. Product review found no actionable findings.
X11 uses the same shared keysyms; new native X11 terminal consumption was not tested.
No physical desktop/input/audio/clipboard check was performed.

The exact Ubuntu baseline executable also passed the Niri/wtype GUI, terminal,
Unicode, typing/Enter and focus-change recovery journey:
`/tmp/koett-paste-floor-niri/result.json`. All owned desktop labs are stopped.
Installed SHA-256:
`c1489eaf813d74baa4c78d13d825afddad8e47953f2a1d06edcede7dce57ef72`
(48,828,544 bytes), backed up before atomic replacement. The prior app was idle
and reloaded with `--background`; its running hash matches the installed update.
No user model, microphone or paste preference was overwritten. Missing persisted
paste preference resolves to Ctrl+V until the user chooses otherwise.

The separately authorized controlled model comparison is recorded in
`docs/model-comparison-2026-10-27.md`: v2 improves over 110M on the bounded public
corpus; Whisper CPU ties v2 on its completed matched subset, while Vulkan model
loading failed under shared VRAM pressure. These are not personal accuracy claims.
The selected/default model remains unchanged. This work is local and uncommitted;
no public release or push occurred.

## Mac-style recording waveform — 2026-10-27

The Linux recording surface now follows the Mac source geometry: 236x48 dark
capsule, coral dot, 17 rounded rolling amplitude bars, separator and monospaced
timer. Real RMS uses the Mac dB mapping and smoothing; there is no synthetic
motion. Capture resets history; silence settles flat. GTK animation preference
changes clear/freeze bars and resume fresh samples. Transcribing stays visible
until delivery/cleanup; focus and pointer pass-through remain intact. No result
pill, model change or wider Settings redesign is included.

85 Rust tests pass (3 existing ignored probes), formatting, strict Clippy,
release build and Rust1.92 check pass. Ubuntu22/GTK4.6 baseline build passes,
maximum GLIBC2.34. Two independent source reviews found no actionable waveform
correctness defects. Existing native history/settings/recording UI checks pass
on that exact executable, including paste-choice persistence and preview handoff.
Dedicated private amplitude-step, silence, dynamic reduced-motion, timer, typing,
repeated capture, Transcribing, error and quit checks also pass. Native light/dark
screenshots and multi-frame GIF evidence are retained. Existing layer-shell
platform limits remain: no GNOME/X11 always-on-top claim.

Parent receipts: `/tmp/koett-waveform-floor-ui/` and
`/tmp/koett-waveform-floor-dynamic/`. Source evidence/summary is under
`CrossPlatform/evidence/waveform-20261027/`. Parent inspected actual rendered
frames; no visible desktop or physical microphone/input/clipboard test occurred.

One earlier reused-session GTK/AT-SPI SIGSEGV remains unexplained, with a stack in
GLib variant construction from GTK/GIO. The raw receipt is preserved as
`reduced-first-crash.log`. Clean runs and source review do not explain or erase
that failure; no causal waveform ownership defect has been established.

Parent also reran reduced-motion capture in the reused private session successfully:
`/tmp/koett-waveform-floor-reduced-reuse/results.json`. This does not establish
the cause of the earlier failure. All owned labs are stopped.
Installed executable SHA-256: `9457c230e71ca63b7f36443d5c841859ba7cc49ac1bd7c38d6b06c48400800eb` (48867720 bytes).
Backup and receipt: `/home/olle/.local/state/koett/install-backups/20261027T113436Z`.
The idle app was reloaded in the background and its running hash verified.
No preferences, model choice, microphone or Pause shortcut were changed.
Changes remain local and uncommitted.

## Settings and recording recovery — 2026-10-27

Settings now opens on Audio, with native Audio/Shortcuts/Text/General navigation
and a persistent Save footer. Hidden Audio releases preview capture. Validation
reveals Text and focuses the exact empty Phrase field. Sustained silence (five
seconds) and excessive peak levels (600 ms) produce measured warnings; ordinary
pauses/transients do not. Warnings do not stop recording or alter ASR text.

Typed delivery failure opens a nonactivating temporary Copy transcript action.
It accepts pointer input only during recovery, clears on capture/timeout, and
retains exact last-copy text afterward. Recording/transcribing remain pointer
transparent. Independent reviews found a guarded X11 skip reporting success and
an overly broad focus assertion; both are fixed, with regression coverage.

Final gates: 89 Rust tests pass, three existing intentional probes ignored;
strict all-target Clippy, formatting, release, Rust 1.92, Windows source check,
29 Python tests, harness Ruff/compilation and diff checks pass. Ubuntu 22/GTK4.6
native Xvfb checks pass for history, exact native export, Settings persistence,
paste choice and real-model Retry. Xvfb clipboard consumption is explicitly
skipped. Private KWin/CPAL checks on the final baseline binary pass real capture,
silence/loud signals, focus preservation, exact recovery Copy and timeout cleanup.
Light/dark and narrow native views were inspected. No physical-session tests ran.
Portal-injection failure and actual X11 guarded paste consumption were not rerun;
the latter has deterministic guard/recovery coverage. Mac checks remain deferred.

Parent receipts: `/tmp/koett-feedback-floor-ui2/`,
`/tmp/koett-feedback-final-native2/`, and
`/tmp/koett-feedback-floor-final-native/`. Renderer-specific screenshot limits
and final hashes are retained in the feedback evidence receipt. All owned labs
are stopped after verification. The installed baseline executable is 48881928
bytes, SHA-256 `3491b63438b502e17a69a018cde687aa3276f1d026d56c62572a39af11cead95`,
maximum GLIBC2.34. Its idle process was reloaded in the background; settings were
hash-verified unchanged. Backup:
`/home/olle/.local/state/koett/install-backups/20261027T121836Z/koett`.
No model, Pause binding, transcript processing, public release or commit changed.

## Thinking pauses are normal — 2026-10-27

Removed the silence warning from both Settings preview and recording feedback
after Olle reported it during thinking pauses. Quiet amplitude is not evidence
of microphone failure. Actual stream/device errors remain visible; sustained
excessive levels retain their existing debounced warning. Tests now assert no
warning through ten minutes of silence and retain loud-input/reset coverage.
The native regression also waits beyond the old five-second threshold in both
preview and recording and checks the warning stays absent while capture continues.
Final source, private native results and installation receipt are in
`CrossPlatform/evidence/silence-20261027/`.

89 tests, strict Clippy, formatting, locked release, baseline build and native
private quiet/loud/recovery checks pass. The fix is installed; runtime reload was
deferred because capture or Settings was active. Preferences are unchanged.

## Native menu pass — 2026-10-28

The tray now opens History on activation and exposes explicit Record, Stop or
Retry, with capture disabled while starting/transcribing. Shortcut/login controls
remain in Settings. The main header retains visible Settings and adds a grouped
native Menu for History, Copy last transcript, Settings, Hide window and Quit.
Copy is disabled before a result. Ctrl+, reuses Settings without discarding a
draft; Ctrl+F and Ctrl+Q keep their expected behavior.

Two fresh reviews covered action behavior, lifecycle and the GTK accessibility
FFI. One found X11 shortcut warnings replacing the engine phase; the fix routes
them as notices and has regression coverage. Final Rust tests: 93 passed, three
existing probes ignored. Formatting, strict all-target Clippy, locked release,
Rust1.92, Windows source checking, 29 Python tests, Ruff and Python compilation
pass. The exact Ubuntu22-built executable passed private KWin capture/menu/copy,
focus, Settings-draft, busy/Retry, hide/reopen and Quit checks. Actual tray actions
were dispatched through a disposable D-Bus watcher. Light/dark and normal/narrow
native screenshots were inspected. No physical desktop interaction occurred.

Final receipts and exact source/artifact hashes are under
`CrossPlatform/evidence/menus-20261028/`. Rendered tray visuals, physical Pause,
portal paste and Mac runtime checks remain unverified for this pass. Existing
platform/release gates are unchanged. All owned labs are stopped after testing.
