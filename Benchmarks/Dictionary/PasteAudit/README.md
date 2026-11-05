# Disposable macOS guest paste probe

These sources are harnesses, not app changes. Never run them in the host login.
`StopDriver` refuses accounts other than the disposable `/Users/admin` guest.
Compile on Apple silicon with `swiftc -target arm64-apple-macosx15.0`.

Use a cloned Tart guest with host clipboard/audio/graphics sharing disabled,
BlackHole as default input and output, copied model caches and no user data.
The unmodified app must be ready with Parakeet v2, raw formatting, empty dictionary,
and toggle shortcut Control-Shift-K (key 40, modifiers 393216). Give only the guest
app and test driver the needed test permissions. On this image, Input Monitoring
(`kTCCServiceListenEvent`) was also required for Koett to receive synthetic keys.

Start `PasteTarget` and make its text view the guest's focused target. Its ordinary
Edit menu routes Command-V through the responder chain into `NSTextView.paste`.
The receipt is written only after `super.paste` returns and includes the inserted
text and guest system uptime. It is not a callback from the app's event-post API.

`StopDriver <synthetic fixture.wav>` clears the target using Command-A/Delete,
posts the start shortcut, waits 300 ms, plays the exact WAV through BlackHole,
waits until playback ends plus 150 ms, and posts the stop shortcut. It polls for
a receipt for at most 180 s. The measured start is immediately before posting
stop-key-down; it includes event queueing, the real recorder stop and the complete
app transcription/delivery path. The 40 ms key release does not start the timer.
The receipt timestamp measures completed text insertion, not compositor rendering.

Use immutable versioned guest executable paths and compare SHA-256 on both sides.
Virtio shared-folder executable caching returned an older binary at a reused path
during setup. Copy each verified binary to a fresh local guest path before running.

All audio and text must come from `Benchmarks/Dictionary/corpus.json` synthesis.
Never mount or export host History, Last Transcript, dictionary or Failed Recordings.
No app code, ASR, pasteboard delivery, paste guards or storage is stubbed here.
This checks a simple native text target, not a browser, T3, rich image paste, the
physical Option keys or a human microphone. VM Core ML results are not native
M5/ANE latency and cannot reproduce the video's absolute number.
