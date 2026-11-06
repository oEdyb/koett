# Draw Lock undo key consumption

The passive sketch monitor previously undid ink but returned the Z event to the
focused application. Drawing undo now runs in the consuming drawing event tap.
Cmd-Z and Ctrl-Z consume keyDown, repeats and the matching keyUp; repeats do not
undo extra strokes. Ordinary typing, redo and undo outside Draw Lock pass through.
The passive overlay monitor no longer executes undo, preventing duplicate work.

Guest runtime source `278a3010c01dba41b26878223a8c92ebeba8a8e0`, disposable macOS 15
admin profile, local Developer-ID-signed Koett build. An owned native editor logs
only its own synthetic key events and Undo action. Its Swift source, exact driver,
source/binary hashes and receipt are in
[evidence](evidence/macos-draw-undo-consumption-20261106/undo-receipt.json).

- Before recording, both Cmd-Z/Ctrl-Z reach the editor; Cmd-Z invokes its Undo.
- Actual recording → Draw Lock → two mouse strokes.
- Cmd-Z changes the production `drawStrokeCount` from 2 to 1; Ctrl-Z changes 1 to 0.
- Neither Z keyDown/keyUp nor the editor's Undo action reaches the recipient while locked.
- Unlock → both chords reach the editor again, including its native Undo action.
- Cancel returns ready without a transcript. The signed executable hash matches staging.

The first driver attempted cross-process held/repeated key injection without
per-action state acknowledgement and failed its drawing-count assertion. It is
not accepted evidence. The final driver uses complete key strokes and reads each
result. Physical auto-repeat is not guest-certified; pure key-state tests cover
repeat swallowing and release after unlock. No host input, microphone, clipboard,
installed app or frozen ZIP was used or changed. No terminal process was exercised;
the native recipient checks the actual key events before AppKit dispatch.
