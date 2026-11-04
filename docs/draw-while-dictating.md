# Draw while dictating

Olle asked on 2026-11-02 for Scrawl's sketch-and-talk behaviour in Koett. Scrawl
(Kit Langton, MIT, github.com/kitlangton/scrawl, v0.3.14 inspected on Olle's Mac)
is the behaviour reference only; no Scrawl code, assets, icons or name are used.

## Behaviour (Linux, implemented)

- Settings › Draw on screen: **Draw & Pin While Dictating**, off by default,
  **Draw key**: Shift (default), Ctrl or Alt, and **Include Selected Text in
  Pins**, on by default.
- A recording never blocks typing by default. Koett reads the draw key from the
  keyboard devices whenever they're readable, so the overlay never takes
  keyboard focus. About 60 times a second it samples which keys are held
  (`EVIOCGKEY`; Linux has no modifier-only query, so a sample briefly contains
  every held key), keeps only Shift/Ctrl/Alt/Super, a yes/no for "another key
  is down", the chosen draw-lock key and, while locked, Z, stores nothing and wipes the sample with volatile writes. It never reads key events or typed text, and
  opens keyboards only while a dictation with drawing records. Keyboards are
  found through sysfs capabilities and rescanned every 2 s, so hot-plugged ones
  join and unplugged handles are dropped; unreadable ones are reported.
- Only physical keyboards count. Virtual devices (uinput, including a tool that
  grabs its device so the compositor never sees it, and `ydotoold`) and mice
  that report a keyboard map (Razer Viper) are ignored, because `EVIOCGKEY` reads
  a device's state even while another program grabs it. A private test names the
  one virtual device allowed with `KOETT_TEST_DRAW_KEYBOARD=<device name>`; then
  only that device counts, so a lab can't be drawn on by anything else.
- A hold must be deliberate. The key must be seen up first, so Shift already down
  when recording starts, or left down from Ctrl+Shift+Space, never draws. Another
  modifier or key during a hold ends it until the key is released, so typing with
  Shift lets the pointer go. In the focused overlay, where a release can be missed,
  a hold with no pointer activity for 15 s is released.
- A draw-key hold under 120 ms does nothing, so typing a capital letter neither
  blocks clicks nor flashes the crosshair, and ink under 12 px is dropped as a
  resting hand rather than becoming a sketch.
- If keyboards can't be read, drawing stays off for that dictation with a
  one-line hint (main window, plus one desktop notification per run). Only the
  opt-in "If keys can't be read, let drawing take the keyboard" uses the focused
  overlay, which pauses typing while recording; Ctrl+Z undo needs that mode.
- Setup (Settings copies it): a `koett-keyboard` group, the user added to it,
  and a udev rule that adds a **read-only** ACL for that group on keyboard
  devices (owner and the `input` group unchanged). Log out and back in once.
  Any app the user runs can then read keyboards, which is why this is opt-in.
- While a dictation records, a transparent overlay covers every monitor. Hold
  the draw key and move the pointer to leave red (#ff3b30) ink. Without the key,
  clicks pass through to the apps underneath. Ctrl+Z removes the last stroke of
  a sketch that has not been captured yet.
- One draw-key hold is one sketch, as in Scrawl: releasing the key closes it
  and the next hold always starts a new sketch, on the same monitor or another,
  so a long dictation can mix talking and many separate sketches. Moving to
  another monitor during one hold starts a sketch there. A closed sketch's
  monitor is captured with the ink visible; the ink fades 1.5 s after release.
  Captures are serialized per monitor: while one is pending, only that sketch's
  ink is shown there (other and newer ink reappears after it), so an image never
  contains another sketch's strokes. The screenshot starts only once GDK reports
  wp_presentation feedback for a frame painted that way (a discarded frame is
  repainted; 2 s fallback when no timing arrives). This relies on GDK's frame
  timings: KWin, niri, Sway and Hyprland implement wp_presentation, so the time
  is the compositor's. On a compositor without it GDK may report an estimated
  presentation time, which GTK does not let Koett tell apart, so there the
  guarantee is only "GDK considers the frame done".
- Sketch numbers follow creation order everywhere (sheet, bullets, pill), even
  when sketches made during a pause share an audio time. Captures that finish or
  fail after Stop still renumber the pill until the next dictation starts.
- Sketch times for speech linking use the captured-audio clock: paused spans are
  excluded, matching the transcript's word times. The pill's bands use its own
  wall clock (`koett_engine::sketch::elapsed_ms`), which includes pauses.
- Frames are scoped to one recording. Every recording start, with drawing on or
  off, starts a new scope; a cancelled or failed recording discards its frames,
  and captures finishing after their recording ended are rejected.
- **Draw lock** (Linux). `Ctrl+Alt+D`, set under Settings › Draw on screen, toggles
  it while recording. `koett draw-lock` does the same, so a desktop keybinding can
  run it and keep the key from reaching the focused app. While locked the mouse
  draws only with the left button down, and lifting it doesn't end the sketch: every
  stroke joins one sketch, so one image, per monitor touched. Clicks reach no app,
  no pin is dropped, and typing keeps working because the overlay never takes
  keyboard focus. The pill shows the pencil for the whole lock in its usual row and
  stays clickable, so Stop and Cancel always work.
- Toggling off, stopping the recording or pressing Esc captures the sketches. Esc
  does not cancel while locked, because losing a dictation to a reflex is worse than
  a second key press; the next Esc cancels as usual. A lock ends itself if every
  keyboard disappears, since nothing else could toggle it off.
- While locked, right-click and Ctrl+Z remove the last stroke. Koett samples key
  state and doesn't consume keys, so the focused app also gets the Ctrl+Z; right-click
  reaches no app. The lock key is a physical key position and the app sees it too. A
  lock key equal to another Koett shortcut is rejected in Settings, and ignored if it
  arrives that way as the default.
- Recognizer word timestamps link speech to sketches: each sketch owns the words
  from the start of the clause in progress when it began until the next sketch;
  words before the first sketch belong to it. Clauses break at `, . ; : ? ! …`
  or a 1.5 s pause. When replacements or cleanup change the text, recognizer
  words are aligned to the final words (longest common subsequence), so removed
  fillers cannot shift a sketch's words; unmatched spans map proportionally.
- Each captured monitor is cropped around the sketch's ink, keeping at least
  45% of the monitor in each direction as context.
- On paste: one numbered contact-sheet PNG (sketches in time order, number chip
  per tile, 1600 px long edge) is pasted with Ctrl+V; once the target has read it,
  the text is pasted with the configured chord as
  `- #N (M:SS) “words”` bullets (`- #N (M:SS) marked` when nothing was said).
  Without sketches dictation is unchanged.
- After the text is read (portal: up to 1.5 s plus 160 ms; wl-copy:
  700 ms), the clipboard offers the same sheet as `image/png` and bullets as
  `text/plain` / `text/plain;charset=utf-8` on the portal path. Without the
  clipboard portal, `wl-copy` leaves `image/png` selected instead: GTK cannot
  reliably claim the clipboard automatically without a fresh focused input
  event. Ctrl+V can reuse the sketch;
  plain-text targets can read the bullets from the combined offer. Done and
  Not pasted **Copy** offer both PNG and text using the button's input event;
  Not pasted **Paste** replays image then text. The most
  recent sketch is retained in memory until another dictation completes.
  A failed image delivery keeps the result in Not pasted even if text succeeds.
- History stores the bullet text, with the plain transcript as raw text. Images
  stay in memory only, like Ask's screen image; nothing is written to disk
  except the portal's own screenshot file, which Koett deletes immediately.
- Ask recordings never show the overlay.
- Every sketch change is published to `crate::sketch` as
  `{index, start_ms, end_ms, live}` on the pill's dictation clock, numbered like
  the pasted sheet and text; the Timeline pill draws the numbered bands and its
  own drawing cue from these events. Undo or a failed capture republishes the
  renumbered sketches and sends `Removed { index }` for the freed number.

## Desktop support and limits

| Desktop | Overlay | Capture | Status |
|---|---|---|---|
| KDE Plasma 6 (Wayland) | layer-shell | Screenshot portal (one-time "Allow apps to take screenshots") | Verified end to end in private KWin 6.8.1 lab |
| niri | layer-shell | `grim -o OUTPUT` (wlr-screencopy) | Same code paths; not run on Olle's visible desktop (needs authorization) |
| Sway, Hyprland, other wlroots | layer-shell | grim, else portal | Unverified |
| GNOME Wayland | no layer-shell | — | Unavailable; dictation unaffected |
| X11 | no layer-shell | — | Unavailable; dictation unaffected |

- The ink (and the recording pill when the ink is near it) is part of the
  captured image.
- Multi-monitor: one overlay and one capture per monitor; portal images that
  span the desktop are cropped per monitor (unit-tested). The lab has a single
  virtual output, so multi-monitor drawing is not yet verified on hardware.
- On Linux, click keyframes and spoken cue keyframes are not implemented. Pins are:
  rules, context and limits are in `screen-context-proposal.md`.
- `wl-copy` cannot report when the target read the image, so the wlroots path
  waits a fixed 700 ms before switching the clipboard to text. The portal path
  waits for the actual read (max 1.5 s) plus 160 ms.
- Nemotron streaming and merged long chunks fall back to evenly estimated word
  times, so linking there is approximate.

## Mac (built 2026-11-02, live check pending)

Same output as Linux: one numbered contact sheet, then the same
`- #N (M:SS) “words”` bullets. Files: `SketchLinking.swift` (port of
`annotation.rs`), `SketchBoard.swift` (holds, numbering, capture phases),
`SketchOverlay.swift` (panels, key, pointer), `SketchCapture.swift`,
`SketchSheet.swift`, `SketchFrames.swift`, `SketchPaste.swift`,
`SketchDictation.swift`, `DrawKey.swift`.

- Menu › **Draw on Screen**: **Draw & Pin While Dictating** (off by default),
  **Include Selected Text in Pins** (on by default) and
  **Draw Key** Shift (default), Control or Option. A key the modifier-only
  dictation shortcut uses is disabled (Option with the default "Either
  Option"), and Koett falls back to the first free key. Turning drawing on
  asks for Screen Recording once.
- One transparent, non-activating panel per screen at `.screenSaver` level,
  shown only while a dictation records. The draw key comes from `.flagsChanged`
  global and local monitors, so the panels never take keyboard focus and typing
  keeps working. Key-down/up codes track other held keys; typed characters are
  never read. The draw key must first be seen released in this recording. A
  modifier chord or another held key ends a drawing hold and blocks it until
  the draw key is released, so dropping Control from Control-Shift cannot
  start drawing. While it is held the panels take the pointer; otherwise clicks
  pass through. The pointer is sampled at 120 Hz while held.
- A hold starts a sketch only after the pointer moves 3 pt, so Shift for a
  capital letter leaves no dot. The sketch's start is the sample where the
  movement began. Otherwise holds, screens and numbering follow Linux.
- Capture: on release, ScreenCaptureKit takes a still of that display with
  every Koett window excluded (overlay, pill, panels), and Koett draws only that
  sketch's strokes onto it before cropping (same crop rule as Linux). A capture
  therefore cannot contain another sketch's ink or the pill, and needs no
  frame-presentation sync. Ctrl+Z undo is not ported: the overlay never has
  keyboard focus and captures start on release.
- Pins (Linux has them too, read through niri and AT-SPI instead): a click while the draw key is held drops numbered
  pin #N at the click instead of ink. It shares numbering, linking, the sheet
  and the paste with sketches; its tile is a crop around a red ring, and its
  bullet gains a context line read through Accessibility only (window owner,
  page URL and title, link or image under the pointer, YouTube `t=` from the
  `ytp-progress-bar` slider, selected text capped at 500 characters). Shift+move
  never pins, Shift+click never starts ink; the full gesture table and its tests
  are in `screen-context-proposal.md` and `SketchPinTests.swift`.
- Clocks: linking uses the recorder's written-audio time (`currentTime`, paused
  buffers are never written); the pill uses its own wall clock. Parakeet token
  times come from FluidAudio `tokenTimings` (`▁` marks a word start); they are
  already mapped back across silence trimming. Nemotron and mismatched tokens
  fall back to an even estimate over the recording.
- Scope: every dictation start calls `SketchFrames.begin`; Cancel discards the
  overlay and frames; captures from another dictation are rejected. Delivery
  waits up to 8 s for open captures, then the last Cancel check runs.
- Pill: `SketchUpdate.upsert`/`.removed(index:)` go through
  `RecordingOverlayController.applySketch(_:session:)`, accepted after Stop for
  the same dictation until the next one starts, so a capture failing during
  transcription renumbers the bands.
- Paste: the sheet is offered lazily as PNG and TIFF, Command-V is posted,
  Koett waits for the target's read (max 1.5 s) plus 160 ms, then pastes the text
  as before. History stores the bullets with the plain transcript as raw text.
  A clipboard manager that reads first makes Koett wait the full 1.5 s.
- Proof: `SketchLinkingTests.testOutputMatchesLinuxByteForByte` replays 300
  dictations generated by the Linux linker (`scripts/sketch-linking-golden.rs`);
  a 0.1 s change to the clause pause fails 7 of them. Sample sheet render:
  `docs/evidence/mac-draw-20261102/contact-sheet.png`.
- Terminals: when the frontmost app is Terminal, iTerm2, Ghostty, kitty,
  WezTerm, Alacritty or Warp (by bundle ID), the sheet is pasted with
  Control-V so terminal agents such as Claude Code read the image; the text
  still uses Command-V. VS Code and Cursor are not detected (their terminal
  shares the editor's app and its focus is not readable without forcing
  Electron accessibility on). Draw on Screen › **Paste Sketch Image with
  Control-V in <app>** flips the chord for the app in front, stored per bundle
  ID in `sketchImagePasteControlV`. In a plain shell, Control-V is quoted
  insert, so the first pasted character arrives literally.
- Limits: screens added during a recording get no panel until the next one.

## niri fallback ownership repair — 2026-11-04

The host advertises RemoteDesktop AvailableDeviceTypes=0 with its configured
GNOME portal backend under niri. Koett checks for keyboard support before opening
a session and therefore intentionally selects wtype. ScreenCast support does not
imply RemoteDesktop keyboard support. The fallback must preserve the same clipboard
bundle as the portal path; PNG-only wl-copy after automatic sketch paste was a bug.

The sketch fallback now owns the regular clipboard through niri's
zwlr_data_control_manager_v1. One in-process worker transitions image → text →
PNG+HTML+plain-text bundle. An additional empty, opaque provenance MIME type lets
it distinguish its own selection notifications from intervening external copies.
It drains queued and socket-readable events immediately before claiming, retains
the old source through a bounded confirmation barrier, and never retries a lost
claim. Each Wayland synchronization has a deadline and observes shutdown; dropping
the owner waits at most 250 ms and never performs a blocking join.

The protocol has no conditional set-selection operation. A new copy in the last
cross-client scheduling gap can still be overwritten. Koett detects the foreign
selection followed by its own claim, reports ownership_race instead of success,
and stops writing. It cannot safely restore the other app's now-canceled source;
the visible error asks for the newer item to be copied again. It does not guess
or reclaim content from clipboard history. Thus strict atomic newer-copy priority
is not achievable in this protocol; the ordinary cancellation path preserves it.

Started transfers retain immutable Arc bytes from their own source and finish
even after ownership loss, with a five-second stalled-reader deadline. A global
cap of four writers bounds concurrency; spawn errors close the request and are
reported without panicking. The controller stops the owner on exit. There is no
helper process or temporary image file. Plain dictations still use their existing
wl-copy text path. Explicit recovery Copy can start a fresh bundle.

Review regressions cover a controlled real-niri copy in the preflight/claim gap
(detected overwrite), a real 2 MiB transfer continuing across selection replacement,
a silent fake compositor deadline, bounded owner drop, shared payload allocation,
writer-cap exhaustion, and injected spawn failure. The supply-chain CI failure
at 29e599c was stale generated inventory/SBOM/gaps, not a policy rejection. The
new crate licence texts and data-control protocol copyright are now bundled with
provenance, and inventories regenerated; licence/advisory policy is unchanged.

Delivery logs include PID, step ID, image/text/offer kind, backend, attempt=1,
byte count, and begin/paste_post/end events. Clipboard reads log MIME and byte
count only. No transcript is logged. No text delivery retry loop was found:
automatic completion posts image once and text once, restores the offer without
keys, then persists. Recovery Paste is a separate explicit operation. ASR
recovery and history failures do not replay a completed clipboard delivery.

The ignored native niri regression exercises the actual fallback image/text
functions with an absent target (clipboard changes, paste guard refuses keys),
then the full bundle, repeated exact reads, newer-copy preservation and shutdown.
The private consumer probe exercises the same owner and final payload. Chrome
rich editor consumed both once per paste; old T3 retained image data. Its DOM
image count is not a count of attachments. No updated-T3 claim is made.
Nested niri did not deliver wtype key events to the browser, and kitty's private
paste action produced no terminal input, so automatic-key and kitty end-to-end
verification remain open. Exact plain bytes were verified via native wl-paste.
[Receipt](evidence/linux-sketch-clipboard/fallback-owner.json) and
[consumer script](evidence/linux-sketch-clipboard/fallback-owner.py) record scope.
No host input, Swift, portal configuration or installation was changed.

Portal API: [AvailableDeviceTypes](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html).
Niri documents the GNOME backend for [screencasting](https://github.com/niri-wm/niri/wiki/Screencasting).

## Passive rich repeat paste — 2026-11-04

The portal's final sketch offer and the existing Copy button now retain four
MIME types together: PNG, HTML, plain UTF-8 text and plain text. HTML embeds the
same PNG as a data URL followed by the escaped, unmodified transcript in a pre
block. Image-aware targets can still select PNG; rich editors can insert both
from HTML; terminals can select plain text. Nothing runs when a format is read.
PR #48 left the wl-copy fallback PNG-only; the niri ownership repair above
replaces that sketch backend. Copy uses the rich offer regardless of the
automatic-paste backend.

Private KWin tests exercised the production Copy helper's exact simultaneous
offer in T3 Code 0.1.2, Chrome's native rich editor and kitty. T3 retained the
image on two pastes (it still does not insert the text). Chrome inserted one
image and exact Unicode transcript per paste, including a second paste from
the same offer. Kitty received exact plain text. Both PNG bytes and the embedded
PNG were checked against the private drawing fixture. These are clipboard target
tests, not a new end-to-end automatic-dictation or Mac runtime test.

T3's receiver fix `1d4662546a` ships in 0.1.24 but was not installed for these
checks. Its real repeat-paste verification remains pending: paste two lines
including @AGENTS.md and a sketch into existing composer text, then repeat and
check both lines, one attachment per paste and surrounding text. Also compare
PNG+text with this exact HTML bundle and check ordinary browser-image copying.
The HTML carries actual transcript body text; its fixed alt is “Koett sketch”,
never the dictation. Retain HTML even after T3's fix because Chrome's native
rich editor needs it to insert both from one paste.

[Combined-offer receipt](evidence/linux-sketch-clipboard/combined-offer.json) and
[private target script](evidence/linux-sketch-clipboard/combined-offer.py).
The fixture came from private dictation/drawing with copied models; host focus,
clipboard, model folders and installed application were untouched.

Automatic image-then-text delivery, its focus guard and the single Copy button
in the 48 px Done pill are unchanged. Request-triggered replay was rejected:
a canceled real Ctrl+V plus background reader could trigger unwanted pasting;
first-format inference could duplicate images. No replay or key monitor is added.
Mac PR #46 at `6ed0ead` likewise offers image and text representations separately;
a recipient can choose only one. Swift is unchanged and was not runtime-tested.

## Repeat-paste repair (2026-11-04)

Before the repair the clipboard was PNG during the first paste, then text only
after the bullet paste. Done/Not pasted Copy also wrote only text, and Paste
recovery replayed only text. The final portal offer now includes both formats without
changing either automatic paste or its focus checks. The wl-copy fallback
preserves the image instead; pill Copy offers both using its fresh input event.
An unfocused GTK clipboard claim returned success but left text selected during
the private full-app drawing test, so automatic restoration uses the existing
portal or wl-copy backend. A combined offer is preferred where reliable so
terminals retain usable text and image-aware targets can attach the sheet. Chrome's native contenteditable chooses the image; targets
decide how much of a multi-format clipboard to insert.

The native `sketch_clipboard_probe` example uses the production Copy helper.
Private KWin/Chrome checks read both exact PNG bytes and exact Unicode text, and
two Ctrl+V events each exposed a PNG file plus text. Kitty Ctrl+Shift+V received
the exact bullets while PNG remained readable. Unit coverage checks all three
MIME types, plain-text behavior and replacement of the in-memory sketch.
T3 Code 0.1.2's actual composer attached the image on each Ctrl+V after removing
the previous attachment; no message was sent. It chooses the image rather than
inserting the offered text. [Target receipts](evidence/linux-sketch-clipboard/targets.json)
cover the production Copy helper and real clipboard consumers; they do not claim
a fresh microphone/drawing end-to-end run.

Mac routing: `SketchPaste.swift` offers PNG/TIFF, then
`DictationSession.swift` clears the pasteboard and sets only the string.
Mac therefore has the same repeat-paste loss by source inspection; Swift was
not changed or runtime-tested here.

## How this was chosen (2026-11-02)

Olle asked for parallel takes. Two sub-agents built alternatives from the same
base commit; each was compared on its own checks and against the lab flow above.

- **Speech linking take** (`take/sketch-linking`): a 16-scenario labelled corpus
  (draw-then-speak, two sketches in one clause, long pauses, filler removal…).
  The original proportional mapping scored 13/16 scenarios, 48/51 words; the
  word-alignment mapping scored 16/16, 51/51. Adopted, with the corpus as
  regression tests (`src/annotation/evaluation.rs`).
- **Clean-frame capture take** (`take/sketch-clean-frame`): capture a clean frame
  when a sketch starts, composite the ink in software, crop around it. Its crop
  was adopted on top of the verified capture-with-ink path. Clean-frame capture
  was not: it was never measured on a real capture, can race the first stroke,
  and the first frame can still contain the pill.

## Proposal: drawing without blocking typing (for Olle, 2026-11-02)

Today the overlay holds exclusive keyboard focus for the whole recording,
because a Wayland client cannot see a bare Shift press unless it has focus.
That blocks typing into other apps while dictating with drawing on. Options:

| Option | How the draw key is seen | niri | KDE | Cost |
|---|---|---|---|---|
| A. Current | Overlay keeps keyboard focus all recording | Works | Works | Typing blocked while recording |
| B. Compositor trigger | A desktop shortcut starts draw mode; only then does the overlay take focus. It sees the key release itself and gives focus back | Press-to-start: niri binds fire on press (no release action, to our knowledge), e.g. `Mod+Shift+D { spawn "koett" "draw"; }`; draw mode ends when Shift is released or with Esc | The GlobalShortcuts portal reports press **and** release, so a true hold works; KDE does not allow a bare Shift binding, so it is a chord (e.g. Ctrl+Alt+D) | Not bare Shift: one extra key or chord per sketch; typing never blocked |
| C. Read keyboard devices (evdev) | Koett reads `/dev/input` key state directly | Works, true bare-Shift hold | Works | Needs the `input` group or a udev rule, which would let Koett read every keystroke system-wide; plan.md rules this out as a default |

**Decision (Olle, 2026-11-02): maximum UX — bare Shift, many separate sketches
per dictation, typing never blocked. Built as C (device state, opt-in with
guided setup) with A as the fallback.** Original recommendation, kept for the
record: make **B the default** and keep **A as an opt-in** ("Shift
anywhere — blocks typing while recording"). On niri, bind one key to
`koett draw`; draw mode starts focused, Shift-move draws as now, and focus
returns on release or Esc. On KDE, register a portal hold shortcut. Offer C
only as a documented advanced setting, never by default. B needs one small
control command and a per-desktop setup note, and removes the typing block
without new privileges.

## Mac repeat paste and recovery — 2026-11-04

Automatic delivery still sends the sheet first and the bullet text second. The
text is now offered lazily too: after its read (1.5 s cap plus 160 ms settle),
Koett restores **one pasteboard item containing PNG, TIFF and plain text**. Another
Cmd+V can select the sketch; a text-only target selects the same exact bullets.
If another application changes the clipboard during that wait, Koett leaves the
newer content alone. Posting a paste event cannot prove a target consumed it.

Done Copy, Not pasted Copy, Copy Last Transcript and Paste Last Transcript retain
both formats for the latest drawing result. Recovery Paste replays image then
text and restores the combined item without appending history again. The image
stays in memory only, until another completed result replaces it or Koett quits;
history and restart recovery remain text-only. A plain next result clears the
old image even if its words happen to match. No pill size or visual changes.

### Mac draw-key activation audit

`SketchOverlay.start` calls `followKey` immediately and installs local/global
`.flagsChanged` monitors. `followKey` reads `NSEvent.modifierFlags`; while drawing,
the 120 Hz ticker reads that same value. `DrawKey.draws` requires the configured
key to be the only Shift/Control/Option/Command modifier. With no modifiers it is
false. The configured key can be Control or Option, and a shortcut conflict can
select a different effective key; Shift is not always the active draw key.

The audit found that a key held at recording start, or releasing Control while
Shift remained down, could engage drawing without a new Shift press. `DrawHoldGate`
now requires the draw key to be observed up in this recording. A chord or normal
key press spoils the hold until the draw key is released; releasing only the
other key cannot resume drawing. The existing 3 pt pointer threshold stays.

macOS flags are not proof of a physical keyboard press. No Mac code posts Shift.
The Linux bug in PR #44 reads a grabbed virtual device through `EVIOCGKEY` even
when its Shift never reaches the compositor. Mac does not read those devices,
so that specific cause does not apply. Physical phantom-Shift behavior was not
reproduced. Gate tests cover startup holds, Control-Shift release, other modifier
and normal-key interruptions, fresh holds and Caps Lock.

AppKit contract: each copy creates a fresh
[`NSPasteboardItem`](https://developer.apple.com/documentation/appkit/nspasteboarditem)
with multiple representation types. Pasteboard items are not cached or reused;
only the image bytes and text are retained for the next copy.

Mac clipboard and draw-hold regression receipts live in
[`docs/evidence/mac-sketch-clipboard`](evidence/mac-sketch-clipboard).
The final full suite ran 396 tests with 9 intentional skips and zero failures.
The production overlay is tested with an empty screen provider, so no panels or
screen captures are created by the hold-transition regression.


## Linux Undo Last Drawing

During a dictation, **Menu → Drawing → Undo Last Drawing** or the tray action
removes the most recently created drawing or pin, even after its image was
captured. The matching image is removed from delivery, its pill band disappears,
and a late capture result cannot restore it. Undo can be repeated until empty.
It is disabled after recording ends; cancelled/new recordings cannot inherit it.

`koett drawing-status` reports active/count/captured_count/can_undo without image
or context content. `koett undo-drawing` performs the same action, returning
`drawing_removed` or a `no_drawing` error with exit status 1. An expired queued
request makes no change. No global Ctrl+Z binding is added: ordinary editor undo
stays with the editor. Existing focus-mode Ctrl+Z edits only current uncaptured
ink. See [verification and desktop limits](linux-drawing-undo-verification.md).
