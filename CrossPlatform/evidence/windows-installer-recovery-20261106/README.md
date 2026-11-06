# Windows installer recovery acceptance — 2026-11-06

Real Windows 11 KVM guest, 4 vCPU / 8 GiB, guest-only input; Defender stayed enabled.
Local MSVC debug executables, NSIS 3.12-1 setup. These are not the final CI release
executables. `build-receipt.json` binds the final tested setup to source file hashes.
Screenshots are extracted unchanged from the adjacent guest receipts; focus records
include foreground HWND and PID. No executables, models or audio are committed.

## Reviewed behavior and results

- F1: `checks-final.json` passes ten recovery cases: changed/missing/unlisted files,
  abandoned staging/removal, same-version repair, actual partial deletion with ACL
  refusal and retry, malformed owner/current JSON, missing versions directory,
  native shortcut/startup/tools and mocked session filtering. The partial-delete
  test proves `0-first.txt` was deleted before the locked file refused deletion.
  Changed/user files are moved outside the app, never silently deleted.
- F2: four Python installer policy tests now assert their intended error text and
  allocate below the project target directory, independently of `/tmp`.
- F3: actual NSIS compilation with a failing uninstaller signer exits 1:
  `signing-failure.json`. Both finalizer directives require exit zero.
- F4/F5: downgrade wording explains uninstall-first/data retention; silent setup
  dialogs have defaults. `run-and-silent-exit.json` proves busy uninstall invoked
  directly with `_?=` exits 1 without hanging or killing Koett. Normal NSIS
  bootstrap `/S` returns 0 even when its child refuses (`silent-guards.json`);
  this bootstrap status limitation is recorded rather than claimed fixed.
- M1: `corrupt-launcher.json` proves corrupt pointer and missing payload produce
  foreground UI Automation error text and exit 1. Launcher errors have no silent
  default. The two error screenshots raced repaint and contain the setup background,
  so they do not prove the dialog pixels; the HWND/PID and UIA text are the evidence.
- M2: all four running-window properties and Start shortcut AppUserModelID read
  back correctly (`final-properties.json`). A pin made from the running window
  launches the current payload after two metadata revisions and original-version
  pruning (`pin-launch-after-upgrades.json`). The revisions reuse the same app
  version/binaries; they are not released version upgrades. BSTR window properties
  work in this guest, so no speculative VT_LPWSTR conversion was made.
- M3: actual cached FFmpeg, ffprobe, yt-dlp and a user config survive upgrade and
  same-version rerun with identical hashes. Carried tools complete real offline
  CPU WAV conversion (`media-after-upgrade.json`). When files are moved, setup
  and uninstall visibly name the preserved location (`finish-final5.json`,
  `apps-finish.json`).
- L1–L8: clear all four properties on destroy; recreate versions directory; clear
  malformed metadata remedy; fallback for empty failure report; Start shortcut
  identity; collision-safe sibling provenance note; corrected signing docs;
  native-mode recovery driver and actual-deletion assertion. Targeted Windows
  clippy, formatting, 24 packaging/workflow tests and inventory check pass locally.

## User-visible acceptance

`final-mic.json`: native preview on virtual HDA input. `held-hotkey.json`: actual
held shortcut starts recording, Cancel returns Ready. `root-paste.json`: stable
launcher forwards a WAV path containing spaces and dictation pastes once into the
same Notepad HWND/PID. Explicit recovery does not create a second history row.
`run-and-silent-exit.json`: the actual Run entry launches Ready without focus theft.
`apps-finish.json`: Settings > Installed apps uninstall removes root, Start link,
Run entry and uninstall registration while retaining application data.
`pristine-uninstall.json`: fresh install leaves startup off; clean native uninstall
has no extra/blank BOM-only dialog. `final-cleanup.json`: no Koett process, root or
startup entry remains. The expected public WAV transcript added one history row;
uninstall comparisons use the post-dictation baseline, not the older fixture hash.

A taskbar InvokePattern attempt was unsupported by Explorer; the completed pin
check uses a real guest click instead. A setup overlap hit the installer mutex;
the corrected harness waits for the old setup process to exit. These were harness
corrections, not omitted acceptance cases. The final setup was tested after both.

## Limits and decisions

No physical Windows PC/microphone, signing certificate, Internet download
SmartScreen reputation, Chrome, actual second interactive Windows session, or
login reboot was tested. The Run command was launched manually; session filtering
uses mocked process lists. No deliberate 120-second timeout was induced. The
final CI release build is separately gated and was not substituted into these
local debug receipts. Unsigned Windows downloads may warn in SmartScreen;
checksums do not authenticate the publisher. Publishing, signing procurement and
winget/store submission remain outside this task. Application data and local
clipboard history policy are unchanged. Shared Cargo feature additions are
Windows-only; no Mac/Linux runtime behavior changed.

The ACPI shutdown request did not complete within 50 seconds. An ordinary guest
`shutdown.exe /s /t 5` request powered it off; final controller status is false.
The lab taskbar pin is retained as test state; no host shell state was changed.
