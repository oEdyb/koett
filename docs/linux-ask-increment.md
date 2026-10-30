# Linux Ask implementation increment

Status: working integrated Linux Ask with native rich rendering and private HTTPS acceptance.
Full platform/release parity remains open; optional speech is outside this increment.
Existing uncommitted model, formatting, media and history work is preserved.
No installation, commit, push, live preference or private user-data access.

## Implemented

`CrossPlatform/src/assistant/` owns provider configuration, the exact Swift system
prompt, request construction, bounded SSE decoding, cancellable/reaped workers,
Secret Service access and one-frame ScreenCast capture. Defaults match Swift:
Groq `qwen/qwen3.6-27b`, temperature 0.4, reasoning effort `none`; OpenRouter
`~openai/gpt-latest`; custom HTTPS endpoint/model. No output-token override.
Cloud Ask is disabled by default. Audio stays in the existing warmed ASR worker.

ScreenCast requests exactly one monitor, hidden cursor and no persistence token.
The restricted PipeWire FD is inherited only by an owned GStreamer child. One
JPEG, at most 1600 pixels wide, travels through an anonymous pipe into memory.
The child is killed/reaped on timeout/cancel/error; the portal session closes
following acquisition or cancellation. No image path is created. Every selection
requires portal permission; automatic active-monitor targeting is not implemented.
X11 captures one root-screen JPEG through the existing x11rb/GStreamer boundary,
with hidden pointer, bounded scaling and memory-only output. An owned rootful
Xwayland probe passed; physical-seat and multi-monitor targeting remain unverified.

The GTK Ask panel is separate from History, uses 480px width and 440–660px source-derived height, displays the
question, screen marker, phase, independent storage warning and response error.
Streaming deltas remain selectable/copyable after a disconnect. Copy is explicit;
Ask never pastes. Provider settings have labeled model/endpoint/password fields.
Credentials pass through secret-tool stdin/stdout and curl configuration stdin;
never settings JSON, command arguments or logs. Locked/unavailable Secret Service
fails closed. Curl redirects are disabled, HTTPS enforced, connect/total deadlines
are 15/60 seconds, response ceiling 2 MiB, event ceiling 256 KiB.

The runtime saves a completed question once through shared resilient persistence
before provider access. Ask while answering drops/reaps the old answer before a
new capture. Ordinary dictation cancels Ask capture/answer ownership. Media/model
changes cannot acquire the microphone during a question. Existing ASR/Raw defaults,
history schema, platform vendor setup and Mac source are unchanged.

## Agent and desktop surface

- `koett ask`: toggle question capture; interrupt an answer with a new question.
- `koett ask-start`, `koett ask-stop`, `koett ask-cancel`: explicit operations.
- `koett ask-status`: JSON phase/screen/error/warning; excludes question/answer/key.
- `koett ask-help`: JSON command/configuration reference.
- `koett ask-configure`: read bounded configuration JSON from stdin.
- `koett ask-key groq|openRouter|custom`: read secret from stdin into Secret Service.

Map Control+Shift+Space to `koett ask` using desktop shortcut settings. This is
an explicit mapping requirement; no registered global Ask shortcut is claimed.
Pause dictation and existing media shortcut bindings are unchanged. Accepted
commands return JSON acknowledgement; status reports asynchronous outcome.

## Verification and boundaries

Evidence: `CrossPlatform/evidence/parity-ask-20261030/`.

- 116 Rust tests passed, three ignored. Tests use synthetic request bytes and a
  loopback protocol fixture; no real audio or frame is sent to any cloud endpoint.
- Request equality checks exact single-question/single-image envelope. Fragmented
  Unicode SSE, truncation, provider error, malformed/oversized/empty response and
  cancellation before first output are tested. Partial text survives truncation.
- Private `koett-ask` ScreenCast probe acquired an actual JPEG (51192 bytes), with
  no image file. Native light and dark integration recorded the public Phoebe sample,
  saved one question, displayed unavailable-key-store error, and then performed
  normal dictation. The public audio sometimes recognizes the name as “Phobe”;
  native assertions also check the “old portrait” phrase rather than spelling.
- Post-media-fix native rerun passed exact Copy, cancel/restart, Stop/Ready and
  public-sample ASR; `/tmp/koett-ask-media-regression/checks.json`.
- Strict all-target Clippy, fmt, locked release, MSRV 1.92, Windows source check,
  Ruff 0.17.2 and Python compilation passed. No Swift toolchain checks ran here.

## Completion acceptance

Current evidence: `CrossPlatform/evidence/parity-ask-finish-20261030/README.md`.
The actual product worker passed public voice → warmed local ASR → one private
portal frame → private Secret Service → local private-CA HTTPS → native full and
partial answer → exact Copy. Private SQLite counted each completed question once.
A real observed Transcribing phase was cancelled before provider access; no request
followed. Cancel before first byte, permission cancel/restart, permission denial,
and normal dictation after Ask all passed. No cloud endpoint or live key was used.
Missing and locked synthetic stores fail closed on a separate private child bus.

`ask_renderer.rs` dynamically loads stable WebKitGTK 6.0 without changing the GTK
4.6 core floor. `build.rs` embeds the unchanged pinned Mac assets and fonts;
no CDN or runtime asset path is required. The renderer uses an ephemeral session,
transparent background, sanitized GFM/highlighted code/tables/math, external
navigation denial and a CSP denying external resources. Original Markdown is the
Copy source. A child-renderer failure exposes the retained plain-text answer.
WebKit is created only on the first Ask, not ordinary dictation startup.

Complete WebKitGTK 6.0 runtime/helpers are an optional distribution prerequisite
for rich Ask. The tested official Arch 2.54.2-1 package was signature-verified and
extracted under `/tmp`, never installed. `with_isolated_webkit.py` supplies fixed
helper/library paths inside a private lab mount namespace without disabling the
WebKit sandbox. See the evidence receipt for SHA-256, signer and invocation details.
Without the runtime, a visible prerequisite message accompanies plain text.

`ask_position.rs` uses the existing preloaded layer-shell mechanism when supported:
480px right-side interactive overlay, keyboard on demand, 22px content insets and
height derived from 72% of monitor height bounded to 440–660px. Other compositors
explicitly report a normal-window fallback; universal always-above is not promised.
Before a new frame, the runtime waits for the GTK thread to hide the old panel.
Capture keeps a process-lifetime Tokio runtime because ashpd caches its D-Bus
connection; repeated portal captures now work after cancellation and errors.

Native light/dark, 14pt settings, listening/error/full/partial/code/math states and
360px fallback were inspected. Escape hides the panel. AT-SPI native controls and
private keyboard/clipboard input were exercised. WebKit's internal accessibility
registry reports warnings with the isolated package; full screen-reader acceptance
is still a release prerequisite. Appearance approval belongs to the parent/user.
No Mac reference screenshot comparison, hardware renderer certification or clean
package installation was performed. TTS, Nemotron, media HLS/browser detection,
remaining menus/recovery and release packaging remain separate work.

## Primary API references

- https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html
- https://gstreamer.freedesktop.org/documentation/pipewire/pipewiresrc.html
- https://curl.se/docs/manpage.html
- https://gnome.pages.gitlab.gnome.org/libsecret/secret-tool.html

No Rust dependencies were added; existing exact pins and MSRV remain unchanged.
Runtime tools currently require `/usr/bin/curl`, `/usr/bin/secret-tool` and
`/usr/bin/gst-launch-1.0` with PipeWire, conversion, scaling and JPEG plugins.

## Parent review fixes preserved

The shared cancellation token, Cancel acceptance during Transcribing, ASR handoff
checks before Secret Service/cloud, completed-line 256KiB SSE limit and dictation
Cancelled event remain intact. Native integrated acceptance now covers the earlier
review gap. Final Rust suite: 116 passed, three existing ignored; strict all-target
Clippy, fmt, locked release, MSRV 1.92, Windows source check, Ruff 0.17.2 for Ask scripts and Python
compilation passed. The broader scripts scan has 21 existing issues in untouched
packaging/build/inventory scripts; see `ruff-final.log` for the parent floor pass. No installation, commit, push, host capture or cloud request.

## Rich completion parent review

Fresh code/A and code/B reviews found a restricted portal FD equal to its child
target could retain close-on-exec, and the rich-renderer termination notice was
left hidden. Parent now explicitly clears descriptor close-on-exec after dup2,
with subprocess regression coverage for both source FD layouts, and shows the
plain-text fallback notice on renderer termination. 117 Rust tests pass, three
existing probes ignored; strict Clippy is clean. The earlier integrated native
receipts predate these small fixes; final native acceptance is still required.
