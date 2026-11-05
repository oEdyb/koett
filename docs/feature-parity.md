# Feature parity

Source audit: 2026-11-04, `origin/main` at `3d7674e`. **Yes means implemented, not newly runtime-tested.** Windows has never been run on real Windows in the evidence available to this audit; every Windows row requires the [Windows plan's acceptance gates](windows-port-plan.md). “Engine” means shared Rust code exists but the Windows UI/action is missing. No builds or labs were run for this document. Public release availability is separate from branch implementation.

## Dictation and recovery

| Feature | Mac | Linux | Windows | Gap closure / default |
|---|---|---|---|---|
| Local offline ASR, loaded once | Yes, FluidAudio | Yes, sherpa/ORT | Engine + tray shell | Windows CPU/WASAPI live acceptance first; retain offline core |
| Toggle / hold / configurable shortcuts | Yes, modifier or chord | Yes, desktop-dependent | Chord registration; hold parity unverified | Windows Raw Input for bare modifier/hold; Linux portal/compositor bindings |
| Selected microphone | Yes | Yes | Engine; no full Settings UI | Native Windows input selector, stable endpoint identity |
| Level preview in Settings | Native Microphone window; releases on close/record | Yes | No | Add Mac and Windows preview that releases capture on close/record |
| Recording sounds / actual RMS | Yes | Yes | Basic RMS pill; sounds unverified | Match Mac cues and waveform; no invented activity |
| Pill rolling strip, timer, sketch bands | Yes | Layer-shell + optional GNOME Shell pill; X11 fallback | Basic GDI meter/timer only | Mac 300×48 reference, shared state/fixtures, Windows D2D/Composition |
| Pill hover Cancel, Pause, History | Yes; all models drop paused audio, preserving captured-audio clock | Yes | No | Wire Windows engine commands; Mac Nemotron pause via segmented capture/restart with preserved audio clock |
| Pill Done, Not pasted, Copy/Paste | Yes | Yes | No equivalent state/recovery actions | Port persistent rescue states; never time out unsaved text |
| Silence trimming / no silence warnings | Yes; Nemotron skips trim | Empty dictation finishes quietly; silent drawings retain image delivery | Engine; runtime unverified | Linux [silence checks](linux-empty-dictation-verification.md); real microphone/model matrix remains open; add Nemotron trim only if streaming semantics allow |
| Escape cancel | Yes, recording-only | X11 yes; Wayland recording-only with keyboard read access | No recording Esc path identified | Windows recording-scoped observer; Wayland read-only keyboard observer; [checks and permission fallback](linux-cancel-errors-verification.md), pill/CLI without access |
| Paste into original target | Original PID/launch identity + available AX window checked per paste; no atomic receipt | niri/KWin/X11 guard; optional GNOME Shell identity; other Wayland desktops copy-only | HWND equality guard in `deliver`; receipt absent | Linux [KWin guard and limits](linux-kwin-paste-verification.md), [GNOME identity checks](linux-gnome-focus-verification.md); Windows PID/liveness + per-stage recheck; keep copy fallback everywhere |
| Terminal versus GUI paste chord | Configurable per-app image delivery | Per-app image rules with niri/KWin and optional GNOME identity; separate text chord | Ctrl+V baseline | Per-app chord rules on Linux/Windows; never guess by sending both |
| Last transcript / Copy and Paste Last | Yes | Yes; CLI uses guarded controller delivery | Shared persistence; no matching tray actions | Native actions on Windows; Linux [CLI checks and limits](linux-last-paste-verification.md), target consumption remains unverified |
| Failed audio / failed text fallback | Yes | Yes | Shared engine/store paths; untested | Windows visible recovery + failure injection, preserve ASR/delivery separation |
| Retry Save / recovery status | Retry Save + Copy Unsaved; stable recovery IDs | Yes | No full recovery UI | Add Retry Save/status on Mac and Windows; stable IDs prevent duplicate saves |
| Crash during active capture | No automatic recovery | No, audio in memory | Unverified; no durable capture journal found | Optional bounded crash-safe local recording journal, explicit privacy/retention; never claim current recovery covers kills |
| Long live dictation | Synthetic long-ASR probes, not full hardware proof | Not newly verified | Not verified | 30-minute live sessions; check memory, dictionary/progress on long-path chunking |
| Permission onboarding | Live-status first-run window | First visible launch + manual Permissions window; live keyboard/shortcuts; explicit mic/screen checks | No focused onboarding | Linux [first-launch checks and limits](linux-first-run-permissions-verification.md); real grant/revocation acceptance remains open; Windows focused status surface |
| Login toggle / tray or menu bar | Native toggle; explicit disabled preference survives reinstall | StatusNotifier tray; GNOME needs indicator provider; app/CLI alternative | Run-key toggle + tray source | Add Mac toggle; Windows verify Explorer restart and user-disabled startup |

Evidence: `Sources/Koett/{DictationSession,DictationDelivery,RecordingOverlay,EscapeKeyMonitor,MicrophoneSelection,PermissionsWindow}.swift`; `CrossPlatform/src/{windows,runtime,audio,state,history}.rs`, `linux/{recording_overlay,input_preview,output,recovery,control}.rs`; [demo limits](video-demo-checklist.md).

## Models and words

| Feature | Mac | Linux | Windows | Gap closure / default |
|---|---|---|---|---|
| Parakeet v2 | Yes | Yes | Engine | Keep choices/defaults; native picker + download status on Windows |
| Parakeet v3 / Swedish in app | Yes, optional v3; accuracy acceptance recorded separately | Yes | Engine, no picker | Add optional Mac v3 using existing supported engine integration; Windows picker; no default switch |
| Parakeet 110M primary ASR | Yes, optional native 110M TDT; synthetic phrase tested, human corpus pending | Yes, default | Engine | Optional Mac 110M primary path; validate accuracy, not just load success |
| Nemotron streaming / Parakeet recovery | Yes, experimental | Yes | Shared engine; Windows unverified | Same experimental label, capture/finalization failure tests; no delivery-triggered retranscription |
| Download verification / progress / offline reuse | Yes | Yes | Engine + tray progress | Native Windows progress/cancel/retry, checksums and atomic activation |
| CPU / GPU / ARM64 | Apple Silicon local runtime | CPU path | CPU candidate, accelerators unverified | Native Windows x64/ARM64 CPU first; WinML/CUDA/QNN only after graph/device benchmarks |
| Dictionary editor, import/export | Yes | Yes | Shared parsing/storage; no UI | Windows native Words page; same exchange format |
| Acoustic dictionary bias + adoption | Cheap spelling/forms by default; v2 CTC sidecar experimental and default-off; Nemotron spelling only | v2/v3 shared-encoder hotwords on verified builds (v2 measured; v3 performance unmeasured); 110M and Nemotron spelling only | Same engine limitation as Linux | Bounded CTC rescorer/optional transducer sidecar for 110M; investigate Nemotron bias; spelling-only must stay labeled |
| Spelling around S1-mini | Before and after | Before and after, with raw recovery preserved | Shared Rust order | Align Rust pre/post passes to Mac using fixture/corpus checks |
| Literal replacements | Yes | Yes | Engine; no UI | Expose Windows editor; replacements remain final stage |
| Fix a word / accepted learning suggestions | Yes | Yes | Shared correction modules; no UI | Windows history action, explicit Add/Dismiss, everyday-word warning |
| Optional S1-mini formatting controls | Yes, raw fallback | Yes, raw fallback | Engine, no full controls | Native Windows page; fail back to raw, measure Windows runtime dependencies |

Evidence: `Sources/Koett/Dictionary/`, `S1MiniCleaner.swift`, `Replacements.swift`; `CrossPlatform/src/{model,formatting,runtime}.rs`, `transcription/{hotwords,nemotron,recovery}.rs`, `dictionary/`, `corrections/`; [dictionary details and benchmarks](dictionary.md). A model present in shared Rust is not proof its Windows artifact is complete.

## Drawing, pins and clipboard

| Feature | Mac | Linux | Windows | Gap closure / default |
|---|---|---|---|---|
| Drawing, hold key, pass-through otherwise | Yes | layer-shell; KWin evidence, not GNOME/X11 | No | Windows per-display HWNDs; Linux GNOME Shell integration and X11 overlay backend |
| Draw lock (toggle key, one sketch across lifts) | No; Linux first | Yes on layer-shell desktops, with keyboards readable or the focus opt-in. Lab-checked on niri and KWin | No | Port to Mac and Windows after Olle tries it on Linux |
| Bare Shift without interrupting capitals/chords | Yes, gesture guards | Opt-in evdev/keyboard setup; click protection starts at the first clean sample, ink still waits 120 ms; real 40–100 ms checks pass; typing now discards provisional ink ([regression](linux-capital-typing-verification.md)); pre-sample/commit gap remains ([receipt](linux-early-shift-verification.md)) | No | Windows recording-scoped state observer + Mac fixture rules; Linux retain explicit permission explanation |
| Undo ink | Recording-scoped Undo Last Drawing; removes pending/captured last frame | Recording-scoped Undo Last Drawing; pending/captured frames; Ctrl+Z still edits current ink in focus mode | No | Windows scoped undo; Linux [checks and limits](linux-drawing-undo-verification.md); no global editor-undo interception |
| Multi-monitor / mixed DPI / hotplug | Per-screen capture; hardware parity not rechecked | Live layer-shell overlay/pill refresh on monitor add/remove, geometry and scale changes; completed images retained; unfinished captures invalidated visibly ([receipt](linux-monitor-hotplug-verification.md)); physical mixed-DPI/active niri-pick acceptance open | No | Real dual-monitor acceptance and add/remove during capture on all |
| Word times, sketch linking, numbered contact sheet | Yes | Yes; some model times estimated | Shared data algorithms, no capture/UI | Windows compose intended ink onto captured background; keep timestamps and model approximation labels |
| Shift-click pins + app/title | Yes | niri + KWin context; other AT-SPI providers need usable geometry | No | Windows UIA + HWND identity; Linux compositor adapters rather than guessed labels |
| Browser URL / selection | Yes; no private-mode detection | Chrome setup required; provider-dependent AT-SPI selection | No | UIA first on Windows; optional browser native-messaging bridge on all for unavailable context |
| Link/image under pointer, tiled windows | Yes via AX | KWin + stock niri floating; optional pinned niri adapter verifies tiled hits; stock tiled still omits hit ([receipt](linux-niri-geometry-verification.md)) | No | Windows ElementFromPoint; Linux compositor geometry adapter or browser integration |
| YouTube timestamp link | Yes, Chrome AX player slider | AT-SPI player range on matched page; [checks/limits](linux-pin-context-verification.md) | No | Linux AT-SPI player range when matched to URL, else browser bridge; Windows UIA range + URL match, else bridge |
| Password avoidance / bounded opt-in selection | Yes | Yes, provider-dependent | No context reader | Reject password/protected UI and stale cross-window context; no synthetic Copy |
| Private/incognito URL exclusion | Automatic detection absent; explicit image-only pins omit all page details | Not established | No implementation | Explicit browser private-mode signal where available; absence must not imply privacy detection |
| First paste image then bullets | Yes, per-app chords | Per-app niri/KWin/optional GNOME image chord; portal/clipboard backend limits | No | Windows staged clipboard with foreground checks each step, persistent rescue if either stage fails |
| Repeat paste PNG + HTML + text bundle | PNG + embedded-image HTML + exact text; real target consumption under verification | Multi-format owner now on main (#49); real target matrix still needed | Text-only baseline | Mac verify/repair same bundle for Done/last/Paste; Windows publish PNG+DIBV5+HTML+Unicode together |
| Preserve newer clipboard copy | Image-to-text and bundle ownership guards; isolated race tests pass | Owner markers/race detection, bounded transfers | No staged bundle path | Port ownership contract on Windows; race fixture plus real clipboard-reader checks |
| Image privacy / lifetime | Memory only | Memory except temporary portal file deleted | Not implemented | Match local ephemeral images; history retains text/raw text, no silent image persistence |
| Click / spoken-cue keyframes | Not established as shipped | Explicitly not implemented | No | Keep as named backlog across all; implement only after exact cue behavior/spec and acceptance are defined |

Evidence: `Sources/Koett/Sketch*.swift`, `DrawKey.swift`, `PinContext*.swift`; `CrossPlatform/src/{annotation,sketch,pin_context}.rs`, `linux/sketch/`, `linux/{feedback,output,clipboard_owner}.rs`; [drawing](draw-while-dictating.md), [pin rules](screen-context-proposal.md). Clipboard helper existence does not prove every app consumes all formats. Preserve this gap until tested in browser agent editor, native rich text, plain editor and terminal.

## History, media, Ask and automation

| Feature | Mac | Linux | Windows | Gap closure / default |
|---|---|---|---|---|
| Searchable native History / Copy / Export | Yes | Yes | Opens legacy Markdown externally, not native history browser | Windows native list/detail backed by existing shared SQLite/FTS APIs |
| Schema / legacy import / raw text | Yes | Yes, same canonical schema | Shared store | Windows migration/duplicate-ID/failed-save tests on real filesystem |
| Unified Settings | Menus + focused native popovers/windows | Yes | Shortcut dialog + small tray menu | Windows grouped native pages; preserve Mac's approved interactions |
| Focused browser media transcription | Yes | Yes plus focused document / typed URL | No; module Linux-gated | Share fetch/ASR/store; native Windows URL dialog + UIA browser URL |
| File/document media entry | Native file picker + typed URL + focused browser + CLI | Yes | No product UI | Add Mac file/URL entry and Windows parity, no browser-cookie import |
| Pause media during dictation | Opt-in accessible Pause controls; unsupported players untouched, manual resume | MPRIS yes | No | Windows GSMTC explicit pause; Mac browser/player-specific explicit Pause commands or opt-in integration |
| Open media transcripts | Yes | Yes | No action | Windows open folder/history action, shared filename/collision rules |
| Vimeo anonymous fetch | Known 401 | Not reverified | No workflow | Track upstream extractor/auth restriction; user-supplied local file is alternative, never quietly import browser cookies |
| `koett-transcribe` URL/file CLI | Yes | Yes, source installer; v2/v3 URL/file, JSON/text/cache | No equivalent product CLI | Linux uses existing wrapper + small Rust adapter; [fresh evidence](linux-cli-verification.md). Windows installer/entry remains open |
| Control CLI / agent JSON | Local user-only socket + JSON status/capture/recovery/media commands | Yes | Engine harness, no equivalent app control | Add Mac/Windows local command endpoint + documented status/errors; avoid network listener |
| Ask text + one screenshot, providers/models/keys | Yes | Yes | No; assistant module Linux-gated | Share HTTP/SSE/session logic; Windows WGC + native panel/settings + Credential Manager |
| Ask spoken replies / interrupt | Yes, Cartesia | Yes | No | Windows audio output adapter, stop on interruption, keys optional |
| Ask Markdown/math/local renderer | Yes, installed-panel live check outstanding | Yes, not newly verified | No | Optional lazy WebView2 renderer on Windows; no browser process in dictation |
| Dark/light, high contrast, scaling, reduced motion | Native surfaces; not all combinations rechecked | Native evidence; live theme changes partly unverified | Basic surfaces only | Narrator/keyboard/theme/contrast + narrow/long-text gates per platform |
| Signed install / updates / architectures | Public notarized Apple Silicon release; later source differs | Native scripts/artifacts; clean-install coverage incomplete | Compile/CI archive only, no real install proof | Windows signed x64+ARM64 installer, clean user/update tests; Linux package gates; Mac release remains separately authorized |

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
