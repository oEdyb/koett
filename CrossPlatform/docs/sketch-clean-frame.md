# Clean-frame capture experiment

The first point starts an asynchronous monitor screenshot. Ink for that group
is withheld until the screenshot worker completes, so its own strokes cannot
enter that screenshot. At group close, a worker joins the capture and composites
round #ff3b30 strokes with separate frame/logical X and Y scales. Crops retain
at least 45% of each monitor dimension and 40 logical pixels around ink,
clamped to monitor bounds. Ink fades immediately on lift, independently of capture.

Public sketch interfaces and delivery are unchanged. No dependencies added.

## Evidence and limits

Unit tests cover fractional scaling, red pixels, and edge-clamped context.
Synthetic sample: /tmp/koett-clean-frame-synthetic.png (not a desktop capture).
Runtime logs report close-to-ready milliseconds (worker time, excluding the
next UI poll of up to 33 ms). No real capture latency comparison was collected.

The initial frame can still contain the recording pill and earlier groups.
Desktop contents changing after first stroke are intentionally frozen.
Slow screenshot permission delays initial visible ink. This is an experimental
tradeoff, not proof of a clean compositor capture.

Private KWin launch succeeded using temporary XDG directories. Screenshot
/tmp/koett-clean-lab.png showed model installation, so recording/sketch capture
was not verified. Lab stopped afterward. No visible desktop was used.
Linux fmt, tests, Clippy and release build passed. Windows cross-check failed
in libsqlite3-sys native build setup.
