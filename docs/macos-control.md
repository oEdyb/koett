# Local Mac control

The installed app starts a user-only Unix socket at
`~/Library/Application Support/Koett/control.sock`. It is mode 0600, checks the
peer's effective user ID, limits requests to 2 KiB and replies to 4 KiB, and
accepts at most four clients with two-second read/write deadlines. No network
listener, permission prompt, transcript content or API key is exposed.

Run `/Applications/Koett.app/Contents/MacOS/koett --control COMMAND [ARG]`.
Omitting COMMAND requests status. Each invocation returns one JSON object:
`ok`, `phase`, `model`, optional `paused`, optional `error: {code, message}`.
Status also includes `pasteAccessGranted` and, after an attempt, a text-free
`paste` diagnostic with an ISO-8601 timestamp; `pasteLogError` reports log I/O
failure. See [paste readiness](mac-paste-readiness.md).
Exit 0 means accepted; nonzero means rejected. An accepted asynchronous command
is not a paste receipt or a completed-media receipt. Read status until ready,
and use the existing pill/history to inspect the result. Do not poll during
ordinary dictation unless a caller needs status.

Commands:

- `status`: readiness and selected model, without transcript content.
- `start`, `stop`, `pause`, `resume`, `cancel`: current dictation controls.
- `undo`: remove the last drawing/pin during recording.
- `draw-lock`: toggle multi-stroke drawing during recording; requires drawing enabled. Status includes `drawLocked` and the content-free `drawStrokeCount`.
- `history`, `dictionary`: open the existing native window.
- `check-updates`: the same foreground check as the menu; requires an enabled feed and an idle app/updater.
- `copy-last`, `paste-last`: use the same recovery actions as the menu.
- `retry-save`: retry pending stable transcript IDs.
- `transcribe-file /absolute/file`: transcribe local audio/video.
- `transcribe-url https://…`: use the existing media fetch path.

Model launch flags remain `--parakeet`, `--parakeet-v3`, `--parakeet-110m` and
`--nemotron`; they override a launch without changing the saved model. Native
shortcut recorders and provider/Keychain settings remain in their existing UI;
control does not dump secrets or edit preferences through unchecked JSON.

Status includes `updates`: feed enabled, backup preparation, pending installation,
install reservation, current build, last error and verified rollback path. A
successful `check-updates` reply means the check started, not that installation
finished. `updates_disabled` means the signed bundle contains no usable feed.

Examples:

```sh
/Applications/Koett.app/Contents/MacOS/koett --control status
/Applications/Koett.app/Contents/MacOS/koett --control pause
/Applications/Koett.app/Contents/MacOS/koett --control transcribe-file '/path/to/video.mp4'
```

The app refuses to replace another live listener or a non-socket file. It can
reclaim its own stale socket after a crash. During model relaunch the new app
retries endpoint startup when ready after the previous instance exits. Tests
pass explicit temporary socket URLs and isolated defaults/storage; constructing
a test controller alone never opens the production socket.
