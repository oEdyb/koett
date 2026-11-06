# Windows distribution gates

M5 merged in #153 as `8a93b7739ad2efbed7690bd01650c51dc7395204`
after all six checks passed on `94a50ea`. M6 remains a release gate, not a
three-platform release claim. The current work hardens the unsigned x64 preview.

## Preview package

The Windows archive contains the native GUI, app/media console, offline WAV
adapter, native-build receipt, canonical original notices, native build sources,
Windows instructions and `manage-preview.ps1`. `preview-release.json` records
architecture, source, explicit unsigned status and SHA-256 for every payload file.
The archive verifier rejects missing/unlisted/tampered files, unsafe paths,
symlinks, mixed executable architectures and non-system DLL dependencies.
Checksums establish integrity, not publisher identity or SmartScreen reputation.

The preview manager installs below LocalAppData with no elevation. Each verified
payload gets its own version directory; the current pointer is replaced atomically.
Upgrade and rollback update the Start shortcut and preserve startup only when it
already points into that preview installation. They do not enable startup or
replace another installation's startup entry. Running Koett must be closed first.
Uninstall verifies owned versions before removing them, refuses unexpected files,
and preserves application data. This is manual preview management, not MSI/MSIX,
a signed installer, a system installation or an automatic updater.

PowerShell reads its own JSON as UTF-8, including Unicode installation paths.
Native commands return JSON with errors and fixes. Execution-policy bypass in the
preview instructions applies only to that PowerShell process; it is not a machine
policy change or a public-release recommendation.

## Runtime hardening

Settings → Dictation options exposes the existing original recording cues,
Include page details in pins, and selected text (on by default, matching Mac/Linux). Windows reuses the existing
CPAL cue implementation; Linux source and playback behavior are unchanged. Cues
follow capture start/stop, and cancellation remains silent.
Output-device work stays off the GUI/audio-capture callback.

Page details off omits pin text context before UIA. A failed settings read also
omits context. Explicitly requested media-source reading remains independent.
Native controls save each option without a later Settings Save reverting it.

Transcript recovery exposes pending raw text, Copy unsaved and Retry save.
Snapshot/retry work runs one operation at a time on a private worker, not the GUI thread.
Copy checks clipboard sequence again inside the clipboard transaction, preserving
newer copies. It never discards pending records. Retry uses the shared resilient
store and removes records only after persistence. The view refreshes when the
pending count changes. Native Quit warns with No selected; CLI Quit and normal
session shutdown refuse unsaved records. Recovery-window keyboard routing leaves
Tab/Enter/Escape to the dialog manager while preserving text selection/Copy.

## Remaining release gates

- **Signing and distribution:** no signing identity/service is provisioned, no
  signing spend is approved, no signed x64/ARM64 installer or SmartScreen outcome
  is established. Publisher eligibility and licence/account decisions need the
  owner. Do not publish an unsigned preview as a signed release.
- **Dependency closure:** FFmpeg/ffprobe/yt-dlp and the optional S1-mini runtime
  were private lab copies and are absent from the archive. Final redistribution,
  pinned artifacts, complete notices/source obligations and runtime packaging
  remain work. Missing media tools are visible failures, not complete media
  support in a fresh archive.
- **Software parity:** bare-modifier global dictation bindings remain open.
  Complete first-run Windows privacy guidance and every remaining partial row in
  [feature parity](feature-parity.md). Preview management does not establish the
  production signed-installer/update contract.
- **Native ARM64 build:** the current native builder rejects Windows targets
  other than x64. ARM64 has neither compiled nor run; native sherpa/ORT and codec
  build/package support must be implemented before device acceptance.
- **Hardware:** physical microphone/USB/Bluetooth, 30-minute live capture,
  sleep/resume, physical mixed-DPI/HDR and native Qualcomm ARM64 need real devices.
  A four-vCPU/eight-GiB Windows guest and prerecorded WAVs cannot prove them.
- **Browsers:** Edge is the authorized browser gate. Chrome and Firefox remain
  not tested. No Chrome terms were accepted or browser installed for these tests.

Ask was shelved by main and is not restored as a parity task. No GPU/NPU claim,
benchmark cohort, account purchase or public release is part of this work.

## API references

[Per-user install locations](https://learn.microsoft.com/en-us/windows/win32/msi/single-package-authoring),
[MessageBox ownership/default buttons](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-messageboxw),
[virtualized UIA items](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-workingwithvirtualizeditems).

[The acceptance receipt](../CrossPlatform/evidence/windows-milestone6/gui-1520556/README.md)
distinguishes exact executables, unchanged-component rechecks, guest execution,
host compile checks and all open gates. A draft PR is not a completed M6 release gate.

[Review acceptance](../CrossPlatform/evidence/windows-milestone6/review-935e21f/README.md)
records the subsequent dictation, capture-retention, private-browser and clipboard
corrections, exact executables and targeted guest checks.
