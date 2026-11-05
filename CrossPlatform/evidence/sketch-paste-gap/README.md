# Sketch paste gap

The installed Linux host's metadata log measured **771 ms** between image and
text key posts (219,053-byte PNG). Its image step spent exactly 700 ms after
posting, followed by 71 ms of text clipboard/focus/native-keyboard preparation.
Both were attempt 1, modifiers were clear, and neither a retry nor the portal's
read wait caused this gap. No History text or clipboard contents were read.

Private niri 26.04 measurements, before → after, median key-post gap:

- T3 Code's actual chat box: **776 → 201 ms**, six cases per version;
  candidate range 189–227 ms. Median visible-image-to-text gap: 694 → 123 ms.
- Chrome's native rich contenteditable: **775 → 198 ms**, six cases per version;
  candidate range 188–211 ms. Visible gap: 753 → 184 ms.
- Kitty with a clipboard-reading terminal application fixture: **776 → 195 ms**,
  two cases per version, candidate 191–198 ms. The fixture reads PNG on Ctrl+V,
  verifies its hash, and accepts bracketed text on Ctrl+Shift+V. This exercises
  real kitty delivery and a local TUI consumer, not an external AI service.
- Plain kitty/text-only fixture: **778 → 768 ms**. It never requests PNG; the
  bounded fallback intentionally remains. It received the exact text once.
- A terminal consumer delaying its PNG request by 250 ms completed at **373 ms**;
  a deliberately throttled large reader at **726 ms**. Both retained the exact
  PNG hash and text once. Neither was forced through the fast deadline.

Each browser version had three small-image and three large-image pastes. The
large PNG has six labelled panels, 2048×1536 pixels and 8,701,340 bytes. All 24
browser deliveries retained one image and one occurrence of the text, with two
actual paste events. The terminal journeys also checked small/large payloads,
slow/no-read paths and exact text (normalizing terminal line endings and
bracketed-paste delimiters). The real T3 frontend was hosted by `~/.local/bin/t3` (CLI version 0.1.2) in
private desktop mode, rather than controlling the running host window. It was
isolated, authenticated with
its own ephemeral bootstrap credential and sent **no messages**. Its responsive
composer was expanded before tests. All inputs/focus/clipboard activity remained
inside the private `koett-paste-focus` lab, with copied model paths.

`measurements.json` contains individual event and visible-arrival samples. The
baseline compiled main's original output.rs against the same owner/native-key
backend as the candidate; the only timing difference is the image wait. Host
before timing is real; after timing is private-lab evidence, not a host install.
The visible measurements poll loaded image/text DOM at 5 ms intervals; they are
not screen-photodiode measurements or an app-side acknowledgement API.

## Change and safety limits

Only the Linux fallback image stage is shortened. Its offer now carries a
per-source transfer-completion signal, armed immediately before its existing
paste. Pre-arming reads cannot release this wait; another offer cannot release
it either. A successful full write **and pipe close** signals completion. The
image stage then observes a 120 ms minimum since posting, followed by the
existing guarded text delivery (~70 ms in the host trace). This interval allows
image event handling; it is not a replacement for the transfer signal.

A started transfer gets up to 1.5 seconds. On failure/stall, automatic and recovery
paths do not send the subsequent text chord; the existing Not pasted/Paste/Copy
recovery remains. Independent writers continue after selection cancellation and
the existing newer-copy protections remain. No read triggers new typing. Wayland
does not identify which client requested a data-control transfer or acknowledge
PNG decoding; completion confirms bytes supplied, not attachment processing.
Targets with no observed PNG request retain the prior 700 ms bound, preserving
compatibility with late/unknown consumers. The 700 ms wait after the text paste
before restoring the combined repeat-paste offer is outside this visible gap and
is unchanged. Clipboard formats, key chords, focus guards and pill controls are
unchanged.

Portal code already waits for its served counter (up to 1,500 ms, polling 20 ms),
then 160 ms; it was not the host's path and is unchanged. Mac SketchPaste uses a
lazy NSPasteboard provider, waits up to 1,500 ms (10 ms polling) for a read, then
always 160 ms. It does the same after text before restoring the combined offer.
Swift is unchanged; Mac timing is source analysis, not a new runtime benchmark.

Validation: 425 Rust tests passed, 19 ignored; formatting, Clippy with warnings
denied and release build passed. The private native-owner regression also passed
(newer copies, cancellation, full transfers, shutdown and the forced ownership
race). New regressions cover pre-paste reads, unrelated sources, complete large
writes and broken pipes. No dependency, packaging, ASR or Swift changes.
