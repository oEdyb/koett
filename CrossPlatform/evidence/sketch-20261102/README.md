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
