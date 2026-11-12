# Koett decisions

> Historical evidence and commands use the [0.3.1 source snapshot](https://github.com/oEdyb/koett/tree/c1e9d812d68fb0c8f035d0041a6d1da29f96c041). See [the evidence archive](evidence-archive.md) for private reproduction.

## 2026-10-26: integrate the existing native port with current Mac recovery

The Linux UI/history work branches from `origin/codex/overnight-2026-10-03`
at b6e2c73, which contains the current Mac microphone/recovery fixes. Integrate
the existing Rust port from `origin/feat/windows-linux-v0.1` at e85d0d0.
Keep Mac Sources/Tests from the newer branch during conflict resolution;
optional multilingual model changes from the draft are outside this task.
Do not merge or update the public draft PR as part of this local integration.

SQLite/FTS5 replaces the active growing Markdown archive on Linux and macOS.
The user confirmed in-app recent entries, search, copy and export, preserving
original legacy files. GTK4 supplies Linux windows; SwiftUI/AppKit supplies Mac
windows. This adds no browser runtime to dictation. See plan.md for sequencing.

## 2026-10-26: verified source versions own imported search rows

Schema v2 tracks each legacy path's verified content version and its imported IDs.
Re-importing an appended archive replaces obsolete imported search rows atomically,
while native recordings, other active source references and original bytes remain
protected. Earlier development schemas lack provenance, so their existing rows
are protected during upgrade rather than guessed away. History opens and queries
never scan the legacy archive; startup imports run in the background.

Linux GUI verification uses the private `agent-lab koett` compositor, input,
audio and portals. Host desktop checks require explicit authorization. This is a
test environment choice; the product still uses native GTK/Swift windows.

## 2026-10-29: scope the native Linux visual theme

History and Settings share a small GTK CSS provider with raw palette values mapped
to semantic roles. Scope it to Koett windows, retain native controls/fonts and
high-contrast colors, and keep the recording pill's established appearance. This
provides coherent surfaces without adding a UI framework or changing dictation.
The reference brief and inspected native captures live in docs/ui-design.md.

## 2026-11-02: the recording pill becomes a dictation timeline

Olle picked concept C from `docs/pill-concepts.html`. On Linux the pill is now
300×48 and shows the whole dictation as one strip of measured levels. Quiet
columns are dots, and sketches are numbered bands. The strip halves its
resolution each time it fills, so ten minutes still fits. While decoding, a
playhead reports finished/planned VAD segments, and the plan is published only
once known. Hover adds Cancel, Pause and History. After the dictation ends, the
pill shows Done (Copy, History) or Not pasted (Paste again, Copy, History).
The action row sits above the strip, so the bottom-anchored strip never moves.

The pill now takes pointer input while recording, so hover works, but it never
takes keyboard focus; decoding stays pointer-transparent. Pause discards input
at the capture callback, and the 10-minute limit still counts wall time.
Cancel drops the capture with no transcript, paste or history row.

Esc cancels only on X11, where the grab exists only while recording. Wayland has
no focus-free way to hear a bare Esc: a portal binding would take Esc from every
app permanently, and layer-shell keyboard focus would steal focus. Wayland
therefore offers the pill's Cancel button and `koett cancel`, which can be bound
in the compositor. Silence dots come from the level floor, not from Silero, since
the live path only runs VAD after 20 s. Draw mode feeds sketch bands through
`koett_engine::sketch`; see `docs/pill-timeline.md`.

## 2026-11-02: dictionary uses decoder hotwords, kept only where they spell a term

Three approaches were built in parallel and scored on one synthetic benchmark
(`Benchmarks/Dictionary/`): sherpa-onnx hotwords, fuzzy text correction, and
suggestions learned from corrections. Hotwords gave the large gain (31% → 73%
term recall on Parakeet v2) but a single biased stream recapitalized ordinary
words. Koett therefore decodes plain and biased streams in one batch and adopts
only biased spans that spell a dictionary term: 73% recall, 0 false terms, 0
broken near-miss sentences, about twice the decode time while a dictionary exists.
One-edit fuzzy matching was dropped: +2 points recall for 3 false rewrites
("parakeets" → "Parakeet"). Exact spelling normalization and user sounds-like
forms stay. An empty dictionary keeps the old greedy path. Details and the Mac
plan: `docs/dictionary.md`.

## 2026-11-02: draw while dictating uses layer-shell, screen stills and one contact sheet

Olle asked for Scrawl's sketch-and-talk behaviour and chose its paste format: one
numbered contact-sheet PNG, then `- #N (M:SS) “words”` text. Linux uses one
layer-shell overlay per monitor during dictation (off by default). Wayland has no
global key state, so the overlay holds exclusive keyboard focus while recording
to see the draw key, and releases it before paste. KWin reported stale modifier
state on pointer events, so drawing follows the overlay's own key events.
Stills, not video: each closed sketch captures its monitor with the ink visible,
first through `grim` (wlr-screencopy: niri, Sway, Hyprland; Olle's niri disables
the Screenshot portal) and otherwise through the non-interactive Screenshot portal
(KDE asks once). Parakeet token timestamps link words to sketches. Image and text
are pasted sequentially, because a single clipboard offer with both types makes
each app choose only one of them. GNOME and X11 lack layer-shell and keep normal
dictation. Details, limits and the Mac to-do: docs/draw-while-dictating.md.

## 2026-11-02: bare Shift via keyboard state, one hold per sketch

Olle chose maximum UX over the compositor-shortcut option: bare Shift, each
hold one separate sketch (release ends it), many sketches per dictation, and
typing never blocked. Wayland clients cannot see Shift without focus, so the
opt-in "Keep typing" mode samples held keys with EVIOCGKEY while a drawing
dictation records and uses only the modifier bits (the kernel has no
modifier-only query; samples are wiped, nothing is stored, key events are never
read). Access comes from a one-time setup Settings copies: a `koett-keyboard`
group with a read-only udev ACL on keyboards (review preferred this over
`uaccess`, which grants read and write). Without access, the focused overlay
remains the fallback and Koett says so. Keyboards are rescanned during a
recording for hot-plug. Read access to keyboards for the user's apps is the
cost, which is why it is opt-in, not default.

## 2026-11-02: Mac draw mode captures without its own windows

The Mac port keeps the Linux output (sheet, bullets, numbering, scope) and the
linker is a direct Swift port proven against Linux-generated golden cases. One
mechanism differs: ScreenCaptureKit can exclude Koett's own windows, so each
sketch is captured from the screen without the overlay and its own strokes are
drawn on afterwards. Linux had to hide other ink and wait for a presented frame;
on Mac a capture cannot contain another sketch's ink or the pill by
construction. The draw key comes from modifier-flag monitors, so typing is
never blocked and no keyboard access setup is needed. Details:
docs/draw-while-dictating.md.

## 2026-11-02: Mac permissions window instead of a failed setup

Modelled on Scrawl's permission step (behaviour only). Koett shows one small
floating window with Microphone, Accessibility and Screen Recording, a short
purpose, a live status and one button per row: Allow (system prompt, first
time) or Open Settings (the exact `x-apple.systempreferences:` privacy pane).
It opens on first launch when anything is missing, at any launch while
Microphone or Accessibility is missing (setup then waits and continues by
itself once both are allowed, instead of failing with Retry Setup), when Draw
While Dictating is turned on or a sketch capture fails without Screen
Recording, after an Ask screen-access failure, and from the menu
(Permissions…). Status is polled once a second while the window is open;
Screen Recording also gets a real ScreenCaptureKit probe after Koett has asked,
because its preflight can stay stale, and offers Relaunch. Input Monitoring is
not listed: the shortcut tap and draw-key monitors run on Accessibility.
Renders: docs/evidence/mac-permissions-20261102/.

## 2026-11-02: the pill stays one fixed 300×48 row

After trying the timeline pill on Mac, Olle said the finished state looked weird,
the pill lingered after pasting and grew taller, and the strip got longer and then
jumped when it compressed. Every state is now one 300×48 row. Done is the old
result row with Copy on the right; it morphs in place from the decoding pill and
fades after 1.75 s unless hovered (Mac timings). Not pasted stays until Copy or the next
dictation. Hover actions replace the strip inside the row. The strip is full width
from the first frame and, after 16 s, resamples the whole dictation every frame, so
it compresses smoothly. Paste again and the hover model name are gone; the tray
keeps Paste last.

## 2026-11-02: pill UI rules are a cross-platform contract

The Mac pill at `e0ca1b9` passed its live test, so its rules are now the
contract for both platforms: one fixed 300×48 row, a rolling full-width strip,
in-place morphs, a short Done with Copy on the right, rescue states that wait,
and off-screen renders at 0:05, 1:00, 5:00 and 20:00 plus morph frames before
any merge. See "Pill and overlay UI rules" in `docs/ui-design.md`. The Linux
strip still resamples the whole dictation and should move to the rolling window.

## 2026-11-03: Linux pill follows the Mac pill exactly

Olle preferred the Mac strip ("the waveform slides to the left constantly, even
when you're not talking") and the Mac design overall. Linux replaces the
whole-dictation strip from #22 with the Mac rolling window, adds Paste to Not
pasted, shows `NN%` instead of a decode playhead while transcribing, allows
hover Cancel/History while transcribing, and uses the Mac close timings (Copy
1 s even while hovered, 0.8 s after the pointer leaves). The Mac pill
(`344d11f`) is the reference; where Linux differs, Linux changes.

## 2026-11-03: a recording never takes the keyboard by default

Olle: "when I'm recording my voice with coet, it turns off my keyboard... I
can't type", and the standing rule "maximum UX": he talks and types at once.
With drawing on and the old opt-in device mode off, the overlay held exclusive
focus for every recording. Turning drawing on is now the opt-in for sampling
held keys, which replaces the separate "Keep typing" toggle (supersedes that
part of the 2026-11-02 decision). Order: devices when readable; focused overlay
only if the user ticks the new last-resort option; otherwise drawing stays off
with a one-line hint. No privileges are added: without existing device access
nothing is read until the user runs the setup command Settings copies.

## 2026-11-04: Linux pastes automatically only where it can check the target

Olle's decision: a wrong-app paste is worse than a clipboard-only result. Niri and
X11 report the focused window, so they keep automatic paste with a focus check.
KDE, GNOME and other Wayland desktops can't, so they copy and show Not pasted, and
pressing Paste sends the text to the app focused at that moment.

## 2026-11-04: autostart goes through the launcher

The autostart entry pointed at the raw binary, which skipped the patched GTK and
the runtime paths. It now uses `$PREFIX/bin/koett`, and launching a fixed version
repairs a stale entry. No re-exec guard: a stale entry runs the old binary, so new
code in it would never run.


## 2026-11-04: Mac recording starts only from a fresh ready-state tap

A tap during transcription is ignored. A queued start could turn the microphone
on several seconds later, after the user had moved on. Removing the queue keeps
one tap tied to an immediate action. The existing shortcut event tap also observes
mouse presses, so Option-click and Option-drag do not masquerade as plain taps;
this uses the app's existing Accessibility grant.

## 2026-11-04: drawing listens only to physical keyboards

A uinput device, grabbed or not, and a mouse with a keyboard map can't start
drawing. The cost: remapper daemons that present a virtual keyboard (keyd, kanata)
no longer trigger drawing through that device, though a physical keyboard they grab
still reports its held keys. Olle's rule is that drawing follows a real Shift hold,
and a test or tool must never take his pointer. The only exception is one device
named by `KOETT_TEST_DRAW_KEYBOARD`, for private labs.

## 2026-11-04: Linux pins find the window with a synthetic click

niri gives no position for tiled windows, so the only exact answer to "which
window is under the pointer" is `PickWindow`, which ends on a click. Koett sends
that click itself, gated so it can never reach an app, and refocuses straight
after, because niri focuses what a press lands on. Measured flicker: focus away
for 16 to 17 ms, nothing moving on a fully visible column, one short shift on a
partly off-screen one. Kept over the alternatives, which are no window context
or a paste into the wrong app. PRIMARY was rejected as a selection source because
nothing proves its owner, so a token selected in a terminal could be pasted under
a browser pin. Details: docs/screen-context-proposal.md.

## 2026-11-05: the pill's drag moves a copy, not the pill

On Linux, the real pill stays in place, invisible at 1/255 opacity, and keeps the pointer grab
while an input-free live copy follows the pointer onto any monitor. On release the real
pill moves there. This gives one code path for every compositor and exact pointer offsets.
The cost is a second window for the length of a drag.

On Mac, AppKit tracks the drag while the nonactivating panel moves directly.
Dragging its background keeps the existing appearance and button hit targets.
Position is saved relative to the display's visible frame, keyed by display ID;
rearranging or resizing a display keeps the pill on it. A disconnected display
falls back to the first available screen without erasing the saved position.

## 2026-11-05: Esc ends draw lock instead of cancelling

While draw lock is on, Esc captures the sketch and leaves the lock, and a second Esc
cancels. In a mode the user entered on purpose, Esc reads as "leave this mode", and
throwing away the whole dictation is the costliest wrong guess. Ctrl+Z also reaches the
focused app, since typing must keep working and sampled keys can't be consumed, so
right-click is the undo that touches nothing else.

## 2026-11-05: Linux cues are synthesized and can be turned off

Linux plays a high tick when capture starts and a low tone when it stops, at the same
moments as the Mac, and stays silent on cancel. The Mac uses macOS system sounds, which
Koett can not ship, so the Linux cues are generated in code with no audio files. Unlike
the Mac there is a Settings switch, on by default, because Olle films with Koett and a
sound in a take should be his choice. In a lab where the microphone hears the speaker
directly, the transcript was identical with the cues on and off.

## 2026-11-05: recordings stop at a limit you choose, 60 minutes by default

Olle asked why there was a 10-minute limit; there was no reason beyond bounding memory.
Settings offers 10, 30, 60 and 120 minutes, and a value outside those in a settings file
becomes 60. The pill says "1 minute left" once, and the stop still saves and pastes. The
whole recording stays in memory in case decoding fails. An 11-minute lab run grew by 192 KB/s
at 48 kHz, so that is about 690 MB at 60 minutes and 1.4 GB at 120, only while recording. Live decoding drops audio as it goes, so
its memory doesn't grow with length.

## 2026-11-06: GNOME drawing switches input in the compositor

GNOME has no layer-shell, so Koett's extension turns a transparent GTK window into a dock over
each monitor. GTK's input region failed there in the lab: presses went to the old surface, even
with extra frames. The extension now switches whether Mutter can hit the window, and hides and
shows its actor in one tick so pointer focus follows. Mutter resets that switch with each commit,
so the extension sets it back. The cost is a dependency on Mutter internals that a future Shell
may change, and it only runs on Shell versions with the extension installed.

## 2026-11-06: X11 overlays are override-redirect

The pill and drawing surfaces on X11 bypass the window manager. Marking them as notifications
didn't hold: GTK rewrites the no-focus hint when it shows a window, and KWin then gave the overlay
focus. Override-redirect windows are never focused or decorated. The cost is that a window manager
can raise something above them, so the overlay raises itself at each draw key press. They're shown
without `present()`, which asks for focus.


## 2026-11-06: Linux updates replace the launcher, not the files

Linux installs are already versioned, so an update unpacks the signed archive and runs the same
`install.py` a person runs, which swaps `bin/koett` atomically. That left no new installer to
trust and gives rollback for free: the old version stays on disk and only the link moves. The feed
is a small signed JSON file rather than an appcast because nothing on Linux would parse the XML.
Settings are compiled in, with no environment or file override, so a user's environment can't point
Koett at another feed, and a build missing any setting is inert. Ed25519 comes from `ring`, already
in the tree through rustls, so only the direct dependency is new. The idle check takes a lock
that starting work also takes, because checking then starting is a race an install could slip into.

## 2026-11-06: CI is manual only

Olle, asked after 76 overnight runs for 35 PRs, chose: "No automatic runs on PRs or main. Workers
fire one run themselves right before merging a batch, and before a release." That replaced a
gate merged hours earlier that skipped main runs a PR had already covered, which never got to prove itself.
The cost is that nothing enforces testing: GitHub shows no check on a PR and won't block a merge,
so main can go red if the procedure in AGENTS.md is skipped. A run also tests the branch, not the
merge, which is why the branch goes onto current main first.

Kept from that work: cache keys without the commit, which had taken caches to 14 GB because each
main run saved another 5.5 GB; restore prefixes by architecture, because `ubuntu-22.04-` also
matched arm's caches; and cached .deb files, because slow apt steps were a crawling Azure mirror,
not dpkg: usually 18 to 45 s, sometimes 7 minutes. Dependency review compares a branch with main
by ref, as it no longer has a PR event.

## 2026-11-06: recording sounds are off by default, with a volume

This answers the question left open by #88. Olle, testing a build: "I don't think you normally need
a sound when you have such like a obvious pill", and of the start cue, "it's really high-pitched."
The start went from 2093 Hz to a soft rise from 494 to 659 Hz; the numbers are measured but only his
ear can judge it. `recording_sounds_volume` is a percentage, 1 to 100, default 50, squared into a
gain so the slider feels even; 100 is the old cue level at most. A saved `recording_sounds: true`
with no volume beside it counts as off, because it predates this choice and no release shipped it.
Windows shares the settings and tones but its popup menu can't hold a slider, so it offers 25, 50,
75 and 100.

## 2026-11-06: auto-update is on for Linux archive installs

Olle, asked whether to turn it on for 0.3.1: "Yeah, do it." CI builds now carry the feed and the
public key. Native packages share that binary, and their layout keeps the updater off, rather than a
second build with its own flag, so what's tested is what ships. The build number is a plain rising
integer in the workflow, 4 for 0.3.1, matching the Mac. The signing key stays on the machine that
signs and never reaches CI, so a stolen CI token can't sign an update.

## 2026-11-07: a paste that didn't happen but copied fades on its own

Olle: "it doesn't have to stay up even after you already pasted it." On Linux and Windows, a
text-only result that wasn't pasted now checks the clipboard by reading it back, copying again if
needed. If the text is there, the pill says `Copied to clipboard` once and fades like Done. The sticky
`Not pasted` with Paste and Copy is only for a clipboard that failed too, a history that failed, or a
result with a drawing, where copying the text would replace the image. On KDE, GNOME and other
desktops without a focus check, every dictation used to end in the sticky pill; now it ends with the
text on the clipboard and the pill gone, and Paste last transcript stays in the tray. A script asking
`koett paste-last` still hears that nothing was pasted. The Mac got the same in #178,
which also keeps drawings and never overwrites a newer copy; Linux and Windows copy the text again
if it's missing.

## 2026-11-07: the pill's hover shows only ✕, and Pause is gone

Olle didn't like Cancel, Pause and History covering the waveform on hover: "there must be some
more intuitive and better UI way to do it." Grilled answers, all yes: the waveform never hides,
the timer turns into ✕ (also while transcribing), one click cancels with no confirm and the pill
says Cancelled and fades, History lives in the tray and window only, and Pause goes from every
visible place: "no one will ever press pause, bro." Linux and Windows match the Mac (#183). The
`pause`/`resume` commands went too. Windows app-CLI codes keep their numbers so an older running
Koett never reads a newer command as something else. The audio pause internals stay with no way in.

## 2026-11-12: clipboard managers skip the drawing's paste offer

After a manager-like client took the clipboard just before a drawing's key (see bugs.md), the
drawing's short-lived paste offer now carries the standard skip markers: `x-kde-passwordManagerHint`
`secret` on Linux (CopyQ and Klipper skip it; cliphist's `wl-paste --watch` still reads the image and
cliphist drops it afterwards; persisters like wl-clip-persist ignore the hint), `org.nspasteboard.TransientType` on the
Mac, and `ExcludeClipboardContentFromMonitorProcessing` with `CanIncludeInClipboardHistory` 0 on
Windows. Only that offer: the text, the drawing kept on the clipboard with its text afterwards, and
Copy stay in clipboard history as before, and Koett's own History doesn't use the clipboard. When
Koett loses the clipboard on Linux, the log names the new owner's types (no contents), since the
data-control protocol doesn't say which client it was.
