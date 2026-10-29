# Focused browser media and result capsule

Shared branch `feat/linux-ui-history`, base `925f0b46721700e68e578cfd0eec80ada5c162bb`.
No commits, pushes, installs, dependency/schema/settings changes or host browser access.
Backend changes in media_transcription.rs belong to the concurrent backend owner.

## Implemented

- User-invoked AT-SPI lookup, using ashpd's existing zbus5.19.0 re-export. Enumerates native application/window state, then only the active frame's accessibility tree; reads URI attributes only on the web-document ancestor of the unique keyboard-focused object. Rechecks active frame, focused object and document attributes before returning. Rejects ambiguity, nonHTTP(S), missing accessibility, timeout, oversized trees and changed focus/document. No clipboard borrowing, polling, title inference, profiles/cookies, debugger, screenshots or capture in the adapter.
- Total async lookup deadline1500ms. Native accessibility calls run on a short-lived worker runtime; the controller joins that bounded lookup. Busy controller requests reject before lookup. This can delay other controller dispatch for up to1500ms; it does not activate GTK or change external focus.
- Tray, X11 shortcut, Wayland shortcut and GTK media action route to the same controller lookup and existing EngineCommand::Media. Explicit `media URL` and `cancel-media` routes remain intact. Failed discovery opens existing manual URL window with its reason. Manual retry preserves the separate completed-media cache.
- `media-help`, `media-current`, `media-status`, `media-copy` expose JSON control. Current reports started/busy/manual_url_required. Status and Copy never print transcript text or provider credentials. Status reports the cached media result, not live per-job progress. Copy uses the same exact cached text as the UI.
- Existing keyboard-none layer surface shows250x48 media capsule, ellipsized preview and explicit Copy. Pointer input is restricted to Copy; waveform/status surfaces remain pass-through. No activation or keyboard mode workaround. Existing timer deadline replaces itself on completion and expires after10seconds. New recording/Ask/media notices/errors clear the result presentation. Full text remains in the existing media cache/window/store and never becomes dictation Last. Unsupported layer-shell retains the existing in-window recovery fallback.

## Native evidence

Private `agent-lab koett-browser-media`, private DBus/AT-SPI/XDG/browser profile/clipboard/audio. Fixture uses read-only cached public Parakeet test WAV exposed by owned loopback HTTP server through an HTML audio tag. Existing generic extractor fetches direct HTTP WAV; no manifest path or microphone capture is required for this journey. No paid/cloud ASR.

`test_linux_browser_media.py` exercises actual Google Chrome Stable forced accessibility on private Wayland/KWin. Native Chromium exposes Document attribute `URI`; role95 web document is checked because Chromium also exposes Document on other accessible objects. It does not establish Firefox or all-desktop support.

Dark/light receipts prove focused-page discovery, existing local ASR, UI/CLI Copy equality, typing retained, busy rejection, changed current document, repeated completion,10second expiry with cache retained and nonbrowser/manual fallback. Screenshots were inspected; these are not product appearance approval. Final verified receipt adds cancel/retry, an unresponsive Chrome process deadline below3seconds, and graceful controller quit. Final frozen binary SHA256: `c6cce36f254bf4fcf3f64abe3403a94f1538e727b3e456c15ab171bccdb0cb61`. Earlier light/dark UI binary: `dc7a0a8de1907a1dab2aa40a4cf79c435f808e87c0d70861ae26956c68b6f857`. The final adapter tightens oversized-tree rejection; the UI is identical. All owned lab processes/streams were stopped with agent-lab stop after the verified run. The test script compares clipboard text to the exact stored body after the metadata separator.

## Gates

- Affected Linux Rust tests:31 passed,2 existing ignores, serialized to avoid shared QUIT_BUSY test races. The first parallel run exposed that existing global-atomic test race; the serialized run passes without changing tests.
- Strict all-target Clippy, locked release, MSRV1.92 check, fmt, affected Ruff, Python compile and diff checks passed.
- Windows source check attempted but blocked: installed toolchains have only x86_64-unknown-linux-gnu; `can't find crate for core` for x86_64-pc-windows-msvc. No target install was authorized/performed.
- Parent owns final integrated full suite, review and packaging.

## Remaining proof limits

No native proof of Firefox, X11/compositors beyond private KWin, focus-changing midlookup rejection, an absent accessibility bus, or unsupported-layer-shell compositor in this increment. The guards/fallback are implemented; these specific fault/platform scenarios remain unverified. Global physical shortcut activation was not newly proven: this lab's portal could not identify the private uninstalled application desktop entry. Existing shortcut dispatch sites are wired to the proven controller; the native end-to-end invocation here used media-current. No separate appearance approval, narrow matrix or complete whole-port parity claim.

## Sources

- https://gnome.pages.gitlab.gnome.org/at-spi2-core/devel-docs/doc-org.a11y.atspi.Document.html
- https://gnome.pages.gitlab.gnome.org/at-spi2-core/devel-docs/doc-org.a11y.atspi.Accessible.html
- https://chromium.googlesource.com/chromium/src/+/refs/heads/main/ui/accessibility/platform/ax_platform_node_auralinux.cc
- Pinned local zbus5.19.0 proxy/connection builder source; official latest docs are6.1.0, so code uses the pinned local API rather than assuming the latest signature.
- Sources/Koett/{BrowserMedia,MediaSession,RecordingOverlay}.swift primary product behavior.

## Parent integration and review

Two independent reviews completed. Review B found no actionable issue. Review A found a same-window focus race during accessibility traversal. Parent now subscribes to bounded AT-SPI focus/window/document events before reading the active frame, registers those events with the registry, and rejects changes throughout discovery. The short-lived connection owns its registrations. Existing final frame/object/document checks remain. Registry API reference: https://gnome.pages.gitlab.gnome.org/at-spi2-core/devel-docs/doc-org.a11y.atspi.Registry.html.

Final frozen integrated binary: `fe9babba1c90cecb8d0bc86b6ca1ee116b6bc5b2fafe2df0ee24f146f05e14aa`. Curated receipts and inspected captures are in `integration/hls` (light) and `integration/dash` (dark). Both run actual Chrome page discovery through finite segmented downloads, warm ASR, exact stored/UI/CLI Copy, private global Ctrl+Shift+T activation, typing retention, cancel/retry, repeated completion and expiry. Controlled window changes and same-window tab switches during lookup return the event-guard error and create no extra transcript. An unresponsive browser remains bounded and falls back to manual entry. The harness now installs only a disposable desktop entry/shortcut, explicitly activates its own browser after modal faults, disables browser extensions, and copies model files to private state rather than linking a writable verification cache.

Parent gates: 146 Rust tests passed / 10 existing opt-in or benchmark ignores, serialized; 35 Python unit checks passed; strict all-target Clippy, release, MSRV 1.92, Windows 1.98 source including tests, affected Ruff/compile, formatting and diff checks passed. GTK 4.6 Ubuntu 22 release compilation passed; this is not native GTK 4.6 GUI proof. The target missing from the child's default toolchain is installed on the parent's explicit 1.98 toolchain, so its Windows source check is resolved. No Windows execution claim.

The private `koett-media-integrated` lab was stopped, with no capture streams before teardown. Earlier parent fixture attempts failed on missing private shortcut preregistration and compositor focus after modal faults; their transient outputs remain in the private cache, not this curated success evidence. Unsupported-overlay compositor runtime, Firefox, physical input/audio and final packaged installation remain unverified. Installed Koett and live preferences remain unchanged.
