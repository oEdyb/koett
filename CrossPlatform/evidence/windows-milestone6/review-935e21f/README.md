Latest CI-race and empty-result cleanup correction: [parallel acceptance](parallel-and-empty/README.md).

# Windows review acceptance

Review base: PR #159 at `935e21f`. The supply-chain correction is local commit
`c6c770d`. Pre-CI integration GUI SHA-256:
`ab0a275ccbd5b77e723704c606b70b84b960397036826b20f09d82307099166f`.
`final-build-receipt.json` records every Rust source and executable hash. Prior
GUI `27e7976` covers Quit, injected ASR recovery and preview management; `69253adf`
covers final privacy checks and drawing delivery. Their exact receipts remain
separate. Those Windows component sources are unchanged in the current build;
the last change preserves audio from standalone capture on a close warning.
No signed release or ARM64 execution is implied.

## Review results

1. **High: parked drawing cancellation.** Cancel releases a parked result's busy
   state and capture latch. An empty result with no frames returns Ready without
   replacing the previous transcript. Native WindowsApp regressions exercise both
   branches and confirm the repeat/quit guards reopen. These are deterministic
   state fixtures executing the actual app methods in Windows, not a claim that
   a physical display-driver stall was reproduced.
2. **High: Quit during dictation.** The shared native Quit guard covers recording,
   transcribing, a queued capture request, parked results and unsaved staged delivery.
   CLI Quit refuses before opening a modal. Native Quit shows a warning; Windows
   session shutdown is vetoed. `quit-voice.json` records real recording refusal,
   visible warning and shutdown veto, followed by one offline Notepad paste with
   stable foreground HWND/PID and no duplicate History on explicit repeat. The
   command immediately after Stop was also refused, but its adjacent status
   snapshot still said Recording; the precise ASR window is covered by native
   state tests rather than misreported as a timed GUI reproduction.
3. **High: slow capture exit.** Proxy samples live outside the collector's return
   value. A bounded join timeout returns all collected audio with a warning for
   normal ASR/recovery, rather than dropping the recording. Standalone Windows
   capture also returns that valid audio and prints the warning. Native tests
   actually block the collector past its deadline and compare retained samples;
   the standalone case compares all 16,000 samples. Existing hung-helper and
   subsequent healthy-open tests also run. This does not prove physical-device
   buffering, USB unplug or Bluetooth behavior.
4. **Medium: text-only drawing delivery.** Text delivery determines success;
   missing image consumption is a separate visible Done message, with Copy and
   no misleading Paste action. `text-only-finish-corrected.json` records a real
   Shift/mouse stroke, contact sheet and one transcript in an external WinForms
   text box. It stays unchanged, the UIA action list contains only Copy, and Copy
   preserves the PNG/HTML/text bundle. Foreground HWND/PID stays the same.
5. **Medium: capture errors leave the app busy.** The Windows runtime publishes
   Ready after Error, while the pill keeps its visible failure. A native channel
   test checks this sequence. The injected ASR fault returned Ready and left media
   and model setup available. Reloading the same cached model allowed recording
   and cancellation again (`model-recover.json`). The initial fixture's immediate
   restart assertion failed because that fixture deliberately terminates its ASR
   worker; the failed receipt is retained, and the successful recovery explicitly
   reloads it. No physical microphone-disconnect test is claimed.
6. **Medium: private browser context.** Brave, Vivaldi, Opera and Chromium/Firefox
   variants receive browser checks. Native and accessible frame titles are checked
   before text gathering; absent/changing accessible frames omit page context.
   Native tests cover aliases/private titles and frame confirmation. Real Edge
   returned the public fixture URL and omitted InPrivate's URL without changing
   focus (`public-source-uncovered.json`, `private-source.json`). Other browser
   binaries were not installed or run.
7. **Medium: cloud clipboard.** Both immediate and delayed offers publish a zero
   DWORD for `CanUploadToCloudClipboard`. Native clipboard round trips read back
   zero and assert no `CanIncludeInClipboardHistory` format was added. Thus local
   Win+V policy is unchanged. No cloud account or second-device sync test ran.
   [Microsoft's format contract](https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-formats#cloud-clipboard-and-clipboard-history-formats)
   defines these independent flags.
8. **Low items.** Ordinary pin status noise is omitted from pasted text; meaningful
   privacy/unavailable notices remain. Selected text was already on by default,
   matching Mac/Linux; documentation now says so. Copy Last leaves the current
   pill's media identity intact, so later pill Copy selects the current result.
   This small routing change was inspected; a new media-result Copy GUI scenario
   was not run. Every README manager command uses process-scoped execution-policy
   bypass. Preview upgrades retain current and rollback versions only, deleting
   older versions only after payload verification. `manager.json` records three
   installs, two retained versions, rollback both ways, data-preserving uninstall
   and unchanged settings/History/Last Transcript hashes. Changed or busy old
   versions are preserved with a warning.

## Validation and harness limits

`audio-units.json` records 40 passing targeted native tests on the final build.
`audio-shutdown.json` records restored configuration, startup off, no app/Narrator
and shutdown; host status confirmed the VM off. Windows
strict Clippy and native binaries/test linking passed. Linux audio/runtime checks
passed 9/5 tests; the final Windows-only standalone branch was followed by another
9-test Linux audio check. Python 3.10 ran the eight dependency-gate tests, inventory
check and actual archive verifier. Six packaging-policy tests passed on the host.
TOML loading is now limited to archive creation; streaming SHA-256 works on 3.10.
The older dependency fixture now includes the required manifest and PE machine.
No policy exception, dependency change or regenerated stale inventory was needed.

Failed harness receipts remain. A multiline WinForms text box exposes Document,
not Edit; TextPattern reads it correctly. The first read still proved one original
paste before the corrected full UI check. An inactive Edge window did not expose
its editor; activating it fixed that query. Koett's own test windows then covered
its center point, so source reading correctly omitted it; closing those windows
allowed the public-source check. No unrelated product behavior was changed to
force a green receipt. Screenshots were extracted unchanged, not resized.

Only Windows-conditional behavior/test fields were added to shared `audio.rs`
and `runtime.rs`; no Swift or Linux application behavior changed. The guest stays
four vCPU/eight GiB at nice 19, with host builds at four jobs/nice 10 and no memory
cap. Guest input only; original configuration and startup-off state are restored.
Full signed M6 release gates remain in `docs/windows-release-gates.md`, including
native ARM64 build support, tool redistribution and physical hardware. Chrome and
Firefox remain untested. No benchmark, signing spend or public release occurred.
