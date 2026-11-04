# Screen context pins

Status: approach A chosen on 2026-11-02. Mac is built (`PinContext.swift`,
`PinContextReader.swift`, `SketchBoard` pins, tests in `SketchPinTests.swift`);
live check pending. Linux is built (`pin_context.rs`, `linux/sketch/gesture.rs`,
`niri_pick.rs`, `pin_reader.rs`, `wl_click.rs`); see "Linux" below. Ask stays until pins have been used for about a week, then is removed.

Koett is used mostly to dictate into AI agents (Claude Code in a terminal,
browser chats). The goal is that the agent also gets *where you were
looking*: the video, the page, the image, the line of code, without
switching apps or copying links by hand.

## What can be read without a full screenshot

| Context | Mac | Linux (niri, Wayland) |
|---|---|---|
| App and window title under a point | `CGWindowListCopyWindowInfo` front-to-back, bounds and owner PID; titles need Screen Recording, which drawing already has | niri `PickWindow` finished by one synthetic click: `tile_pos_in_workspace_view` is null for tiled windows on niri 26.04, so positions cannot locate one |
| Browser URL and page title of the clicked tab | Accessibility: walk up from the element at the click to `AXWebArea`, which carries `AXURL` and `AXTitle` (checked on Olle's Chrome, 2026-11-02). Works for Chrome, Brave, Edge, Arc, Safari. No AppleScript, so no Automation prompt | AT-SPI `Document.GetAttributes` (`URI` on Chromium). Chrome exposes its page only when started with `ACCESSIBILITY_ENABLED=1` and `--force-renderer-accessibility`; either alone is not enough |
| Element under the pointer (link, image) | `AXUIElementCopyElementAtPosition` on the app's element, then `AXURL` / `AXDescription` | AT-SPI `Component.GetAccessibleAtPoint`; needs window coordinates, which niri gives only for floating windows |
| Selected text | `AXFocusedUIElement` → `AXSelectedText` | AT-SPI `Text.GetSelection`. PRIMARY is not used: nothing proves which app owns it |
| YouTube video time | YouTube's progress bar is `role="slider"` with `aria-valuenow` in seconds and class `ytp-progress-bar` (checked in the live player `base.js`, 2026-11-02). Chrome exposes it as an `AXSlider` with `AXDOMClassList`, so Koett reads `AXValue` | Not read: the paste has the plain link. Chromium's MPRIS service implements `Position` but sets no `xesam:url` (Chromium source, `system_media_controls_linux.cc`) |
| GitHub repo, PR, file, line | All of it is in the URL (`/owner/repo/pull/22`, `/blob/main/a.swift#L10-L20`); nothing extra to read | Same |

Ruled out: macOS MediaRemote (entitlement-locked since macOS 15.4), and
Chrome `execute javascript` (needs the hidden *Allow JavaScript from Apple
Events* developer toggle).

## What other tools do

- **Scrawl** (Kit Langton): ⇧-click drops a numbered pin when the press moves
  under 12 pt and lifts within 1 s; longer presses become ink. A click after
  ink in the same ⇧ hold is part of the drawing. Window titles stay out of the
  clipboard.
- **Superwhisper**: per-mode *application*, *selected text* and *clipboard*
  context. History shows exactly what was sent to the AI.
- **Wispr Flow**: *Context Awareness* drew a backlash when users found
  active-window screenshots going to the cloud without clear notice; current
  docs describe reading text near the cursor through accessibility instead
  (secondary sources; no official desktop page found).
- **Windows Recall**: continuous snapshots; after the backlash it became
  opt-in, with app and site exclusions and private browsing excluded.
- **Granola**: audio only, deliberately no screen.

Privacy expectation from all of these: context is fine when it is *explicit*,
*visible* and *local*; it is a scandal when it is ambient and leaves the
machine unseen.

## Options

**A. Context pins (chosen).** While dictating with the overlay on, Shift+click
drops numbered pin #N where you click. It reads the window, URL, video time,
link or image under the pointer and selected text, and adds a screenshot tile
with a marker. Pins share numbering, word linking, the contact sheet and the
paste with sketches.

**B. Automatic context line on every dictation.** No gesture. Rejected: at
paste time the frontmost app *is* the agent you are dictating into, so the
line would describe the terminal. It also attaches context nobody asked for,
which is the Wispr Flow failure.

**C. A separate context mode.** Another shortcut; every window clicked into
during the recording becomes a pin. Rejected for now: a second mode to
remember, and switching back to the agent or Cmd-Tab creates pins nobody
wanted.

## How pins and drawing stay apart

Drawing is **Shift held + pointer movement with no mouse button**. A pin is a
**mouse click while Shift is held**. The overlay only exists while a dictation
records with *Draw & Pin While Dictating* on.

| You do (while dictating, Shift held) | Result | Test |
|---|---|---|
| Move ≥ 3 pt, no button | Ink; one hold = one sketch (unchanged) | `testHoldWithoutClickDrawsAsBefore` |
| Click: press and release within 1 s, travel < 12 pt | Pin #N at the press point | `testClickDropsPin` |
| Hand jitters while aiming: < 12 pt of ink, then click | The tiny ink is dropped; pin | `testJitterBeforeClickIsDroppedForPin` |
| Click mid-stroke (≥ 12 pt of ink already in this hold) | No pin; the click is part of the drawing and ink continues | `testClickMidStrokeStaysInk` |
| Press and drag ≥ 12 pt with the button held | Ink from the press point; never a text selection | `testDragWithButtonDraws` |
| Press held > 1 s without moving | Nothing | `testLongStillPressDropsNothing` |
| Double-click | One pin (the second click has `clickCount` 2) | `testDoubleClickDropsOnePin` |
| Trackpad tap-to-click | Same events as a click → pin | `testTapToClickDropsPin` |
| Ink after a pin, same hold | A new sketch with its own number | `testInkAfterPinIsNewSketch` |
| Bare Shift tap (capital letter) | Nothing | `testBareShiftTapDropsNothing` |

- A click while Shift is held never reaches the app underneath: the overlay
  takes the pointer for the whole hold.
- Without Shift, clicks pass through as before.
- Outside a dictation there is no overlay, so Shift+click text selection works
  normally.
- Why not a bare Shift tap: Koett reads modifier flags only, never key events,
  so a tap cannot be told apart from Shift+letter while typing, and a tap does
  not say *where*.

## What gets pasted

The contact sheet as before, with a tile per pin (crop around the click, red
ring marker). Then the bullets, each pin followed by its context line:

```
- #1 (0:02) “can you pull this video and summarise the part about pinning”
  Google Chrome · “How Rust async works - YouTube” · https://www.youtube.com/watch?v=abc123&t=83s
- #2 (0:09) “and look at this image here”
  Google Chrome · “Keep the Linux pill… · Pull Request #22” · https://github.com/oEdyb/koett/pull/22 · image https://…/pill.png
  selected: “one fixed 48 px row”
- #3 (0:15) “then circle this button”
```

#3 is a sketch, unchanged. An agent resolves "this video" to #1's URL and
time, and can run `koett-transcribe <url> --text`, `yt-dlp`, `gh pr view 22`
or fetch the image. Anything without a URL is in the tile.

If the YouTube time cannot be read, the link is the plain video URL.

## Privacy defaults

- Nothing is read without a click. No background capture, no automatic line.
- Everything is read locally. The only way out is the paste, and it lands in
  the input box where you see it before you send.
- Images stay in memory; history keeps the bullet text, including context lines.
- Selected text is included by default, only from the app you pinned, capped at
  500 characters. Menu › Draw on Screen › *Include Selected Text in Pins*
  flips it.
- Password fields are never read. Koett's own windows are never pinned.
- Mac needs no new permission: Accessibility and Screen Recording are already
  granted for paste and drawing.
- Known gap: private/incognito windows are not detected on Mac yet. Their URL
  is pasted like any other; you see it before sending.

## Linux

Same gesture table and paste format as the Mac; `gesture.rs` ports the Mac's
rules and their tests. Differences, all measured on niri 26.04:

- **Finding the window takes a synthetic click.** niri gives no position for
  tiled windows, and `PickWindow` ends only on a click. Koett sends one through
  `zwlr_virtual_pointer_v1` as soon as the pin is confirmed.
- **The click cannot reach an app.** A pick returns nothing where the overlay
  accepts input, and a click sent before the pick starts goes to whatever is under
  the pointer. So the click waits for proof: the pick's pointer grab sends the
  overlay a pointer leave. Only then does the overlay open a 64 px hole at the pin
  and let the click through. A real click anywhere else still lands on the overlay,
  and so does any click on the other monitor. If the proof never comes, the overlay
  stays whole and absorbs the cleanup clicks. A click is never repeated while the
  hole is open. If a pick stays unanswered, the overlay closes the hole first, then
  ends the pick, so it cannot eat the next real click. `PickGate` holds these turns.
  Right after the click a second pick starts as a shield, so a real press while the
  hole closes (the second click of a double click, in the lab) is absorbed rather
  than reaching the app; a click on the whole overlay then ends it. What remains is
  a real press landing in the milliseconds just before the click or between the two
  picks, so it reaches the window it was aimed at. In a sweep on the slow nested lab
  that hit one double-click gap in nine, and focus still came back afterwards.
- **A wrong label is worse than none.** If the pointer has left the hole by the
  time the click lands, shortly after the pin, the overlay blocks the pick: that pin
  gets its image and bullet but no context line, rather than another window's. Two
  windows with the same title give no page or selection. A pin over no window, or
  on another desktop, has no context line either.
- **Focus goes back at once.** niri focuses whatever a press lands on, even
  during a pick, with no way to skip it (`on_pointer_button`, niri 8ed0da4), and
  the paste must reach the window being dictated into. Koett reads the focused
  workspace's active window first, because niri reports no focused window while
  an exclusive overlay holds the keyboard, and refocuses right after the click.
  Measured in nested niri on a debug build under load: focus is away 16 to 17 ms
  when typing stays free, 33 to 50 ms with the focused overlay. Frames sampled
  every 50 ms showed nothing moving for a fully visible column. A partly
  off-screen column made the view shift by at least 12 px for one sampled frame,
  then return exactly. **Kept**, because the alternatives are no window context,
  or a paste into the wrong app.
- **A wrong label is worse than none.** Past 80 px of movement before the grab,
  the pick is discarded, though a pin within about 100 px of a window's edge can
  still be mislabelled by moving across it at once. Two windows with the same
  title give no page or selection. A pin over no window, or on another desktop, has an image and bullet
  but no context line.
- **Current context support:** KWin can supply read-only window geometry;
  YouTube time comes from the matched page's accessible seek slider. Other
  desktops need usable AT-SPI frame geometry. Stock niri tiled native Wayland
  browser hits still lack reliable coordinates; those omit link/image metadata.
  Apps without AT-SPI text still provide no selection.
  See [current checks and limits](linux-pin-context-verification.md). Chrome gives only app and title until its one-time setup,
  which Settings copies. That setup makes Chrome keep its accessibility tree on.
- **Fast Shift-clicks pass through.** The overlay takes the pointer only once the
  key has been held 120 ms (so typing a capital is never interrupted) and its
  input region has been committed. A click sooner than that reaches the app.

Evidence: `CrossPlatform/evidence/pins-20261103/README.md`.

## Effort

- Mac: about 2–3 days, done in this branch. YouTube time: read through Chrome's
  accessibility tree, falling back to the plain link.
- Linux: about 2–3 days. GTK overlay click, niri window at point, AT-SPI URL
  (exists), `GetAccessibleAtPoint`, MPRIS position.

## Ask

Keep it for now. Remove it after about a week of using pins. Pins give richer
context to an agent that has tools; Ask is about 1.9k lines of Swift plus the
Linux port, a pinned renderer, Cartesia and Keychain keys, and it is Koett's
only cloud path. The counter-case is hands-free spoken answers with no agent
window open.

## Open questions

1. Should private/incognito windows drop the URL (needs an AppleScript check and
   its Automation prompt), or is "you see it before sending" enough?
2. After the trial week: remove Ask entirely, or keep only its spoken replies?
