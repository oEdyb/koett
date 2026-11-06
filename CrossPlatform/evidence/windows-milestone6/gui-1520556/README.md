Latest review corrections and newer exact executables: [review acceptance](../review-935e21f/README.md).

# Windows M6 preview acceptance

Prior integration GUI SHA-256:
`1520556302bec6c3dd43a49e05d8961c7e86f1188535c944a2f8d0477254a2a6`.
`build-receipt.json` maps every Rust source file to this executable and its console
siblings. These are locally linked unsigned debug x64 binaries, not executions of
a signed release or ARM64 build. The existing Windows 11 QEMU/KVM guest used four
vCPU, eight GiB, nice 19. Host Rust builds used four jobs/nice 10 with no memory
limit, with the guest off. No host app/data/pointer/keyboard was controlled.

## Rebase acceptance on current main

After rebasing onto main `173ac7c`, the current local GUI is
`bcbf9c7f01d11050135b3b054933b1632d734199ca66ca09ac931df7fe058df7`.
`rebased-build-receipt.json` maps its exact sources and all console/test binaries.
Main added shared audio pause accounting and recording-limit changes; these are
inherited main changes, not an expanded Windows shared-code diff.

- `rebased-units.json`: 23 native Windows tests passed, including ten audio and
  five runtime tests, plus the eight M6 component tests below.
- `rebased-pause.json`: real GUI start/pause/resume/cancel kept the clipboard
  sentinel and foreground HWND/PID. Cancellation returned to Ready.
- `rebased-offline-voice.json`: with the NIC disconnected, blank provider keys,
  a nonexistent browser-runtime override and no Koett browser modules, cached
  CPU WAV dictation pasted once into Notepad without changing foreground HWND/PID.
  Explicit repeat Paste added the text only on request and did not change History.
- `rebased-shutdown.json`: restored the original configuration hash, startup off,
  no Koett/Narrator process and no profile backup. Host status then confirmed off.
- Rebased Windows strict Clippy and all three native binaries/test executable
  linked. Linux's affected audio/runtime checks passed 9/5 tests, and strict
  Clippy passed. Six packaging and ten workflow-policy tests plus unchanged
  supply-chain inventory validation passed. The actual archive passed hash,
  architecture, notices and system-DLL verification.

These are local unsigned debug builds. The earlier integration checks below
remain evidence for unchanged Windows sources; their executable hashes are not
silently relabeled as executions of the rebased binary. The first request sent
before the boot bridge was ready received no matching response; a read-only
follow-up confirmed the prior executable and no app before the install retry.
No product failure or hidden successful mutation was inferred from that timeout.

## Final executable checks

- `units-1520556.json`: eight targeted native tests passed: three original cue
  tests, recovery button states, multiline keyboard routing, image-only pin
  privacy, independent Settings options and real clipboard newer-copy protection.
- `narrator-1520556.json`: Narrator ran while the actual native text/buttons were
  exposed. Tab reached Close and Enter closed the window. Disabled Copy/Retry
  and enabled Close were correct. This proves keyboard/provider access, not a
  human listening or full speech-accuracy review.
- `recovery-faults-1520556.json`: denied History plus a blocked recovery directory
  retained one completed raw transcript. The already-open window refreshed live.
  CLI Quit refused; the native warning focused No, reported default ID 7, and
  keyboard Enter retained the app/text. Copy Unsaved preserved foreground HWND/PID.
  Failed Retry kept the text with a short visible error. After faults were
  restored, Retry persisted once; the second retry left the database hash unchanged.
- `manager-cycle-atomic.json`: per-user install/upgrade/rollback launched both the
  prior accepted GUI (`1010c5b`) and final GUI from a Unicode installation path.
  The previous executable is a lab fixture in the current package layout, not a
  previously shipped installer. Startup paths and the Start shortcut followed
  the selected version. Running-app, checksum and unknown-file guards refused
  changes. Settings (including dictionary/replacements), History and Last
  Transcript hashes stayed identical. The caller was not an administrator.
- `startup-after-reboot.json`: with the virtual NIC disconnected, the installed
  final GUI started from its quoted Unicode Run entry after a genuine guest OS
  reboot and became Ready. Foreground HWND/PID and the executable hash are recorded.
- `fresh-profile-start.json` and `installed-offline-voice.json`: the original
  Roaming Koett data was moved aside, preserving a preseeded local model cache
  under the same Windows account. Empty Copy/Paste/recovery commands refused
  without clearing a sentinel. Cached CPU WAV dictation pasted once into Notepad
  with the same foreground HWND/PID; explicit repeat paste changed no History.
  Koett loaded no browser modules and had a nonexistent WebView2 runtime override.
  Windows Search/Widgets WebView2 processes remained; no runtime was uninstalled.
- `fresh-profile-finish-final.json`: native History contained exactly one fresh
  record. The new fixture data was preserved separately and every original data
  hash was restored. This is fresh application data, not a clean Windows account/OS.
- `manager-final-checks.json`: reserved-device paths and a versions-directory
  junction were refused. All 1,426 files beyond the test junction survived.
  Uninstall removed only the preview, its shortcut and own startup entry while
  preserving data hashes. Another fresh install left startup off; its uninstall
  again preserved all hashes. The original startup entry was absent and restored
  to that state.

## Unchanged-component checks

`pre-keyboard-build-receipt.json` (GUI `0a8032a`) covers the exact unchanged cue,
pin reader, Settings-option and clipboard sources. That GUI passed:

- Real QEMU output during capture start/stop: peak 10,803 and RMS 1,607.98 in the
  recorded segment; with sounds off, no output bytes/nonzero samples appeared.
  This proves guest output and the switch, not physical speakers or microphones.
- Native option toggles plus later Settings Save did not revert them. A physical
  guest Shift/click pin with page details off copied PNG plus text explicitly
  labeled image-only, omitting the fixture title and selected secret.

`scaling-build-receipt.json` (GUI `124d16a`) and
`recovery-scales-refreshed-124d16a.json` cover all six light/dark combinations at
actual 100/150/200% Windows scale. The window remained 600×420 logical pixels and
buttons 32 logical pixels high. `recovery/layout.rs` is unchanged in the final
build. Later recovery edits addressed warning/keyboard behavior, not geometry.
The accepted final images were inspected. No screenshots were resized or retouched.

## Faults found and corrected

The empty view originally disabled Close instead of Retry. Live pending records
could leave a stale empty snapshot. DPI changes required the same monitor
reconciliation as M5. Full storage errors exceeded the status row; they now keep
short actionable text. Native Quit now owns its visible Koett window and focuses
No. The multiline edit returned `DLGC_WANTALLKEYS` for Tab/Enter/Escape; a narrow
native subclass now leaves those keys to the dialog manager and preserves normal
text selection/Copy. Actual Narrator keyboard access passed after that fix.

Windows PowerShell 5.1 exposed three real installer faults: hidden AppData needs
`Get-Item -Force`, BOM-less JSON needs explicit UTF-8, and `File.Replace` needs
`NullString.Value` rather than PowerShell's empty-string coercion. The interrupted
initial activation retained its verified version and null current pointer; the
fixed manager resumed it without changing user data.

Harness corrections are explicit. The wrapper originally lost a nested script's
exit code; it now propagates it and requires JSON `ok`. The file-hash fixture's
array expressions were corrected and now require three actual stores. Caption
Close and button Close are distinguished by control ID. Windows Display Settings
cached its old scale list after a resolution change; reopening Settings plus its
ItemContainer/VirtualizedItem patterns exposed 200%. Failed receipts/keyboard
queries remain alongside the passing checks. One historical failed Narrator
screenshot was 1,553,391 bytes; its original remains in the local cache and its
hash/size are committed instead. Final screenshots are all committed unchanged.

## Scope and open gates

The final archive verifier passed payload hashes, canonical original notices,
actual x64 PE architecture and system-only DLL imports. Six packaging-policy tests,
strict Windows Clippy, native linking and formatting passed locally. No benchmark
cohort ran. No Swift/Linux app source changed; Windows reuses the existing CPAL
cue module. New shared workflow changes affect Windows packaging steps only.
Independent review and the final CI release artifact remain separate checks.

This is preview hardening, not completion of the signed M6 release gate. See
[remaining gates](../../../../docs/windows-release-gates.md): signing/SmartScreen,
MSI/MSIX/automatic updates, tool and optional formatter redistribution, native
ARM64, bare-modifier bindings, focused Windows privacy onboarding and physical
mic/display/sleep/long-capture acceptance. Chrome and Firefox are not tested.
No licence purchase, signing account or public release was performed.

`final-shutdown.json` records restored original settings, startup off, original
1920×1080 display mode, no app/Narrator process and no profile backup left in place.
The host subsequently confirmed the VM was off. Fresh test data remains private
in the guest lab directory; no models, audio, executables or databases are committed.
