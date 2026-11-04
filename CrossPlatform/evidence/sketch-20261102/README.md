# Draw while dictating — private KWin lab evidence, 2026-11-02

Private `agent-lab koett` (KWin 6.8.1, one 1920×1080 virtual output), release
build of feat/draw-while-dictating, Parakeet v2 int8, isolated XDG dirs and audio.
Speech: the model's `test_wavs/0.wav` played into the lab's virtual microphone.
Target: Google Chrome with `paste-target.html` (logs each paste's MIME types).
Shift was held through KDE's fake-input protocol with `lab-gesture.c`.

1. `01-ink-while-dictating.png`: Shift held, red ink over the Chrome tab.
2. `02-one-sketch-pasted-in-chrome.png`: one paste `image/png` 1600×906 (numbered
   sheet), then `text/plain` `- #1 (0:03) “…whole sentence pair…”`.
3. `03-two-sketches-linked-per-sentence.png`: sketches at 0.8 s and 5.05 s of
   audio; sheet 1600×457 with chips 1 and 2; text
   `- #1 (0:02) “Well, … her eyes.”` / `- #2 (0:06) “It is certainly very like the old portrait.”`.
4. `04-undo-leaves-plain-dictation.png`: sketch removed with Ctrl+Z; only plain
   text was pasted.

Focus returned to Chrome after each recording (active title bar; paste landed).
Not covered: multiple monitors, niri/grim on hardware, Sway/Hyprland, GNOME/X11.

After rebasing onto main with the Timeline pill (PR #5):

5. `05-timeline-pill-numbered-bands.png`: the pill shows bands 1 and 2 at the
   sketch moments, published through `koett_engine::sketch`.
6. `06-rebased-on-timeline-pill-pasted.png`: same two-sketch paste (1600×457
   sheet, then two linked bullets); pill shows Done.
7. `07-ink-cropped-sheet-with-aligned-linking.png`: final branch (alignment
   linker + ink-aware crop). Tiles are cropped around each sketch's ink with at
   least 45% of the monitor as context; the pill is outside both crops.

After the PR #6 review fixes:

8. `08-review-fixes-two-sketch-regression.png`: two-sketch flow unchanged
   (1600×457 sheet, two linked bullets, pill bands 1 and 2).
9. `09-cancelled-sketch-not-pasted-later.png`: draw, `koett cancel`, turn
   drawing off, dictate normally: only `text/plain` is pasted, no image.
10. `10-presentation-gated-capture.png`: captures start only after GDK reports
    wp_presentation feedback for the frame showing only that sketch's ink; the
    log has no timeout fallback ("without presentation feedback") entries.

Device mode (draw key read from keyboard state, no overlay focus):

11. `11-device-mode-typing-while-drawing.png`: after the first Shift hold drew a
    circle, "typed while recording" went into Chrome mid-dictation; Chrome kept
    focus (active title bar).
12. `12-device-mode-three-holds-three-sketches.png`: three separate holds gave
    three sketches (1356×1151 sheet) and bullets #1–#3, each linked to its own
    clause; pill bands 1, 2, 3.

Shift came from `lab-grabbed-shift.py`: a uinput keyboard grabbed with
EVIOCGRAB before any key is sent, so the host compositor never received the
key; Koett saw it only through EVIOCGKEY key state. The pointer moved through
KWin fake input without key events (`lab-gesture 0 …`).
Not covered in the lab: the "access denied" fallback (unit-tested decision;
focus mode itself is the verified PR #6 path).
13. `13-device-mode-hotplugged-keyboard.png`: the virtual keyboard appeared
    after recording started; the rescan picked it up, three holds gave three
    sketches (#3 after speech ended is `marked`), and typing still reached
    Chrome.

Typing never blocked (2026-11-03, drawing on, no opt-in, lab `koett-typing`):

14. `14-no-focus-default-typing-and-drawing.png`: text typed during the recording
    landed in Chrome, which kept focus; a Shift hold still draws (pencil, band 1).
15. `15-capital-tap-does-not-draw.png`: a 40 ms Shift tap plus pointer movement
    left no ink and no band.
16. `16-no-key-access-types-and-draw-off.png`: keyboards hidden with bubblewrap:
    Shift + movement drew nothing, typing still reached Chrome.

Phantom Shift (2026-11-04, lab `koett-realshift`; Shift from `lab-grabbed-shift.py`,
gesture from `lab-gesture.c`, pill visible in every frame; ink = sampled red pixels):

17. `17-old-build-virtual-shift-draws.png`: the build then installed (`8b4e91af`),
    only the virtual Shift held: ink 601, pencil and band. The bug.
18. `18-fixed-virtual-shift-no-ink.png`: the fix, same input: ink 0.
19. `19-fixed-named-lab-keyboard-draws.png`: the fix with that device named in
    `KOETT_TEST_DRAW_KEYBOARD`: ink 581, so drawing still works.
20. `20-fixed-ctrl-release-no-ink.png`: Ctrl, Shift, release Ctrl, move: ink 0.
21. `21-fixed-fresh-hold-draws.png`: then a fresh Shift hold: ink 385. The dialog is
    the lab's screenshot portal for the sketch capture.

`phantom-shift-sample.log`: read-only poll of Shift on Olle's desktop devices, no
input sent. The grabbed lab device went down four times for 2 s each with no
keyboard press; his real K100 press at 01:05:35 is separate.
