# Feature parity

Source audit: 2026-11-04, `origin/main` at `3d7674e`. **Yes means implemented, not newly runtime-tested.** Windows has never been run on real Windows in the evidence available to this audit; every Windows row requires the [Windows plan's acceptance gates](windows-port-plan.md). “Engine” means shared Rust code exists but the Windows UI/action is missing. No builds or labs were run for this document. Public release availability is separate from branch implementation.

## Dictation and recovery

| Feature | Mac | Linux | Windows | Gap closure / default |
|---|---|---|---|---|
| Local offline ASR, loaded once | Yes, FluidAudio | Yes, sherpa/ORT | Engine + tray shell | Windows CPU/WASAPI live acceptance first; retain offline core |
| Toggle / hold / configurable shortcuts | Yes, modifier or chord | Yes, desktop-dependent | Chord registration; hold parity unverified | Windows Raw Input for bare modifier/hold; Linux portal/compositor bindings |
| Selected microphone | Yes | Yes | Engine; no full Settings UI | Native Windows input selector, stable endpoint identity |
| Level preview in Settings | No | Yes | No | Add Mac and Windows preview that releases capture on close/record |
| Recording sounds / actual RMS | Yes | Yes | Basic RMS pill; sounds unverified | Match Mac cues and waveform; no invented activity |
| Pill rolling strip, timer, sketch bands | Yes | Yes | Basic GDI meter/timer only | Mac 300×48 reference, shared state/fixtures, Windows D2D/Composition |
| Pill hover Cancel, Pause, History | Yes; Nemotron pause absent | Yes | No | Wire Windows engine commands; Mac Nemotron pause via segmented capture/restart with preserved audio clock |
| Pill Done, Not pasted, Copy/Paste | Yes | Yes | No equivalent state/recovery actions | Port persistent rescue states; never time out unsaved text |
| Silence trimming / no silence warnings | Yes; Nemotron skips trim | Yes; empty-result path not fully traced | Engine; runtime unverified | Exercise silent live capture and empty ASR on all; add Nemotron trim only if streaming semantics allow |
| Escape cancel | Yes, recording-only | X11 yes; Wayland no global Esc | No recording Esc path identified | Windows recording-scoped observer; Wayland compositor binding/portal shortcut, pill/CLI fallback |
| Paste into original target | No strong target guard/receipt | niri/X11 guard; other Wayland desktops copy-only | HWND equality guard in `deliver`; receipt absent | Add Mac guard; Windows PID/liveness + per-stage recheck; keep copy fallback everywhere |
| Terminal versus GUI paste chord | Configurable per-app image delivery | One global choice | Ctrl+V baseline | Per-app chord rules on Linux/Windows; never guess by sending both |
| Last transcript / Copy and Paste Last | Yes | Yes; CLI paste-last copies only | Shared persistence; no matching tray actions | Native actions + honest CLI naming on Windows; align Linux CLI behavior/docs |
| Failed audio / failed text fallback | Yes | Yes | Shared engine/store paths; untested | Windows visible recovery + failure injection, preserve ASR/delivery separation |
| Retry Save / recovery status | Copy Unsaved only | Yes | No full recovery UI | Add Retry Save/status on Mac and Windows; stable IDs prevent duplicate saves |
| Crash during active capture | No automatic recovery | No, audio in memory | Unverified; no durable capture journal found | Optional bounded crash-safe local recording journal, explicit privacy/retention; never claim current recovery covers kills |
| Long live dictation | Synthetic long-ASR probes, not full hardware proof | Not newly verified | Not verified | 30-minute live sessions; check memory, dictionary/progress on long-path chunking |
| Permission onboarding | Live-status first-run window | No window; portal prompts | No focused onboarding | Small Linux/Windows status surface with system-settings links, no extra dictation screens |
| Login toggle / tray or menu bar | Login on at install; no toggle | Yes | Run-key toggle + tray source | Add Mac toggle; Windows verify Explorer restart and user-disabled startup |

Evidence: `Sources/Koett/{DictationSession,DictationDelivery,RecordingOverlay,EscapeKeyMonitor,MicrophoneSelection,PermissionsWindow}.swift`; `CrossPlatform/src/{windows,runtime,audio,state,history}.rs`, `linux/{recording_overlay,input_preview,output,recovery,control}.rs`; [demo limits](video-demo-checklist.md).

## Models and words

| Feature | Mac | Linux | Windows | Gap closure / default |
|---|---|---|---|---|
| Parakeet v2 | Yes | Yes | Engine | Keep choices/defaults; native picker + download status on Windows |
| Parakeet v3 / Swedish in app | No; transcribe CLI has v3 | Yes | Engine, no picker | Add optional Mac v3 using existing supported engine integration; Windows picker; no default switch |
| Parakeet 110M primary ASR | No; auxiliary vocabulary model only | Yes, default | Engine | Optional Mac 110M primary path; validate accuracy, not just load success |
| Nemotron streaming / Parakeet recovery | Yes, experimental | Yes | Shared engine; Windows unverified | Same experimental label, capture/finalization failure tests; no delivery-triggered retranscription |
| Download verification / progress / offline reuse | Yes | Yes | Engine + tray progress | Native Windows progress/cancel/retry, checksums and atomic activation |
| CPU / GPU / ARM64 | Apple Silicon local runtime | CPU path | CPU candidate, accelerators unverified | Native Windows x64/ARM64 CPU first; WinML/CUDA/QNN only after graph/device benchmarks |
| Dictionary editor, import/export | Yes | Yes | Shared parsing/storage; no UI | Windows native Words page; same exchange format |
| Acoustic dictionary bias + adoption | v2 CTC sidecar boost; Nemotron spelling only | v2/v3 transducer hotwords; 110M/Nemotron spelling only | Same engine limitation as Linux | Bounded CTC rescorer/optional transducer sidecar for 110M; investigate Nemotron bias; spelling-only must stay labeled |
| Spelling around S1-mini | Before and after | After only | Shared Rust order | Align Rust pre/post passes to Mac using fixture/corpus checks |
| Literal replacements | Yes | Yes | Engine; no UI | Expose Windows editor; replacements remain final stage |
| Fix a word / accepted learning suggestions | Yes | Yes | Shared correction modules; no UI | Windows history action, explicit Add/Dismiss, everyday-word warning |
| Optional S1-mini formatting controls | Yes, raw fallback | Yes, raw fallback | Engine, no full controls | Native Windows page; fail back to raw, measure Windows runtime dependencies |

Evidence: `Sources/Koett/Dictionary/`, `S1MiniCleaner.swift`, `Replacements.swift`; `CrossPlatform/src/{model,formatting,runtime}.rs`, `transcription/{hotwords,nemotron,recovery}.rs`, `dictionary/`, `corrections/`; [dictionary details and benchmarks](dictionary.md). A model present in shared Rust is not proof its Windows artifact is complete.

## Drawing, pins and clipboard

| Feature | Mac | Linux | Windows | Gap closure / default |
|---|---|---|---|---|
| Drawing, hold key, pass-through otherwise | Yes | layer-shell; KWin evidence, not GNOME/X11 | No | Windows per-display HWNDs; Linux GNOME Shell integration and X11 overlay backend |
| Bare Shift without interrupting capitals/chords | Yes, gesture guards | Opt-in evdev/keyboard setup; early clicks can pass through | No | Windows recording-scoped state observer + Mac fixture rules; Linux retain explicit permission explanation |
| Undo ink | Not ported in documented Mac path | Ctrl+Z before capture | No | Add recording/drawing-scoped Mac/Windows undo; never consume ordinary editor undo outside that scope |
| Multi-monitor / mixed DPI / hotplug | Per-screen capture; hardware parity not rechecked | Hardware multi-monitor unverified; new monitor waits until next recording | No | Real dual-monitor acceptance and add/remove during capture on all |
| Word times, sketch linking, numbered contact sheet | Yes | Yes; some model times estimated | Shared data algorithms, no capture/UI | Windows compose intended ink onto captured background; keep timestamps and model approximation labels |
| Shift-click pins + app/title | Yes | niri context; others image-only | No | Windows UIA + HWND identity; Linux compositor adapters rather than guessed labels |
| Browser URL / selection | Yes; no private-mode detection | Chrome setup required; provider-dependent AT-SPI selection | No | UIA first on Windows; optional browser native-messaging bridge on all for unavailable context |
| Link/image under pointer, tiled windows | Yes via AX | niri floating only; tiled lacks position | No | Windows ElementFromPoint; Linux compositor geometry adapter or browser integration |
| YouTube timestamp link | Yes, Chrome AX player slider | No | No | Linux AT-SPI player range when matched to URL, else browser bridge; Windows UIA range + URL match, else bridge |
| Password avoidance / bounded opt-in selection | Yes | Yes, provider-dependent | No context reader | Reject password/protected UI and stale cross-window context; no synthetic Copy |
| Private/incognito URL exclusion | Not detected | Not established | No implementation | Explicit browser private-mode signal where available; absence must not imply privacy detection |
| First paste image then bullets | Yes, per-app chords | Yes, global chord; portal/read-delay limits | No | Windows staged clipboard with foreground checks each step, persistent rescue if either stage fails |
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
| File/document media entry | CLI; app focused-browser workflow | Yes | No product UI | Add Mac file/URL entry and Windows parity, no browser-cookie import |
| Pause media during dictation | No (MediaRemote restricted) | MPRIS yes | No | Windows GSMTC explicit pause; Mac browser/player-specific explicit Pause commands or opt-in integration |
| Open media transcripts | Yes | Yes | No action | Windows open folder/history action, shared filename/collision rules |
| Vimeo anonymous fetch | Known 401 | Not reverified | No workflow | Track upstream extractor/auth restriction; user-supplied local file is alternative, never quietly import browser cookies |
| `koett-transcribe` URL/file CLI | Yes, Mac-specific | No equivalent product CLI; WAV harness is not parity | No equivalent product CLI | Shared Rust CLI with same input/output/exit contract and yt-dlp/FFmpeg delivery on Linux/Windows |
| Control CLI / agent JSON | Dictionary import only | Yes | Engine harness, no equivalent app control | Add Mac/Windows local command endpoint + documented status/errors; avoid network listener |
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
