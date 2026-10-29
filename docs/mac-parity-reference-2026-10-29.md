# Koett Mac parity reference — 2026-10-29

## Evidence and limits

- Repository: `/Users/olle/dev/local-voice-input`, macOS (`Darwin`), branch `main`.
- Exact checked-out HEAD: **`10a2a7662762aa43db5cd91df6f1f13ba4a149bd`** (`docs: record installed build with meter audio fix`).
- Locally stored `origin/main` resolves to that same SHA; ahead/behind **0/0**. Origin URL: `https://github.com/oEdyb/koett.git`. No network verification of live GitHub was performed; no fetch.
- Checkout clean before and after research. AGENTS takeover commit/remote snapshot is stale; prefer the above Git facts and current source.
- Source only. No installed-app execution or interaction, physical-session UI automation, audio, host-screen capture, transcript data, Keychain, credential values, or cloud requests. Current saved preferences were not read; defaults below come from source. No build/test gates run, since this is read-only research, not a change or functional verification.
- Only artifact files under `/tmp` were created. Neither checkout nor production install was changed. No subagents.

For the PC implementer, pinned source root:
`https://github.com/oEdyb/koett/tree/10a2a7662762aa43db5cd91df6f1f13ba4a149bd`

## Capture inventory — no safe actual reference capture available

**Selected reference images: none.** Geometry and appearance below are source-derived, not screenshot-verified.

Existing ignored files found: `.tmp/microphone/menu.png`, `installed.png`, `recording-state.png`. These were **not opened or copied**. Provenance for `menu.png` is `.tmp/microphone/menu-preview.swift`: it uses an accessory app, pops a real menu onto the user's screen and executes `/usr/sbin/screencapture -x` without a window restriction. Thus it is a host-desktop capture, unsuitable for this task. The other two captures lack a verified synthetic/offscreen provenance.

`Tests/KoettTests/AssistantMarkdownTests.swift:122` builds synthetic content and calls WebKit `takeSnapshot` to assert transparent pixels, but does not persist an image. RecordingOverlay tests assert accessibility/state, not saved visual snapshots. `.build` image inventory contains upstream FluidAudio marketing/docs images, not Koett UI references.

An isolated `/tmp/koett-mac-parity-20261029/offscreen-fixture.swift` was compiled from the exact `RecordingOverlayFormat` and `RecordingMeterView` source, with synthetic constants, no controller/audio/network and no NSWindow. Its first safety guard required `NSApplication.shared.setActivationPolicy(.prohibited)` to succeed. AppKit returned false; the guard terminated the process **before any view construction/rendering**. No PNG was produced. No fallback to accessory/regular activation, visible windows, app run loop, installed process, or screenshots was attempted. This fixture is not a working reference artifact.

## Main surface: native status menu, not a dashboard

Source: [KoettMenu.swift](https://github.com/oEdyb/koett/blob/10a2a7662762aa43db5cd91df6f1f13ba4a149bd/Sources/Koett/KoettMenu.swift#L5), [Koett.swift](https://github.com/oEdyb/koett/blob/10a2a7662762aa43db5cd91df6f1f13ba4a149bd/Sources/Koett/Koett.swift#L440).

- Accessory/Login Item app, no Dock icon or main application window. Native square status item with SF Symbol `waveform`; native NSMenu typography/material/row geometry, not custom painted settings cards. Source does not hardcode menu width or row height; do not invent exact values.
- Menu order and separators: preparation/error status when applicable → Model Parakeet v2 / Nemotron 560 ms (Test) → Microphone submenu → Formatting… → Toggle / Hold → Dictation Shortcut / Media Shortcut / Reset → Ask About Screen submenu → Transcribe Current Media / Open Media Transcripts → latest result/error, Copy Last / Paste Last / conditional Copy Unsaved / Open Transcripts / conditional failed-recording and failed-transcript folders → disabled version → Quit.
- Selected model/mode/provider/voice/device use native checkmarks. Busy state disables configuration; recovery/last actions disable while busy or text absent. Latest result/error remains in the menu after the pill closes (session-only).
- Setup adds status, short reason, conditional Microphone/Accessibility settings links, Show Error…, Retry Setup. Percentages, model load/warm-up/ready use the same small feedback surface.
- Microphone root includes resolved device name or `None available`; submenu has System Default, separator, alphabetized available devices. Explicit persistent UID selection never changes the OS default. Missing explicit input falls back built-in → system default → first available, saving the replacement. System Default remains a mode. A lost active device ends capture and preserves its current audio; fallback is for the next session, never mixed mid-recording. See `MicrophoneMenu.swift:4`, `MicrophoneSelection.swift:17`, `MicrophoneAvailability.swift`.

## Recording/status/result pill

Source: [RecordingOverlay.swift](https://github.com/oEdyb/koett/blob/10a2a7662762aa43db5cd91df6f1f13ba4a149bd/Sources/Koett/RecordingOverlay.swift#L68).

All dimensions are logical points. AppKit drawing coordinates use a bottom-left origin.

- Recording/status/progress **236 × 48**. Result **250 × 48**. Borderless transparent nonactivating floating NSPanel, shadow, screenSaver level, all Spaces/apps/fullscreen auxiliary, stationary, no animation. Recording/status ignore mouse events. Anchored to screen visible-frame horizontal center, **72 pt above its bottom**; never steals keyboard focus.
- Drawn pill: bounds inset 1 pt; full half-height radius; fill sRGB `(0.075, 0.075, 0.08, 0.97)` ≈ `#131314` at 97% opacity; white 10% alpha 1 pt outline. **This pill is painted dark, not Liquid Glass or a blur material.**
- Recording: coral sRGB `(1, .31, .26)` ≈ `#FF4F42`, 8 × 8 dot at x=16, vertically centered. 17 waveform bars start x=39, width=3, step=6, radius=1.5, heights `3 + level*24`; indices 6…10 coral, others white 82%. Divider x=153, y=12, 1 × 24, white 13%. Timer centered on x=194; system monospaced digits **13 medium**, white 90%; format `m:ss`.
- Level clamps -50…0 dB then uses `pow(normalized, .75)`, smoothed `previous*.55 + target*.45`; 30 Hz. Reduce Motion removes level motion and updates elapsed time at 1 Hz. One stable accessible status, not waveform announcements every frame.
- Stop immediately becomes `Transcribing…` in the same pill until paste/error; too-short dictation produces no post-stop status. Status text 13 medium, white 90%, tail truncation, rect x=36 width=184; coral 8 pt dot x=17. Errors normally dismiss after 5 s.
- Download progress: x=36, y=10, 184 × 3 track (white 14%); coral fill fraction. Text occupies x=36, y=12, 184 × 32. Accessible progress indicator has numeric 0…1 and percent description.
- Media result: five coral mark bars with heights `[7,13,19,13,7]`, width 2.5, gap 2, starting x=16. One normalized preview, no source logo or multi-line text; text starts x=44, remaining width before Copy. Preview normalization bounds it to 96 characters, then native truncation handles width.
- Copy button rect `(width-90, 8, 80, 32)`; radius 16; white fill 12%, hover 20%, copied 16%. `doc.on.doc` symbol 13 semibold + `Copy` 12.5 semibold with 6 pt gap. Copied switches symbol to coral checkmark and label to `Copied`; closes after 1 s. Result closes after 10 s if untouched; latest result remains in menu. Accessible button supports press.

## Formatting popover and local cleanup

Source: [FormattingPopover.swift](https://github.com/oEdyb/koett/blob/10a2a7662762aa43db5cd91df6f1f13ba4a149bd/Sources/Koett/FormattingPopover.swift#L6), `S1MiniCleaner.swift:35`, `Koett.swift:142`.

- Small **300 × 190** native NSPopover anchored below the status item, transient (outside click/Escape), animations off. It stays open across setting changes.
- Vertical stack: title → four-row native NSGridView → caution note → status. Stack margins 16; spacing 10. Grid row spacing 8, column spacing 12, right-aligned left labels, **160 pt** control column.
- Title `Formatting`: system **15 semibold**. Ordinary field labels use native NSTextField defaults (source sets right alignment only; no explicit font or color). Note/status **11**, secondary. Current source note: `S1-mini by Superwhisper may change wording.` Status is `Loading S1-mini…` during preparation.
- Output: native single-selection segmented `Raw | S1-mini`; Styling popup: Casual / Semi-casual / Semi-formal / Formal; Structure popup: Prose / Lists; Context popup: General / Email. Choices editable while Raw is selected; only Output enables cleanup. Disabled during preparation or non-ready state.
- Fresh defaults: Raw, Semi-casual, Prose, General. Legacy saved style can enable S1-mini if its explicit enabled key is absent. Do not confuse fresh defaults with this machine's active saved values.
- Cleanup is local S1-mini Q4_K_M, verified 462 MiB model, warm local llama-server, official control line, thinking off, greedy decode, measured N7/M48 config. Requires llama.cpp; optional; failures fall back to raw. Store raw + cleaned text. It is optional because measured cleanup can change meaning; Linux parity must not silently enable it.
- ASR models in this Mac source: **Parakeet v2** default and **Nemotron Streaming EN 0.6B / 560 ms Test** only. FluidAudio exactly `0.15.6`, pinned revision `4dbf4f9f9a5ff3a53ade848d7ba4e3df13db859b` (Package.resolved). No Mac 110M menu, replacements editor, MPRIS integration or multilingual selector found. Existing Linux additions are not evidence of Mac behavior.

## History: file-based, no HistoryWindow

Source: [Koett.swift:594](https://github.com/oEdyb/koett/blob/10a2a7662762aa43db5cd91df6f1f13ba4a149bd/Sources/Koett/Koett.swift#L594), `TranscriptPersistence.swift`, `TranscriptRecovery.swift`, `TranscriptStore.swift`, `LastTranscriptDelivery.swift`, `TerminationProtection.swift`.

- **There is no `HistoryWindow.swift` or HistoryWindow type in this checkout.** Open Transcripts prepares and opens `Transcripts.md` with NSWorkspace in the user's default external app. There is no Mac in-app history/search/sidebar design to copy.
- Durable Copy Last / Paste Last restore `Last Transcript.txt` at startup, and run no ASR or history append. Menu actions defer until menu tracking ends so paste targets the previous app.
- Shared dictation/Ask-question/failed-media persistence uses stable record IDs, timestamp, model, text and optional raw; recovery staged as readable Markdown before atomic normal history under an advisory lock. Replay once after startup/before later results; unreadable files untouched. Copy Unsaved retains all memory-only text if both destinations fail. No database.
- Paste occurs before disk writes on success. Clipboard/paste failure still persists once and never triggers ASR recovery. Audio removed after successful ASR even if delivery fails. Failed local ASR retains audio and exposes recovery folder.
- Quit/model restart refuses during work or while completed text has no durable destination. Restart blocks admission to new sessions. These recovery behaviors are part of parity, not just UI cosmetics.

## Media

Source: [MediaSession.swift](https://github.com/oEdyb/koett/blob/10a2a7662762aa43db5cd91df6f1f13ba4a149bd/Sources/Koett/MediaSession.swift#L6), `BrowserMedia.swift:54`, `MediaTranscriptStore.swift`, `MediaTranscriptPersistence.swift`.

- Ctrl-Shift-T (rebindable), focused Chrome/Safari URL, one public HTTP(S) item with yt-dlp + FFmpeg. No cookies/login extraction. Linux MPRIS alone is not equivalent to browser-URL media transcription.
- One item; no playlist/live media; duration ≤3 h, source ≤2 GiB, socket timeout 30 s, total fetch 30 min; cancellation terminates child.
- Same already-warm Parakeet performs local media ASR. Pill phases: Finding media… → Fetching audio… → Transcribing locally… → compact Copy result. Save one dated Markdown with title, source URL and text. Failed media-file save goes through shared recovery with source/title intact. Remove temporary media audio on success/failure.
- Open Media Transcripts opens its folder. Known platform names only for exact host boundaries; no third-party logos.

## Ask panel / screen context / TTS

Source: [AssistantPanel.swift](https://github.com/oEdyb/koett/blob/10a2a7662762aa43db5cd91df6f1f13ba4a149bd/Sources/Koett/AssistantPanel.swift#L72), `AssistantSession.swift`, `AssistantWorkLifecycle.swift`, `AssistantClient.swift`, `AssistantConfiguration.swift`, `ScreenContextCapture.swift`, `CartesiaSpeechOutput.swift`.

- Ctrl-Shift-Space (rebindable): first press captures one active-display JPEG in memory (maximum width 1600, proportional height, compression .68, cursor hidden, own app excluded) and starts local voice question; second stops and sends final locally transcribed question + that image. Press during an answer cancels then starts a new question. Completed question saved before cloud work/cancellation. This feature never controls other apps.
- Provider submenu: Groq, OpenRouter, Custom HTTPS OpenAI-compatible chat completions; editable model, custom endpoint, per-provider API key. Source default models: Groq `qwen/qwen3.6-27b`, OpenRouter `~openai/gpt-latest`, custom unset. Temperature .4; no max_tokens/completion cap in request. Credentials in Keychain, never source/config screenshots. None read here.
- Borderless nonactivating right-side panel, shadow, screenSaver level, all Spaces/apps/fullscreen auxiliary, no animation. **Width 480**, height `clamp(visibleHeight*.72, 440, 660)`, right inset **18**, vertically centered on captured display.
- **Dark** appearance. macOS 26 regular Liquid Glass tinted black 18%, continuous rounded rectangle **radius 28**; older OS ultraThinMaterial fallback. Outer padding 2, content padding **22**, VStack spacing **18**.
- Header HStack gap 10: waveform 15 semibold in 30 × 30 white-10% circle → `Koett Ask` 16 semibold → spacer → conditional `Screen included` caption/display icon → 26 × 26 Close circle (white 8%; xmark 11 bold).
- Optional independent storage warning and response error, orange caption labels. Question group: `YOU` caption2 semibold secondary, 5 pt gap, question 14 secondary limited to 4 lines. Divider opacity .35.
- Main remaining answer area is transparent local WKWebView; empty state centered phase symbol 22 medium + phase label 15 medium, 12 gap, secondary. Footer caption status icon/label left; small bordered `Copy answer` right when copyable. Partial answer and Copy remain after stream failure; separate error labels do not overwrite answer.
- Phases: Listening… / Transcribing locally… / Answering… / Done / Could not answer. Symbols mic.fill / waveform / sparkles / checkmark / exclamationmark.triangle.fill.
- Renderer CSS: SF/system sans 16 px, line height 1.52, white 94%, transparent. h1 1.45rem, h2 1.25rem, h3 1.08rem; code .88em monospace; code blocks padding 12 × 13, radius 10, black 30%; tables horizontal scroll. Supports sanitized GFM, tables/tasks, highlighted code and inline/block math. Local pinned Marked 18.0.10, KaTeX .18.4, highlight.js 11.12.0, DOMPurify 3.4.14. Nonpersistent WebKit store; CSP `connect-src 'none'`, no runtime renderer network.
- Speak Replies optional/off fresh default, Katie fresh default; voices Katie/Skylar/Jameson/Gemma/Archie. Cartesia Sonic `sonic-3.5-2026-05-04`, WebSocket prewarmed while question recorded, sentence-streamed 44.1 kHz Float32 playback. Speak only opening paragraph/≤3 sentences; strip Markdown/URLs; stop before detailed/code blocks. New dictation/media/Ask immediately stops speech. TTS failure never loses text answer.

## Concrete parity gaps versus supplied Linux description

The Linux description supplied was dictation/history/meter/waveform/models 110M/v2/settings/replacements/MPRIS. Its implementation was not inspected here; omissions below mean **not described**, not proven absent.

1. Optional local S1-mini cleanup with all four formatting controls and raw safety/recovery.
2. Browser URL media fetch/transcription + source/title Markdown + compact result/copy lifecycle.
3. Ask: voice + one screen frame, provider/model/endpoint configuration, streaming rich-text/math answer panel, interruption, partial-error recovery.
4. Optional streamed spoken Ask opening with voice choice, prewarm and interruption.
5. Persistent per-device input selection with missing-device reconciliation and mid-session failure retention; all three configurable shortcuts, modifier tap/chord conflict rules and Hold/Toggle.
6. Shared durable transcript recovery, Last actions, memory-only Copy Unsaved, quit/restart protection; recovery folder actions and visible outcomes.
7. Small native status-menu hierarchy, compact painted pill, fixed source-derived geometry, Reduce Motion/accessibility, setup progress/errors. Existing Linux history/settings can remain extra functionality, but a large dashboard is not the Mac's main surface.
8. Experimental Nemotron streaming plus complete Parakeet backup WAV, recovery only for capture/stream/finalization errors; no per-recording model reload.

## Latest locally available skill/reference guidance

Canonical local skills repository `/Users/olle/dev/skills`, origin `https://github.com/oEdyb/skills.git`, HEAD **`de6ef52`** (2026-10-29). It has existing uncommitted changes, including development references; these were read and preserved. This is the latest **local working-copy guidance**, not a claim about live remote state. No fetch/cloud lookup or skill write.

Relevant sources:
- `skills/development/SKILL.md`, `references/context.md`: inspect actual repo foundation/current files first, preserve existing work, pin dependencies, prove real behavior before completion.
- Working-copy development addition **W19** (2026-10-25): improve UX, DX and AX together; names tradeoffs; preserve existing behavior/checks. `references/workflow.md:24`.
- Working-copy addition **F26**: every screen speaks once; cut taglines, helper text and duplicate secondary labels, retain real field/error/accessibility labels. `references/frontend.md:26`. F27: changing ornament alone cannot rescue wrong structure; change the named problem visibly.
- F25 (clarified 2026-10-28): no decorative status dots; a real recording/live-state indicator is meaningful. F28: neutral surfaces/one accent, coherent spacing/icon/type system, no emoji or AI badges. These are web/app general rules; **actual Mac source uses native system type and `YOU`**, so do not misrepresent the guideline as a description of current Koett.
- F1: cannot claim visual completion from source; Linux implementation still needs actual Linux captures examined after final changes. Mac physical-session capture was explicitly prohibited here.
- `references/frontend-local-library.md` now routes to **UI Atlas first** (`/Users/olle/dev/ui-atlas/atlas`), search/board/get and image judgment; offline fallback `/Users/olle/dev/skills/tools/ui-library/library.py`. Only explicit fetch downloads source; inspect reuse terms/dependencies. No Atlas service or network started in this research. Native Mac Koett source is the primary parity reference; new web component catalogs are not a reason to replace its native architecture.
- `frontend-component-sources.md` new local entries: symbools is noncommercial study only (do not ship/trace/adapt trademark marks); EasyUI is React/Tailwind/Framer Motion, not a native Mac/Linux parity implementation. They do not override native project constraints.

## Implementation handoff

Use the pinned source links and geometry above as the Mac specification. Label visual fidelity **unverified against a real Mac reference capture**. Verify Linux behavior and appearance independently with synthetic fixtures and real target screenshots; do not call a Linux HistoryWindow a copied Mac feature. Preserve Linux-only improvements while bringing the missing optional feature and recovery contracts across. No Mac production action is required by this research.
