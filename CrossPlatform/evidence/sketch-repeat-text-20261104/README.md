# Repeat-paste text regression

The installed baseline (8970c15, including #42) and the candidate used a single
private agent-lab koett-repeat-text KWin desktop, private app state and a copied
Parakeet model. No host pointer, clipboard or microphone was used. No T3 message
was sent. The test audio is the model's test_wavs/0.wav.

receipt.json records representation types and assertions without transcript
text. The baseline offered PNG only; repeat paste put an image in T3 and Chrome
without the transcript. The candidate offered text only; each target received
two complete copies. T3 renders the leading bullet marker as a list. Kitty's
normal Ctrl+Shift+V delivered exact Unicode text twice. Explicit copy-sketch
then attached the image in T3 without removing the existing text; copy-last
selected text again.

The two Python files are snapshots of the actual PC fixture, with its explicit
local paths. targets.py baseline and targets.py candidate were run through
agent-lab run koett-repeat-text -- uv run --with playwright python ...
They require the installed T3 binary, Chrome, kitty, the copied model and the
private compositor input helper at the recorded cache paths. The helper sends
Shift and pointer motion through KWin's private fake-input protocol; it never
creates or grabs a host uinput device. The app uses /dev isolation to exercise
the existing opt-in focused drawing fallback. The screenshot permission was
reused from the earlier owned private Koett lab.

These are repeat-paste checks, not fresh proof of automatic key injection:
KWin has no supported external-focus query here, so Koett correctly kept the
automatic guard and showed Not pasted. Automatic image/text ordering, keys,
waits and guard code were not changed. The user confirmed that initial automatic
delivery already worked.

main-copy-sketch.png shows the persistent Copy last sketch control on the actual
native main window. The Done pill also has an image button; recovery keeps its
existing Paste and Copy buttons within the same 300×48 row.
