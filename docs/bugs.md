# Koett fixes

## 2026-10-26: Linux native ONNX environment abort

The first ASR-only source build compiled and linked, then aborted in
`OrtApis::CreateEnv` with `free(): invalid pointer`. GDB located it in the
ONNX execution-provider discovery regex, before model decoding. `nm -C` showed
that the pinned prebuilt ONNX Runtime archive instantiated the original
libstdc++ regex ABI, while sherpa instantiated `std::__cxx11::regex_traits`.
Building sherpa and its C++ dependencies with `_GLIBCXX_USE_CXX11_ABI=0` on
Linux matches the prebuilt archive and removes the mixed-ABI regex collision.
See [GCC PR118408](https://gcc.gnu.org/bugzilla/show_bug.cgi?id=118408) and
[GCC's ABI guidance](https://gcc.gnu.org/onlinedocs/libstdc++/manual/using_dual_abi.html).

Verification: Ubuntu 22.04/GCC11 source build and current CachyOS runtime both
pass the official 7.435-second model sample. The actual Koett long-input probe
also passes 420 seconds at 44.1 kHz with its repeated anchors and isolated short
utterance. Keep the model self-test in build gates; compilation cannot detect
this failure. No model or recognizer configuration change was required.

## 2026-10-26: history queries repeatedly scanned the legacy archive

An initial implementation imported from `open_default`, so every search/get
recomputed the old archive's content hash. SQLite-only open and explicit
background startup import keep queries independent of legacy file size.
Regression tests verify that opening/querying does not read the old archive.
Unparseable valid UTF-8 archive chunks remain searchable with unknown dates,
while exact source bytes remain available and unchanged.

## 2026-10-26: appended legacy archives duplicated earlier search rows

Content hashes alone could not supersede the earlier searchable representation
when a file grew. Source/version ownership and an atomic verified import now
reconcile obsolete imported-only rows. Real SQLite regressions cover append,
retry, interruption, other sources and native ID protection. Independent review
found no remaining high-confidence issues in that bounded storage scope.

## 2026-10-26: portal paste permission lacked the application name

Shortcut registration and paste used separate D-Bus connections. Only the former
registered Koett's identity, so KDE called the paste requester unidentified.
Registering the same app ID on the paste connection makes the private KDE prompt
name Koett. The portal clipboard owns CLIPBOARD only, so its paste chord is Ctrl+V;
terminals requiring another chord retain manual-paste recovery.

## 2026-10-26: inventory license order changed between runs

The vendored package and repository both have a file named LICENSE. Sorting a set
by filename alone left ties dependent on Python's hash seed. Sorting emitted file
records by name and content hash gives deterministic inventory output, verified
with multiple hash seeds.

## 2026-10-26: Linux recording feedback and Settings presentation

The Linux candidate omitted the contracted nonactivating recording pill and could
leave an existing Settings dialog behind another window. Optional GTK layer-shell
feedback now shows live levels/elapsed time above applications without accepting
keyboard or pointer input; Transcribing remains visible until completion. Missing
layer-shell support retains in-window feedback rather than pretending to provide
an always-on-top window.

Settings reuses its dialog and forwards the user's Wayland activation token to
that window. GTK can remove the token from the environment before Rust main;
reading GDK's cached startup token fixes that case. Recording commands retain
plain IPC bytes and never carry activation authority. Aggregate interactive-window
focus protects paste while either History or Settings is active.

Private KWin and exact Niri 26.04 proofs cover focus-preserving capture, typing,
levels, transcription feedback and Settings activation. The existing Settings
window foregrounds through an actual private Niri keybinding; merely spawning
from IPC was insufficient activation evidence. GTK4.6 fallback, tests, lint and
baseline release build pass. See the follow-up in the Linux verification report.

Read-only inspection of recent user history confirms malformed words and uneven
punctuation, not just a language mismatch (the user confirmed English). Successful
recording audio was not retained, so intended words and root cause cannot be
reconstructed. An isolated larger English v2 trial establishes compatibility and
higher RAM/CPU cost, not improved personal accuracy. The selected model remains
unchanged. No private transcript text belongs in repository evidence.

## 2026-10-27: selected ALSA input disappeared while microphone stayed present

Koett reported `Microphone unavailable: alsa:usbstream:CARD=Xstrm` while the
Xstrm device and EasyEffects source remained present and Klippt captured from
EasyEffects. This proves a selected-input resolution failure, not exclusive
microphone contention. Restored the prior system-default selection with a private
settings backup; all other settings remain unchanged. Runtime reloads input
settings before each capture. No physical capture test was performed; user retry
is still needed to confirm recovery. Klippt was left running.

## 2026-10-27: paste chord changed with Linux output backend

Portal output owned CLIPBOARD and sent Ctrl+V, while external/X11 output used
Shift+Insert and PRIMARY. A backend change could therefore change terminal/GUI
behavior. A validated explicit Ctrl+V/Ctrl+Shift+V preference now drives every
Linux backend. PRIMARY is preserved, target guards and independent history remain.
Private real GTK/kitty tests cover exact text, typing, Enter and clipboard-only
recovery after focus changes. This does not infer which shortcut arbitrary apps
accept; Settings exposes the choice. See the verification report for limits.

## 2026-10-27: guarded X11 delivery hid clipboard-only recovery

The focus guard correctly skipped automatic paste after the target disappeared
or changed, but returned success. It now reports that clipboard-only outcome to
the typed recovery path, preserving the guard and saved text while exposing Copy.
Regression coverage checks matching, changed and missing targets and the emitted
recovery update. Settings validation evidence now checks the exact empty Phrase
entry receives focus rather than accepting any focused editable field.

## 2026-10-27: thinking pauses incorrectly produced an input warning

The new five-second silence warning treated normal dictation pauses as a problem.
Removed silence from capture feedback for both Settings preview and the recording
pill. Low amplitude does not establish microphone failure. Real device/stream
errors remain visible, and sustained excessive input retains its separate warning.
Regression coverage verifies even ten minutes of silence produces no warning.

## 2026-10-28: upstream recording-meter audio fix integrated

Upstream `1f04616` moved macOS duration and power reads out of the microphone
file sink mutex into atomics. The former meter reader could collide with the
real-time callback's nonblocking lock acquisition and drop audio. Its regression
test and source fix are merged here together with `origin/main` at `10a2a76`.

The Linux path already uses atomic RMS/peak reads, a ring-buffer capture callback
and a UI-local elapsed clock. Independent source inspection found no equivalent
meter lock contention; no speculative Rust patch was added. This finding does
not rule out unrelated capture failures. Swift execution remains unverified on
this Linux workstation; run the upstream regression on macOS before release.

## 2026-10-28: shortcut warnings replaced the recording phase

X11 shortcut registration/validation failures used the same Error phase as an
engine failure. During capture or transcription, a shortcut conflict could make
the UI display an enabled Retry rather than Stop or disabled Transcribing.
Shortcut failures now use a nonfatal notice, preserving the engine phase and
the shared tray/window action mapping. Regression coverage exercises unavailable
and invalid shortcuts during both capture and ASR. Internal UI control messages
also no longer leak into the tray's displayed phase.

## 2026-11-02: dictionary adoption rewrote neighbouring words

Codex review of PR #8. Adoption took a whole differing biased span if it
contained any term, so "Send Superbase invoices" became "Send Supabase receipts",
and hotword-boosted homophones turned "the cloud" into "Claude". The spelling
pass also matched `C++` by its letters alone, writing `C++++`, and missed
decomposed "Göteborg"; imports with 50,000 forms took 1.8 s on the UI thread.
Adoption now requires the biased span to be exactly one term, places it over
the closest plain words only, and keeps plain words that are all common English.
Punctuated terms match literally, keys are NFC, and imports are capped with set
deduplication. The benchmark gained a lost-words metric; it fell from 6 to 1.

## 2026-11-02: second dictionary review (Codex, main at 6ffd62f)

Adoption panicked on a punctuation-only biased span ("Hello , world"), doubled
term symbols ("Use G++." → "Use C++++."), missed multi-word terms whose last
word matched alone ("Old Dyberg"), and could turn everyday Swedish into a term
("Jag fikar idag" → "Figma"). Literal terms missed decomposed text. Fix a word
wrote Corrections.json 0644, stripped term symbols while learning ("c sharp" →
"C"), compared case-sensitively ("ask Grok" → "Ask Groq"), kept corrections
and dismissals without bound, relearned on every refresh, and dropped focus
after Add or Dismiss. All fixed with a regression test each; a Swedish
common-word list and an inflection guard (parakeets ≠ Parakeet) were added.

## 2026-11-03: drawing never started on a second monitor (niri)

Olle held Shift on his niri desktop and nothing drew. The focused overlay asked
for exclusive keyboard focus only on the first monitor GDK listed (DP-1), but
niri grants exclusive layer-shell focus only on its active output (DP-3, where
he worked), so the overlay never saw Shift. The private lab had one output.
Every monitor's overlay now asks for exclusive focus and listens for the key;
release and the device-mode fallback apply to all of them. Regression test:
`focused_mode_asks_for_keyboard_focus_on_every_monitor`. Device mode ("Keep
typing") did not depend on focus and was unaffected.

## 2026-11-03: recording blocked typing

With drawing on, the overlay held exclusive keyboard focus for the whole
recording, and #27 extended that to every monitor, so nothing could be typed on
Olle's niri desktop while dictating. Focus is now used only as an explicit opt-in;
by default the draw key comes from keyboard devices, or drawing stays off with a
hint. Losing every keyboard mid-recording no longer takes focus either. Regression
test: `a_recording_never_takes_the_keyboard_unless_opted_in`.

## 2026-11-04: Linux demo hardening

A lab run pointed `KOETT_MODEL_DIR` at Olle's real v2 folder, and Koett quarantined
it and installed 110M there; it was restored after a hash check. Labs must copy
models first. Fixed in the app: the pill's Paste only copied; no microphone with the
window closed showed nothing; the 10-minute stop gave no reason and a late stop
re-decoded everything; a double-tap on Stop started a new recording; a slow live
decoder raised a false overflow warning; a press before ready did nothing; S1-mini
could hold a paste for 180 s. Each has a regression test.

## 2026-11-04: drawing started with no Shift press

Olle: "it presses shifts by itself sometimes and then my mouse just goes off".
Since #35 Koett read Shift from every device reporting the key, and `EVIOCGKEY`
sees a device even while another program grabs it. `lab-grabbed-shift.py`, run on
the host kernel by a test on the same PC, held a virtual Shift that his desktop
never saw and his running Koett obeyed: it drew and took clicks. A read-only sampler
logged that device going down with no keyboard press, and his unwanted pasted
sketches fall inside those runs. The installed build reproduced it in a private lab.
Only physical keyboards count now, which also drops `ydotoold` and his Razer mouse's
keyboard map. A hold needs the key seen up first, so releasing Ctrl from
Ctrl+Shift+Space or a key stuck at start can't draw; other keys end a hold, and a
focused hold idle for 15 s is released. Regression tests are in `sketch/keys.rs`.
Labs must name their device in `KOETT_TEST_DRAW_KEYBOARD`.

## 2026-11-05: the draggable pill didn't drag

#79 shipped on a build check and did nothing for Olle: a ghost followed the pointer, the
pill stayed at the bottom, and the drop never landed. It used Wayland drag-and-drop. The
first margin-based rewrite stuck at the monitor edge, then failed on KWin. Three causes,
all measured: niri keeps the press's surface origin for the whole grab while KWin follows
the surface a variable time later; a layer surface can't change output without a remap,
which ends the grab; and a window with nothing to draw gets no render node, so GTK skips
the frame and the old pill stays on screen. The earlier lab passed because it never
compared the pill to the pointer. Labs now assert one recording dot within 3 px of the
pointer in a screenshot at each step, on niri and on two KWin outputs.

## 2026-11-05: a mistyped koett command looked like a successful one

Found while adding `transcribe-file`. Any CLI error outside a hard-coded list opened a
desktop dialog saying Koett could not start and exited 0, so a script saw success. The
media commands now print a JSON error and exit 1. `start`, `stop` and `toggle` keep the
dialog on purpose, since a desktop keybinding has nowhere else to show it.

## 2026-11-05: auto-paste stopped after 4096 windows until a restart

Found in the audit of the Linux parity PRs. The KWin and GNOME focus checks remembered
every window and refused new ones once 4096 were known, so an autostarted Koett ended up
showing Not pasted for every new window, with no reason given. Tokens now come from a
counter that never reuses a value, and the oldest identity is forgotten, so memory stays
bounded and two windows still can not look like the same target. Also from the audit: a
display change dropped a pin whose image was taken while its context was still being read,
under a notice saying captured images are kept, and the X11 Esc grab retried on every loop
when it failed.

## 2026-11-05: an emptied dictation wiped the clipboard, and X11 pasted with no focus

Found in the audit of the Linux parity PRs. A replacement rule that removes a whole
dictation got past the empty check, which looked at the text before replacements, so
Koett cleared the clipboard, pressed paste and saved a blank history row. On X11 the
focus guard compared raw focus values, so None and PointerRoot matched themselves and
Ctrl+V went to whatever was under the pointer. Both reproduced on the old build in a
private lab with real speech. Regression tests are in `linux.rs`.

## 2026-11-07: Koett said it pasted when nothing was pasted

Olle, in T3 Code on niri: clicking a thread in the sidebar left the caret blinking in the composer,
yet the dictation never appeared, until he clicked the composer and dictated again. His log shows
both deliveries the same way: focus check passed on the same window, Ctrl+V posted, outcome ok.
Koett counted sending the keys as pasting. So he got neither the text nor `Copied to clipboard`.

The drop itself didn't reproduce. In a private lab with a nested niri, Electron 43 took Koett's
Ctrl+V after a direct click, after a sidebar click that moves focus by script, and with script focus
and no click at all. So the virtual keyboard's timing and keymap aren't a general cause. What T3 Code
does with a paste after a thread switch is still open.

Every real Wayland paste reads the clipboard from its owner, so Koett now serves the text itself
and only reports a paste when a read starts after the key, within 1.5 s; the portal path counts its
transfers the same way. Reads before the key, such as CopyQ's, don't count. With no read, the pill
says `Copied to clipboard`. The log now records `target_read` or `no_clipboard_read`, so the next
failure in T3 Code shows whether it ever asked for the text. A target that reads and then discards
the text is still reported as pasted. X11 has no such check yet.

## 2026-11-07: pressing Copy pushed its label out of the pill

Olle: "the copy goes like outside the pill window". On Linux, Copy on a sticky `Not pasted` pill
relabelled its button `Copied`, and the wider button grew past the pill's right edge. Copy now
morphs the whole pill into `Copied to clipboard` and fades it like Done, even under the pointer;
a failed copy says `Copy failed · saved` in the row and keeps the buttons. The first fix showed
`Copy failed` instead: the failed delivery's five-second error row outranks Done, so Copy now
clears it. Windows already had a `Copied` phase from #179 and now uses it for Copy too.

## 2026-11-07: Koett refused to paste where Olle stopped

Olle: "i stopped it, it dodnt paste, but when i clicked ctrl v right after it poasted". He started
dictating in Chrome, drew mark #1 two seconds in, moved to T3 Code and stopped. Koett had captured
the paste target when recording started, so the focus guard compared T3 Code (111) with Chrome (117)
and refused. The drawing made the result an image, and images kept the old sticky rescue, even though
the drawing and text were on the clipboard: his own Ctrl+V pasted them.

Olle chose both rules: the paste goes to the window focused at stop, and the guard covers only stop
to paste, so focus that moves while transcribing still refuses. A drawing that can't paste now says
`Copied to clipboard` and fades like text, but only when Koett's offer was confirmed as the clipboard
owner and its image and exact text read back; otherwise the rescue stays. Both platforms re-capture
in the one function every stop passes through, so the tray, CLI, shortcut and hold release agree.
A recording that ends without a stop command, such as the time limit, still uses the start target.
Linux X11 drawings keep the sticky rescue, since that path never offered the drawing back.

## 2026-11-08: T3 Code kept the drawing but lost the spoken text

Olle, dictating with a drawing into T3 Code: the image always landed, the text sometimes didn't.
His log showed both pastes going through, and T3 Code reading the text 5 ms after its key, both times.
Koett pasted the drawing first and the text about 190 ms later. T3 Code's composer resets its
editor to its draft whenever they differ on a re-render, and attaching an image re-renders once its
async preparation ends. T3 re-encodes images over 2000 px, so a big contact sheet takes longest;
when that finished after the text was in the editor but before the text reached the draft, the
text was wiped. His failing drawing was 927 KB, the one that worked 242 KB.

A private lab ran T3 Code's own paste rules and image module in a stand-in composer under CPU load:
image first lost the text 5 times in 6 with a 2560 px drawing, never with 1200 px. Text first
landed both 6 times in 6 under the same load. Pasting both as one item can't work in T3 Code: a paste
with an image file attaches the image and drops the text. So Linux pasted the text, waited 300 ms
after the target read it, then pasted the drawing. Superseded on 2026-11-11: text first split the
message when Enter came right after the text; see below.

## 2026-11-08: a drawing that failed after its text offered to paste the text again

Found in the review of the Windows port of text-first (#190). Once the text pastes first, a drawing
that then fails has a partial result. Linux reported it like nothing had pasted: `Copied to clipboard`
when the drawing read back, otherwise the sticky rescue, whose Paste pastes the text a second time.
A full paste whose clipboard offer then failed also fell to the rescue. Now the pill says
`Text pasted · drawing copied` or `Text pasted · drawing failed` and fades, with no Paste, and a full
paste is Pasted whatever the offer does. On Wayland a text the target never reads already stopped
before the drawing, so the drawing can't replace unread text; X11 still can't tell. A cancel can't
land mid-paste on Linux, since the paste runs in the loop that handles it and the engine ignores a
Cancel once it's Ready. Lab: nested niri, a real drawing through draw-lock, Electron targets.

## 2026-11-11: after a drawing, Ctrl+V gave the drawing without the text

After a drawing dictation into T3 Code fully pasted, Koett put the drawing and its text back on the
clipboard as one item. T3 Code drops the text from any paste that carries an image, so pasting the
text again gave only the image. #194 then kept the text alone after a full paste. Reverted the same
day: the real problem was the message being sent between the text and the drawing; see below.

## 2026-11-11: the drawing went into the next message, then was gone from the clipboard

Olle: "now the image doesn't get pasted again ... it's not in my clipboard afterwards either." His log
and T3 Code's own message records (timestamps and attachment counts only) showed T3 did insert the
drawing every time, in the next message. With text first, Koett pasted the drawing about 0.4 s
after the text, and the message was sent about 0.5 s after the text showed, 45 to 112 ms after the
drawing's key, carrying the text and no image. Koett sends no Enter and T3 Code 0.1.8 has no
auto-send, so this is an Enter pressed as soon as the text appeared. #194 then left only the text on
the clipboard, so the drawing couldn't be pasted back.

Now the drawing goes first, the text follows 1 s after the target reads the drawing, and the drawing
stays on the clipboard with its text after every drawing paste, on the Mac, Linux and Windows. The
second 1 s keeps the #189 wipe away: T3 Code finished attaching a 2560 px image in at most 231 ms
under heavy CPU load in the lab. An unread drawing no longer counts as pasted on Linux, so a text
field or terminal says `Text pasted · drawing copied`; a drawing that landed without its text says
`Drawing pasted · text copied` or `text failed`, and neither offers Paste. Lab: nested niri, a real
drawing, an Electron composer whose Enter sends a message. An Enter 0.42 s after the text sent the
text without the image on the old build and both together on the new one.

## 2026-11-12: another app took the clipboard just before the drawing's key

On Linux a drawing dictation pasted only its text, and Ctrl+V gave the drawing afterwards. The log
showed Koett's drawing offer replaced by another client's selection 70 ms before Ctrl+V, so the
target read that instead and the drawing never landed. Which client did it on that machine is
unknown: CopyQ with the same settings didn't do it in the lab, and the log has it once in 56
deliveries. Koett also stopped its old offer before making the new one, leaving the clipboard empty
for a moment, which is when a clipboard manager puts its last item back.

Now a live offer switches in place with no empty moment, and each offer tracks from the protocol's
own events whether it's still the selection. Right before each key, if another app took the
clipboard, Koett reads it back. The same bytes (a manager like wl-clip-persist re-serving every
copy) still get the key, though the paste can't be confirmed. Different bytes are a newer copy and
win, as on the Mac: no key for the drawing or its text, and the pill keeps Paste and Copy. Lab:
nested niri, a real drawing, and a watcher that copies a stale item the moment the drawing is
offered. The old build pasted the stale item 3 times in 3 and the drawing never. The new one
pastes nothing and leaves the newer copy on the clipboard. Plain dictation still pastes once behind
a manager that re-serves the same bytes. Windows already checks it still owns the clipboard right
before the key; the Mac now does too.
