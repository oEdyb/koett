# Scrawl comparison: November 2026

Research date: 2026-11-05. **Recommendation: shorten the image-to-text gap, not change the order.** Olle clarified: “there's like a big delay between when they get pasted.” Scrawl offers a useful latency reference, but its fixed delay is not evidence of reliable delivery in every target.

## Scope and evidence

- Koett: fetched `origin/main`, inspected `61e121e13044a7eb1df8d4ebf3fb8b1a889884d3`, including Mac Swift and Linux Rust. These are source capabilities, not a claim about the currently installed binaries.
- Scrawl main: `b5fa070` (2026-11-01 UTC), latest stable [v0.3.14](https://github.com/kitlangton/scrawl/releases/tag/v0.3.14). Linux: separate draft branch `feat/linux`, `79b131599662ac872fe8ab07e98728a6d93d46a0`, shipped as [Linux preview 4](https://github.com/kitlangton/scrawl/releases/tag/linux-preview-4), based on 0.3.13. Do not infer Linux behavior from main's unsupported-platform stubs.
- Cloned separately into `<build-home>/.cache/koett-research/scrawl-2026-11-05-1793838266`. Read README, release history, recent commits, all issue bodies returned by the paginated issues API (excluding PRs), selected issue comments, and relevant source. No Scrawl build, execution, installation or benchmark.
- **Verified** below means source/API inspection. User reports are attributed. All real-app latency, accuracy, permission flow, hardware and compatibility outcomes are **unverified in this research**. Existing Koett receipts are historical evidence, not fresh tests.
- Behavior and ideas only. No Scrawl implementation, assets, icons or branding are copied into Koett.

## What changed since the previous v0.3.14 inspection?

**No newer product release or main-branch behavior found.** `git log v0.3.14..origin/main` contains only the two Sparkle appcast commits. The v0.3.14 product fix is Hex 3 API discovery (try the `hex` data directory, then legacy `voice-control`), already present at the previous inspection. Linux preview 4 predates that inspection too. There is no separate changelog file in the inspected tree; release notes and commit history supply the change record.

Recent changes worth understanding, already present by v0.3.14: language selection and command transcribers, arrow/circle recognition, macOS 15 support, optional audio ducking, clearer numbered frames, permission-helper exclusions (0.3.13); simpler Cancel/Finish pill and crash recovery (0.3.12). See the [release history](https://github.com/kitlangton/scrawl/releases). New issue activity is evidence of remaining problems, not new shipped fixes.

## Paste gap: the decisive comparison

### Scrawl

[Mac/Windows handoff source](https://github.com/kitlangton/scrawl/blob/b5fa070/src/handoff.rs#L261-L301) prepares the image and summary, waits for shortcut modifiers to release (up to 350 ms), copies image/path, waits **20 ms**, posts paste, waits **160 ms**, copies text, waits **20 ms**, and posts paste again. Therefore the two synthetic paste calls are separated by **180 ms of explicit sleeps**, plus clipboard and event work. It does **not** wait for a clipboard-read acknowledgment. This is a code-derived schedule, not measured time until the attachment or text appears.

For GUI apps it offers PNG first; source comments name OpenCode Desktop, Slack, Linear and browsers. For recognized terminals it pastes a **persistent image file path** first, allowing an agent TUI to attach it, then summary text. Comments name Ghostty, OpenCode TUI and Claude Code. These are intended targets, not an exhaustive tested compatibility list.

[Linux handoff](https://github.com/kitlangton/scrawl/blob/79b131599662ac872fe8ab07e98728a6d93d46a0/src/handoff.rs#L291-L314) uses the same explicit 20/160/20 ms schedule. [Linux platform notes](https://github.com/kitlangton/scrawl/blob/79b131599662ac872fe8ab07e98728a6d93d46a0/docs/linux.md) describe X clipboard ownership, `wl-copy` on KDE, and RemoteDesktop portal key injection on Wayland. Issue [#91](https://github.com/kitlangton/scrawl/issues/91) reports that Alt+Shift+V fails where that portal is absent. A short timer cannot fix missing transport.

Mac normal Copy copies summary text; repeat paste shortcut reruns the full image-and-text sequence from the stored bundle. The automatic handoff ends with text on the clipboard; it does not restore the prior clipboard or Koett's combined image/text offer. Windows has an experimental combined-format Copy, which should not be confused with the default Mac path.

### Koett

[Mac SketchPaste](../../Sources/Koett/SketchPaste.swift) offers lazy PNG/TIFF, posts image paste, polls every **10 ms** for a read after the post, waits up to **1,500 ms**, then adds **160 ms** before continuing to text. Thus an immediate read gives roughly 160 ms plus polling/processing; a missed read or unsupported image target can cost roughly **1,660 ms** before text preparation. A clipboard manager reading before the paste can consume the provider callback early and leave the full timeout. This is a plausible cause of the reported gap, **not a reproduced diagnosis**.

[Linux output](../../CrossPlatform/src/linux/output.rs) uses **700 ms** unconditionally after fallback image paste; the clipboard portal route uses the **1,500 ms + 160 ms** read/settle policy. These are different paths, so there is no single Koett paste latency. Mac [readiness checks](../../Sources/Koett/PasteReadiness.swift) can additionally wait up to 1.5 seconds per preparation for held modifiers or an unready target. Capture/transcription time before the image appears is a separate metric.

After text consumption Koett may wait again before restoring its combined clipboard item. That final wait affects recovery/clipboard readiness, **not the earlier image-to-text gap**. Do not add both waits and call the sum the visible gap.

**Keep image then text.** Both products use it; an attachment arriving before the explanation is coherent for agent inputs. No evidence here shows reversing the order improves compatibility or the observed pause. Koett should pursue Scrawl-like responsiveness while preserving its read handling, focus checks and recovery. Do not globally replace 700/1,500 ms with 160 ms: Scrawl overwrites the clipboard without knowing whether the target has consumed the image.

Measure separately: image paste event, clipboard read, visible image attachment, text paste event, visible text. Exercise Claude Code/Ghostty, OpenCode desktop/TUI, a browser chat input, Slack, and a plain-text editor, with and without a clipboard manager and under CPU load. Record missing/duplicate attachments as well as median/tail gaps. Target roughly **200 ms between events on confirmed fast consumers** as an experiment, not a promised visible latency or universal timeout.

## Ranked improvements worth taking

Effort is a planning estimate, not a measured delivery commitment: small = 1–2 engineering days; medium = 3–5; large = more than a week including platform checks.

### 1. Make image and words feel like one delivery

**Scrawl:** a short two-event handoff, about 180 ms explicit inter-event delay ([source](https://github.com/kitlangton/scrawl/blob/b5fa070/src/handoff.rs#L278-L301)). **Benefit:** directly addresses Olle's complaint. **Effort: medium**, including measurements and slow-reader regression checks on both platforms. Start with Koett's unconditional 700 ms fallback and missed-read cases; preserve reliable behavior for unknown consumers. **Koett already beats it:** guarded focus, newer clipboard preservation and combined repeat-copy formats. Keep those safeguards. Scrawl has no measured end-to-end advantage established here.

### 2. Undo completed drawings on Mac

**Scrawl:** Cmd+Z removes the last stroke ([controls](https://github.com/kitlangton/scrawl/blob/b5fa070/README.md#capture-controls)); recording pill was simplified to Cancel/Finish in 0.3.12. **Benefit:** a mistaken mark should not force restarting a dictation. **Effort: medium** for a scoped native action, frame removal and late-capture rejection. **Koett already beats it on Linux:** Undo Last Drawing removes even captured drawings/pins, repeatedly, through menu/tray/CLI; [receipt and limits](../linux-drawing-undo-verification.md). Mac lacks drawing undo. Prefer explicit recording-scoped undo before considering a global Cmd+Z interceptor that could steal editor undo.

### 3. Verify permission completion with an actual capability check

**Scrawl:** welcome permission rows update automatically; a fresh helper checks grants and a one-frame capture probe detects stale Screen Recording access before relaunch ([permission flow](https://github.com/kitlangton/scrawl/blob/b5fa070/README.md#welcome-and-permissions)). **Benefit:** avoids “enabled in Settings but still broken.” **Effort: medium on Mac**, with clean-install/revocation checks; Linux needs separate portal/device checks. **Koett ahead:** normal dictation does not require screen access; drawing remains opt-in. Take capability checking and recovery, not a mandatory multi-screen tour or screen permission for everyone.

### 4. Improve interpretation of marks without taking more screenshots

**Scrawl:** identifies arrow/circle/line/pin shapes and describes them in structured marks ([mark interpretation](https://github.com/kitlangton/scrawl/blob/b5fa070/src/sketch.rs#L308-L469)). **Benefit:** an agent can distinguish “move this here” from “change this region.” **Effort: medium**, requiring a held-out gesture corpus; recognition must never alter the original ink or pretend certainty. **Koett already beats it in Linux pin context:** selected text and accessible target context are available, whereas Scrawl's Linux `agentContext` is absent. Shape labels are an optional enhancement, below latency and undo.

### 5. Add a simple signed Mac update check

**Scrawl:** Sparkle updates installed Mac builds; Homebrew delegates updates to it ([installation](https://github.com/kitlangton/scrawl/blob/b5fa070/README.md#install), [appcast](https://github.com/kitlangton/scrawl/commit/fa4580e)). **Benefit:** users receive fixes without reinstalling manually. **Effort: large**, including signing, feed ownership and rollback/release validation. **Koett:** no comparable updater found in inspected app sources; source/build installation remains manual. Scrawl Linux/Windows only open the releases page. This is release work, not a reason to copy its cross-platform shell.

## Behavior users feel: remaining comparisons

**Drawing and pins.** Both support modifier-hold ink, pass-through normal clicking and numbered pins. Scrawl uses Shift, a short low-motion click for pins, and fading red ink. Koett has configurable draw keys, deliberate-hold gates and typing protection; Linux also has draw lock for several strokes in one sketch. Its keyboard access is opt-in and limited to recording-time state sampling; Scrawl Linux requests the broader `input` group. Neither grant should be described as harmless. Koett's configured-key conflict handling and physical-device filtering are concrete strengths. [Koett behavior contract](../draw-while-dictating.md).

**Speech linking.** Scrawl associates clause-aligned text with frames and can cut extra frames on spoken cues, clicks and visual changes. Koett aligns words to explicit sketches/pins, including after text cleanup; it does not continuously record the screen. Both have estimated-timestamp fallbacks. Scrawl offers richer replay/context, while Koett keeps capture bounded and local dictation simple. Do not adopt continuous screen recording or cue-generated frames for this request.

**Focus and clipboard.** Scrawl can drag a Paste chip onto an input, click it, wait 65 ms, then paste; otherwise Mac restores the last external app if needed, waits up to 220 ms for activation plus 25 ms settle ([activation source](https://github.com/kitlangton/scrawl/blob/b5fa070/src/capture/macos.rs#L944-L976)). Koett's per-stage target identity/readiness and clipboard-generation guards are stronger protection against delivery into a newly focused window. Linux reliability is desktop-specific in both. Posting a key event is never proof of insertion. Koett's combined image/text recovery is better for repeat Cmd/Ctrl+V, but its images are memory-only; Scrawl's durable bundles support recovery and revisiting old captures. Keeping images after restart would be a separate retention/privacy decision, not a free improvement.

**Models and accuracy.** Scrawl Mac defaults to local Apple Speech (SpeechAnalyzer on macOS 26; older on-device recognizer on 15), offers Hex and a command adapter; Linux preview uses downloaded, hash-checked Parakeet through transcribe.cpp. Experimental live Apple transcription reduces post-stop work but is not the default. Koett defaults to resident local Parakeet, with optional engines/cleanup; transcription stays independent of drawing. Issue [#93](https://github.com/kitlangton/scrawl/issues/93) reports Apple lag/errors; [#89](https://github.com/kitlangton/scrawl/issues/89) and [#90](https://github.com/kitlangton/scrawl/issues/90) report Hex dropping quiet/short speech. No shared accuracy corpus or speed benchmark was run: **no winner on recognition accuracy**. Avoid silence segmentation that deletes soft words; do not switch models on anecdote.

**Pill and overlay.** Scrawl provides recording-mode chips briefly at start, a compact Cancel/Finish state, and completed capture actions. Koett's pill already connects numbered sketch bands to the spoken timeline and supports pause/recovery. Scrawl's broader mode picker serves screen capture; Koett should keep its smaller dictation flow. On Mac Koett excludes its windows from still capture and composites only the intended sketch; Linux can include its pill/ink. Neither Scrawl nor Koett establishes universal Linux overlay support.

**Multi-monitor.** Scrawl starts on the display under the pointer, offers area/window/display selection and follows recording targets. Koett provides per-monitor sketch capture and ordered numbering, with documented hardware-verification limits. Scrawl Linux XWayland coordinate limitations and stale native-Wayland pointer position are substantial; Koett's layer-shell path is a better fit on supported compositors, but its drawing is unavailable on GNOME/X11. Scrawl has historical GNOME/X11 coverage Koett drawing lacks. Do not claim Koett wins every Linux desktop.

## Complaints and praise to learn from

- [#121](https://github.com/kitlangton/scrawl/issues/121), including follow-up: niri black/tiled overlays, scaled-output coordinates and missing movie finalization; newer niri fixes capture start but not the opaque overlay. [#91](https://github.com/kitlangton/scrawl/issues/91): Hyprland click failures, wrong monitor and missing paste portal. Test actual native Wayland targets and output topology; an app launch is insufficient proof.
- [#117](https://github.com/kitlangton/scrawl/issues/117) and [#99](https://github.com/kitlangton/scrawl/issues/99): tap-to-click and settings controls not responding. Check normal trackpad taps, not only injected full clicks. [#9](https://github.com/kitlangton/scrawl/issues/9), closed: pin gesture ambiguity; retain Koett's deliberate gesture distinction.
- [#115](https://github.com/kitlangton/scrawl/issues/115): wrong Chrome window title. The reporter also says they “really like scrawl.” That is qualitative praise, not evidence of a specific winning feature. Bind context to the actual window, not just its process.
- [#98](https://github.com/kitlangton/scrawl/issues/98): always-on LAN ingest listener and token exposure concern. Keep Koett's no-server normal dictation path. [#1](https://github.com/kitlangton/scrawl/issues/1), closed: long-stroke Metal buffer crash on dual 5K displays. Include long strokes and high-density displays in overlay validation.
- [#126](https://github.com/kitlangton/scrawl/issues/126) and [#125](https://github.com/kitlangton/scrawl/issues/125): requests for completion-action choice and a dedicated finish shortcut. These are requests, not implemented features. The #126 reporter explicitly says they are loving Scrawl while asking for less completion friction. Overall evidence supports demand for quick capture and direct agent handoff; no systematic satisfaction data or latency praise was found in the inspected issues.

## Decision and limits

Prioritize the visible paste gap, then Mac undo, then permission capability recovery. Keep image-first delivery and Koett's stronger clipboard/focus safeguards. Do not import Scrawl code, continuous capture, agent routing, a LAN listener or a mandatory permission tour. No app code or defaults changed by this report.

Follow-up verification should establish actual attachment/text arrival times and failure rates before choosing shorter waits. This research deliberately did not build/run either app, grant permissions, interact with Olle's desktop, or claim a runtime comparison.
