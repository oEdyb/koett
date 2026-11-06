# Feature parity

Baseline source audit: 2026-11-04, origin/main at 3d7674e. Linux rows were corrected on
2026-11-05 after a second audit traced about 30 of them to code; the other Linux rows were
not re-traced. Windows entries now
also include accepted M1–M3 guest evidence and M4's targeted Windows 11 checks.
**Yes means implemented; it does not imply every hardware/app combination was
runtime-tested.** Current Windows evidence and limits are in
[draw and pin](windows-draw-pin.md). Public release availability remains separate
from implementation; later parity/distribution gates are still open.

## Dictation and recovery

| Feature | Mac | Linux | Windows | Gap closure / default |
|---|---|---|---|---|
| Local offline ASR, loaded once | Yes, FluidAudio | Yes, sherpa/ORT | Guest CPU WAV engine verified (M2/M3); physical-device/long-session gates open | Windows CPU/WASAPI live acceptance first; retain offline core |
| Toggle / hold / configurable shortcuts | Yes, modifier or chord | Yes, desktop-dependent | Guest toggle/hold and chord registration verified; bare-modifier binding remains a parity gap | Windows Raw Input for bare modifier/hold; Linux portal/compositor bindings |
| Recording limit in Settings, default 60 min, 1-minute warning | Yes; 10/30/60/120 menu; 662s guest capture passed, real 60min expiry open | Yes | Native 120/60-minute Save and collector bounds checked; live warning/long-session gate open | Run on Windows |
| Selected microphone | Yes | Sound server sources, shared with other apps; falls back when lost, stalled or silent. ALSA cards with no server | Native ID selector; emulated endpoint persistence/default/unavailable checks passed | Verify Windows endpoint selection/save/reopen and unavailable-device behavior |
| Level preview in Settings | Native Microphone window; releases on close/record | Yes | Guest virtual preview start/stop plus bounded opening/enumeration/preview faults passed; physical signal open | Verify Windows close/record release and actual input levels |
| Recording sounds / actual RMS | Tink and Basso system sounds; always on | Original synthesized cues at the same moments, with a Settings switch (on by default); real RMS | Basic RMS pill; sounds unverified | Match Mac cues and waveform; no invented activity |
| Pill rolling strip, timer, sketch bands | Yes | Layer-shell, draggable. Optional GNOME Shell pill, draggable, which [returns after the extension turns back on](linux-gnome-pill-verification.md#screen-lock). X11 override-redirect pill, draggable | Native D2D/Composition 300×48 pill; M1/M4 guest states, physical Stop and Reduce Motion checked | Mac 300×48 reference, shared state/fixtures, Windows D2D/Composition |
| Pill hover Cancel, Pause, History | Yes; all models drop paused audio, preserving captured-audio clock | Yes | Guest pointer/UIA actions and pause/cancel verified; actions exposed without hover | Wire Windows engine commands; Mac Nemotron pause via segmented capture/restart with preserved audio clock |
| Pill Done, Not pasted, Copy/Paste | Yes | Yes | Persistent rescue, tray/pill recovery and drawing bundle repeat guest-verified | Port persistent rescue states; never time out unsaved text |
| Silence trimming / no silence warnings | Yes; Nemotron skips trim | Empty dictation finishes quietly; silent drawings retain image delivery | Guest silent WAV preserves clipboard/history; silent drawings retain image delivery | Linux [silence checks](linux-empty-dictation-verification.md); real microphone/model matrix remains open; add Nemotron trim only if streaming semantics allow |
| Escape cancel | Yes, recording-only | X11 yes; Wayland recording-only with keyboard read access | Recording-scoped cancel and draw-lock unlock implemented; guest checks passed | Windows recording-scoped observer; Wayland read-only keyboard observer; [checks and permission fallback](linux-cancel-errors-verification.md), pill/CLI without access |
| Paste into original target | Original PID/launch identity + available AX window checked per paste; no atomic receipt | niri/KWin/X11 guard; optional GNOME Shell identity; other Wayland desktops copy-only | HWND/PID/integrity/clipboard checks before every stage; guest editor/browser/terminal and recovery checks; no universal receipt | Linux [KWin guard and limits](linux-kwin-paste-verification.md), [GNOME identity checks](linux-gnome-focus-verification.md); Windows PID/liveness + per-stage recheck; keep copy fallback everywhere |
| Terminal versus GUI paste chord | Configurable per-app image delivery | Per-app image rules with niri/KWin and optional GNOME identity; separate text chord | Native text/default-image/per-app/inherited settings guest-verified; never sends both chords | Per-app chord rules on Linux/Windows; never guess by sending both |
| Last transcript / Copy and Paste Last | Yes | Yes; CLI uses guarded controller delivery | Guest native tray/pill actions, retained original destination across cancel, no repeat history write | Native actions on Windows; Linux [CLI checks and limits](linux-last-paste-verification.md), target consumption remains unverified |
| Failed audio / failed text fallback | Yes | Yes | Accepted M2 injected ASR/storage/recovery guest gates; physical failure matrix remains open | Windows visible recovery + failure injection, preserve ASR/delivery separation |
| Retry Save / recovery status | Retry Save + Copy Unsaved; stable recovery IDs | Yes | No full recovery UI | Add Retry Save/status on Mac and Windows; stable IDs prevent duplicate saves |
| Crash during active capture | No automatic recovery | No, audio in memory | Unverified; no durable capture journal found | Optional bounded crash-safe local recording journal, explicit privacy/retention; never claim current recovery covers kills |
| Long live dictation | Synthetic long-ASR probes, not full hardware proof | Not newly verified | Not verified | 30-minute live sessions; check memory, dictionary/progress on long-path chunking |
| Permission onboarding | Live-status first-run window | First visible launch + manual Permissions window; live keyboard/shortcuts; explicit mic/screen checks | No focused onboarding | Linux [first-launch checks and limits](linux-first-run-permissions-verification.md); real grant/revocation acceptance remains open; Windows focused status surface |
| Login toggle / tray or menu bar | Native toggle; explicit disabled preference survives reinstall | StatusNotifier tray; GNOME extension adds a top bar menu when no tray provider runs; app/CLI alternative | Run-key toggle + tray source | Add Mac toggle; Windows verify Explorer restart and user-disabled startup |

Evidence: `Sources/Koett/{DictationSession,DictationDelivery,RecordingOverlay,EscapeKeyMonitor,MicrophoneSelection,PermissionsWindow}.swift`; `CrossPlatform/src/{windows,runtime,audio,state,history}.rs`, `linux/{recording_overlay,input_preview,output,recovery,control}.rs`; [demo limits](video-demo-checklist.md).

## Models and words

| Feature | Mac | Linux | Windows | Gap closure / default |
|---|---|---|---|---|
| Parakeet v2 | Yes | Yes | Native picker; guest CPU WAV, corrected assertion and restart persistence passed | Keep choices/defaults; native picker + download status on Windows |
| Parakeet v3 / Swedish in app | Yes, optional v3; accuracy acceptance recorded separately | Yes | Native picker; English guest CPU WAV passed, Swedish accuracy not measured | Add optional Mac v3 using existing supported engine integration; Windows picker; no default switch |
| Parakeet 110M primary ASR | Yes, optional native 110M TDT; synthetic phrase tested, human corpus pending | Yes, default | Native picker; guest CPU WAV and switch-back passed | Optional Mac 110M primary path; validate accuracy, not just load success |
| Nemotron streaming / Parakeet recovery | Yes, experimental | Yes | Native picker; guest CPU streaming WAV passed, specific recovery faults untested | Same experimental label, capture/finalization failure tests; no delivery-triggered retranscription |
| Download verification / progress / offline reuse | Yes | Yes | Engine + tray progress | Native Windows progress/cancel/retry, checksums and atomic activation |
| CPU / GPU / ARM64 | Apple Silicon local runtime | CPU path | x64 guest CPU passed; physical x64/ARM64 and accelerators unverified | Native Windows x64/ARM64 CPU first; WinML/CUDA/QNN only after graph/device benchmarks |
| Dictionary editor, import/export | Yes | Yes | Native Words; guest CRUD/import/export/denied-write/rename checks passed | Windows native Words page; same exchange format |
| Acoustic dictionary bias + adoption | Cheap spelling/forms by default; v2 CTC sidecar experimental and default-off; Nemotron spelling only | v2/v3 shared-encoder hotwords on verified builds (v2 measured; v3 performance unmeasured); 110M and Nemotron spelling only | Same engine limitation as Linux | Bounded CTC rescorer/optional transducer sidecar for 110M; investigate Nemotron bias; spelling-only must stay labeled |
| Spelling around S1-mini | Before and after | Before and after, with raw recovery preserved | Shared Rust order | Align Rust pre/post passes to Mac using fixture/corpus checks |
| Literal replacements | Yes | Yes | Native editor and final delivery stage passed real CPU WAV; original ASR and empty-result clipboard preserved | Expose Windows editor; replacements remain final stage |
| Fix a word / accepted learning suggestions | Yes | Yes | Native correction editor; strict UIA Save/Add/Dismiss/everyday/denied-write checks passed | Windows history action, explicit Add/Dismiss, everyday-word warning |
| Optional S1-mini formatting controls | Yes, raw fallback | Yes, raw fallback | Native controls; guest formatting/raw fallback passed with private CPU runtime; runtime is not bundled | Native Windows page; fail back to raw, measure Windows runtime dependencies |

Evidence: `Sources/Koett/Dictionary/`, `S1MiniCleaner.swift`, `Replacements.swift`; `CrossPlatform/src/{model,formatting,runtime}.rs`, `transcription/{hotwords,nemotron,recovery}.rs`, `dictionary/`, `corrections/`; [dictionary details and benchmarks](dictionary.md). A model present in shared Rust is not proof its Windows artifact is complete.

## Drawing, pins and clipboard

| Feature | Mac | Linux | Windows | Gap closure / default |
|---|---|---|---|---|
| Drawing, hold key, pass-through otherwise | Yes | layer-shell; KWin evidence. GNOME through the extension, lab-checked on Shell 51. X11 with a compositor, lab-checked in parts | Guest physical-key canvases/pins and normal cross-process typing/clicks verified; hardware matrix open | Windows per-display HWNDs; Linux GNOME Shell integration and X11 overlay backend |
| Draw lock (toggle key, one sketch across lifts) | Yes; recording-scoped shortcut/menu, multiple strokes, Esc unlock then cancel ([guest checks](macos-draw-lock-verification.md)) | Yes on layer-shell desktops and GNOME, with keyboards readable or the focus opt-in. Lab-checked on niri and KWin | Guest lock, lifts, right-click/Ctrl+Z undo and Escape unlock verified | Port to Mac and Windows after Olle tries it on Linux |
| Bare Shift without interrupting capitals/chords | Yes, gesture guards | Opt-in evdev/keyboard setup; click protection starts at the first clean sample, ink still waits 120 ms; real 40–100 ms checks pass; typing now discards provisional ink ([regression](linux-capital-typing-verification.md)); pre-sample/commit gap remains ([receipt](linux-early-shift-verification.md)) | Recording-scoped Raw Input, seen-up/chord/other-key guards; all 15 gesture fixtures and guest input checks passed | Windows recording-scoped state observer + Mac fixture rules; Linux retain explicit permission explanation |
| Undo ink | Recording-scoped Undo Last Drawing; removes pending/captured last frame | Recording-scoped Undo Last Drawing; pending/captured frames; Ctrl+Z still edits current ink in focus mode | Recording-scoped undo; guest right-click/Ctrl+Z removal and pending/captured frame rules | Windows scoped undo; Linux [checks and limits](linux-drawing-undo-verification.md); no global editor-undo interception |
| Multi-monitor / mixed DPI / hotplug | Per-screen capture; hardware parity not rechecked | Live layer-shell overlay/pill refresh on monitor add/remove, geometry and scale changes; completed images retained; unfinished captures invalidated visibly ([receipt](linux-monitor-hotplug-verification.md)); physical mixed-DPI/active niri-pick acceptance open | Guest 168/96 DPI, negative portrait ink, detach/Extend and frame retention passed; physical cable/HDR open | Real dual-monitor acceptance and add/remove during capture on all |
| Word times, sketch linking, numbered contact sheet | Yes | Yes; some model times estimated | Guest multiple-frame sheets and audio-time annotations passed; model approximation labels retained | Windows compose intended ink onto captured background; keep timestamps and model approximation labels |
| Shift-click pins + app/title | Yes | niri + KWin + GNOME extension + X11 context; other AT-SPI providers need usable geometry | Guest Edge/native editor/terminal context with explicit unavailable fields; own Koett context rejected | Windows UIA + HWND identity; Linux compositor adapters rather than guessed labels |
| Browser URL / selection | Yes; Chrome document URL fallback before web-tree readiness ([checks](macos-browser-pin-document-verification.md)); selection requires usable AX | Chrome setup required; provider-dependent AT-SPI selection | Guest Edge public URL/selection/password and InPrivate omission passed; Chrome/Firefox untested | UIA first on Windows; optional browser native-messaging bridge on all for unavailable context |
| Link/image under pointer, tiled windows | Yes via AX | KWin + stock niri floating; optional pinned niri adapter verifies tiled hits; stock tiled still omits hit ([receipt](linux-niri-geometry-verification.md)) | UIA patterns and HWND geometry implemented; unavailable provider-specific fields stay absent | Windows ElementFromPoint; Linux compositor geometry adapter or browser integration |
| YouTube timestamp link | Yes, Chrome AX player slider | AT-SPI player range on matched page; [checks/limits](linux-pin-context-verification.md) | Actual Edge watch URL with Video time unavailable fallback; real-clock parser fixtures passed | Linux AT-SPI player range when matched to URL, else browser bridge; Windows UIA range + URL match, else bridge |
| Password avoidance / bounded opt-in selection | Yes | Yes, provider-dependent | Guest password omission passed; capped opt-in selection, stale-window checks and no synthetic Copy | Reject password/protected UI and stale cross-window context; no synthetic Copy |
| Private/incognito URL exclusion | English Chrome/Chromium Incognito AX-window marker omits page fields ([checks](macos-private-pin-verification.md)); other browsers/languages use explicit image-only pins | Chrome and Chromium Incognito and English Firefox Private Browsing keep only the app name; other browsers and languages use the Include Page Details switch | Guest Edge InPrivate clears page fields; unknown privacy remains explicit, broader providers untested | Explicit browser private-mode signal where available; absence must not imply privacy detection |
| Include Page Details in Pins switch | Yes, on by default; off gives image-only pins | Yes, on by default; off gives image-only pins | No | Add the Windows switch |
| First paste image then bullets | Yes, per-app chords | Per-app niri/KWin/optional GNOME image chord; portal/clipboard backend limits | Guest read-based staging into original Edge; unsupported image targets expose recovery | Windows staged clipboard with foreground checks each step, persistent rescue if either stage fails |
| Repeat paste PNG + HTML + text bundle | PNG + embedded-image HTML + exact text; Chrome/native/T3 0.1.25 guest consumption verified ([T3 receipt](macos-t3-repeat-paste-verification.md)) | Multi-format owner now on main (#49); real target matrix still needed | Guest PNG/DIBV5/HTML/Unicode, native rich Edge paste and automatic restore passed; no history duplication | Mac verify/repair same bundle for Done/last/Paste; Windows publish PNG+DIBV5+HTML+Unicode together |
| Preserve newer clipboard copy | Image-to-text and bundle ownership guards; isolated race tests pass | Owner markers/race detection, bounded transfers | Guest image-stage and text-stage races passed; owner/sequence/marker guards preserve newer copies | Port ownership contract on Windows; race fixture plus real clipboard-reader checks |
| Image privacy / lifetime | Memory only | Memory except temporary portal file deleted | Memory-only frames/bundles; History retains text/raw speech; guest silence/cancel and recovery checked | Match local ephemeral images; history retains text/raw text, no silent image persistence |
| Click / spoken-cue keyframes | Not established as shipped | Explicitly not implemented | No | Keep as named backlog across all; implement only after exact cue behavior/spec and acceptance are defined |

Windows M4 software gates passed in the guest; current receipts and hardware/provider limits:
[Windows draw and pin](windows-draw-pin.md). Chrome is explicitly untested and
does not block the authorized Edge browser gate.

Evidence: `Sources/Koett/Sketch*.swift`, `DrawKey.swift`, `PinContext*.swift`; `CrossPlatform/src/{annotation,sketch,pin_context}.rs`, `linux/sketch/`, `linux/{feedback,output,clipboard_owner}.rs`; [drawing](draw-while-dictating.md), [pin rules](screen-context-proposal.md). Clipboard helper existence does not prove every app consumes all formats. Preserve this gap until tested in browser agent editor, native rich text, plain editor and terminal.

## History, media and automation

| Feature | Mac | Linux | Windows | Gap closure / default |
|---|---|---|---|---|
| Searchable native History / Copy / Export | Yes | Yes | Native searchable list/detail; full-search/raw Copy/export/cancel/legacy checks passed | Windows native list/detail backed by existing shared SQLite/FTS APIs |
| Schema / legacy import / raw text | Yes | Yes, same canonical schema | Shared store | Windows migration/duplicate-ID/failed-save tests on real filesystem |
| Unified Settings | Menus + focused native popovers/windows | Yes | Native Settings/Words/Model pages and draw/paste/recording-limit controls guest-verified | Windows grouped native pages; preserve Mac's approved interactions |
| Focused browser media transcription | Yes | Yes plus focused document / typed URL | No; module Linux-gated | Share fetch/ASR/store; native Windows URL dialog + UIA browser URL |
| File/document media entry | Native file picker + typed URL + focused browser + CLI | Native file picker + typed URL + focused browser + CLI (`transcribe-file`, `transcribe-url`, `media`) | No product UI | Add Mac file/URL entry and Windows parity, no browser-cookie import |
| Pause media during dictation | Opt-in accessible Pause controls; unsupported players untouched, manual resume | MPRIS yes | No | Windows GSMTC explicit pause; Mac browser/player-specific explicit Pause commands or opt-in integration |
| Open media transcripts | Yes | Yes | No action | Windows open folder/history action, shared filename/collision rules |
| Vimeo anonymous fetch | Known 401 | Not reverified | No workflow | Track upstream extractor/auth restriction; user-supplied local file is alternative, never quietly import browser cookies |
| `koett-transcribe` URL/file CLI | Yes | Yes, source installer; v2/v3 URL/file, JSON/text/cache | No equivalent product CLI | Linux uses existing wrapper + small Rust adapter; [fresh evidence](linux-cli-verification.md). Windows installer/entry remains open |
| Control CLI / agent JSON | Local user-only socket + JSON status/capture/recovery/media commands | Local user-only socket; `status`, `pause`, `resume`, `undo`, `history`, `dictionary`, capture, recovery | Engine harness, no equivalent app control | Add Mac/Windows local command endpoint + documented status/errors; avoid network listener |
| Dark/light, high contrast, scaling, reduced motion | Native surfaces; not all combinations rechecked | Native evidence; live theme changes partly unverified | Basic surfaces only | Narrator/keyboard/theme/contrast + narrow/long-text gates per platform |
| Signed install / updates / architectures | Public notarized Apple Silicon release; source adds [signed idle-only Sparkle updates](macos-updates.md), disabled until a feed is configured | Native scripts/artifacts; clean-install coverage incomplete | Compile/CI archive only, no real install proof | Windows signed x64+ARM64 installer, clean user/update tests; Linux package gates; Mac feed/release activation remains separately authorized |

Evidence: `Sources/Koett/{HistoryWindow,HistoryDatabase,BrowserMedia,MediaSession,AssistantSession,AssistantClient,AssistantMarkdown,CartesiaSpeechOutput}.swift`; `CrossPlatform/src/lib.rs` target gates; `media_transcription.rs`, `assistant/`, `linux/{history_view,settings_view,media,control}.rs`; `CrossPlatform/src/windows.rs::{open_history,deliver}`; [CLI contract](transcribe-cli.md), [UI contract](ui-design.md).

## Hard limits versus missing implementation

No ordinary product feature above is declared impossible across an entire OS. Some **universal guarantees** cannot be delivered under the current security model:

- **Windows standard-user injection into elevated targets:** blocked by [SendInput UIPI](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-sendinput). Copy/manual paste is default; an explicitly approved UIAccess helper has additional restrictions. Secure desktop is outside normal app control ([UIA security](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview)).
- **Windows arbitrary-app paste receipt/atomic foreground guard:** SendInput reports insertion, not target consumption. Reduce races, preserve text and let the user retry; never claim guaranteed delivery.
- **Read arbitrary app selection/URL/password:** accessibility providers choose what to expose; [UIA text patterns](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingtextandtextrange) are not a universal browser API. Passwords must remain unread. Use explicit local context/browser bridge or image-only pin.
- **Wayland universal global Esc, window geometry and overlay:** current Koett lacks these on some compositors, not an OS-wide impossibility. Wayland core has no general global-key/window-position API; use portals, a compositor binding/extension, or explicit Cancel. GNOME has no layer-shell path in this implementation. The exact compositor extension scope needs separate research; do not advertise blanket Wayland parity.
- **Mac universal pause through private MediaRemote:** project evidence records entitlement restrictions; this is not proof all pause approaches are impossible. Keep player/browser-specific explicit Pause adapters on the backlog; never replace pause with a toggle that might start stopped media.
- **Protected capture:** [Windows capture protection](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity) can exclude content. Blank/denied capture must be reported; no bypass. Other platforms' protected-content behavior needs its own verification.

“All features everywhere” means closing each row or shipping a clearly named closest alternative, not calling an absent feature impossible. No rows may be silently dropped. Acceptance and unresolved purchasing/security choices live in [Windows port plan](windows-port-plan.md).

## Linux follow-up order

[Row-4 checks and limits](linux-paste-permissions-verification.md): per-app image
rules and the permissions surface. Row 5 merged in PR #65.
Row 6 includes [guarded CLI Paste Last](linux-last-paste-verification.md),
[first-visible-launch Permissions](linux-first-run-permissions-verification.md)
and [recording-scoped drawing undo](linux-drawing-undo-verification.md).
The remaining work is tracked in the [Linux open-row queue](linux-open-rows.md);
real permission grants, non-niri targets, silence, long capture, monitor hotplug,
provider/clipboard matrices and packaging remain open.
After row 6, revisit these explicitly open integrations:

- **Tiled niri pointer context:** an optional pinned compositor patch now
  supplies verified pick-time geometry. Native tiled link/image/scroll checks
  pass; stock IPC remains unchanged. Adapter deployment, normal-session
  packaging and physical mixed-DPI acceptance remain open. Retain image/window
  context when exact link/image coordinates cannot be proved.
- **GNOME drawing overlay:** implement a GNOME Shell-compatible overlay/input
  integration; layer-shell alone does not provide it. Verify pass-through,
  recording-scoped gestures, capture exclusion, cancellation and accessibility.

Linux CTC dictionary bias: [real audio benchmark, setup and deadline limits](ctc-dictionary-benchmark.md).
The optional sidecar requires local verified v2 files and can fall back on long
chunks; Nemotron acoustic bias is still open. Remaining row-6 work stays in the
open queue. Optional niri adapter deployment and GNOME overlay integration
remain open.

Every currently open Linux row and its reason is enumerated in the
[Linux parity audit](linux-parity-open-audit.md). GNOME focus identity is a
separate verified component; GNOME drawing/capture/pointer context remain open.

Optional GNOME recording pill: [native fullscreen/remap/cancel/focus checks and
remaining limits](linux-gnome-pill-verification.md). GNOME drawing, capture and
pointer context remain open; a recording pill does not supply those backends.

Mac source closures, closest alternatives and remaining physical/provider gates: [Mac open rows](macos-parity-open-rows.md).
