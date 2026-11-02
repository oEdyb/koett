# Private X11 shortcut acceptance

The focused harness is `CrossPlatform/scripts/test_linux_shortcuts_x11.py`.
It starts rootful Xwayland inside `agent-lab koett-shortcuts-x11-owned`, receives
its allocated display through `-displayfd`, removes `WAYLAND_DISPLAY`, and sets
`XDG_SESSION_TYPE=x11` and `GDK_BACKEND=x11`. XTest events, clipboard reads,
screenshots, GTK accessibility, model capture and settings target only that
private server/session/audio graph. No host input, focus, clipboard, microphone,
audio output, user settings, transcript history, cloud calls or model downloads.

The final binary is a frozen copy of the parent's release build, SHA-256
`26f178545ddd5c1044cbfbe75297fe52d7f49ab41ad3a9c1e4f9c6f96e099921`.
The fixture uses the already cached Parakeet model and its public `test_wavs/0.wav`.
`xclip` was absent from the host. Package `xclip 0.13-6.1` was extracted under
`/tmp/koett-x11-tools`, added only to the private process PATH, without installing
anything on the host. Package SHA-256:
`bd6c0c7795b7bad6bd28bed8ddd5b47a8a9194408f585b31441cf716de959b4e`.
Source: https://mirror.krfoss.org/cachyos/repo/x86_64_v4/cachyos-extra-znver4/xclip-0.13-6.1-x86_64_v4.pkg.tar.zst

The initial fixture required X11 session type as well as removing Wayland display.
GTK ComboBox exposes Selection, while Action has no action zero and Component
focus is unavailable; Selection selects the native page. GTK shortcut button
names come from the mnemonic label (Dictation/Ask/Media); child labels contain
the captured chord. These were fixture corrections, not product failures.

A product failure was reproduced: recording an already grabbed shortcut in the
native editor stayed at “Press your shortcut…” because XGrabKey consumed the
press. `grabbed-chord-before-fix.png` was inspected and shows this state. The
parent fixed delivery to the active recorder. `core-before-capture-fix.json`
retains an earlier partial run; its false result reflects missing fixture xclip
at the final Copy Last step, after the other listed checks passed. It is not the
final acceptance result.

Full matrix: **PASS**, exit zero, 13 recorded checks plus actual media-window
shortcut dispatch. `results.json` carries exact assertions, active/desired/failure
JSON, binary hash and exact public-WAV Last hash. All five final PNGs were
inspected: native shortcut page, staged collision message, captured swapped keys,
media window, and post-rollback usable main window. The rollback error is proved
by JSON rather than the transient notice in the screenshot.

Proved: real Dictation toggle/repeat/release, disabled Ask Error/cancellation,
media opening, native Escape releasing capture suppression, native new binding
Save/reopen/activation, duplicate Save rejection without persistence, real native
capture of already grabbed swapped keys and both actions after Save, Hold repeat
and release including release while the native recorder is active, occupied-key
failure/JSON/rollback/prior activation while unrelated settings persist, public WAV
transcription with exact Last==History, exact Last through Settings Save/restart,
actual Copy Last clipboard byte equality, and saved shortcut activation after a
full process restart.

Python compile and Ruff pass. No Rust/product files were edited by this worker.

Reproduction (after providing xclip on the private PATH):

```sh
agent-lab start koett-shortcuts-x11-owned
agent-lab run koett-shortcuts-x11-owned -- env PATH=/tmp/koett-x11-tools/usr/bin:/usr/local/bin:/usr/bin:/bin /usr/bin/python3 /home/olle/dev/koett/CrossPlatform/scripts/test_linux_shortcuts_x11.py --binary /tmp/koett-x11-final-release --output /tmp/koett-x11-acceptance-final-2 --theme dark --renderer cairo
agent-lab stop koett-shortcuts-x11-owned
```

Limits: this is actual X11 input and XGrabKey behavior on a private rootful
Xwayland server, not a physical Xorg desktop/keyboard test. Ask was deliberately
disabled: Error dispatch and cancellation are proved, not paid-provider replies.
Media window dispatch is proved, not a full media transcription. Shortcut collision
error persistence/visibility is proved; no general Settings error-focus claim.
Automatic paste into another app is outside this focused matrix. No Rust build,
full suite, or independent product review was run by this worker.

Capture-lifecycle follow-up: **PASS**, exit zero, three focused native checks on
the subsequently rebuilt release, SHA-256
`03f901951be6bba12629aa7b1c55df0b3e19b31d9cd7b91945cc9e20ea084563`.
`capture-lifecycle-results.json` proves repeated activation of the same recorder
still captures an already grabbed key and releases suppression; Ask Reset cancels
an active Dictation recorder and restores actual shortcut recording; closing an
active recorder and reopening supports another capture while discarding unsaved
changes. `capture-lifecycle-complete.png` was inspected. The full matrix was not
repeated after these targeted capture-lifecycle changes.

Run the same command with the later binary, a fresh output directory, and
`--capture-lifecycle` for this focused follow-up. The fixture waits for GTK's
asynchronous accessible button activation before injecting keys; the first
attempt omitted that wait and correctly failed before capture became visible.
That attempt is not a product failure.

Cleanup: private Pulse source outputs were empty and the owned remap source was
unloaded (`audio-cleanup.json`). Both owned Koett processes, Xwayland servers,
accessibility registries and playback processes exited. The owned lab was stopped,
and `agent-lab status koett-shortcuts-x11-owned` reported `stopped`. Raw private
settings/history/model output stayed under `/tmp`, outside the evidence directory.
