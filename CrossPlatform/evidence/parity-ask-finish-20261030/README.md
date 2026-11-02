# Ask completion evidence

Tests ran 2026-10-29; directory name follows the requested handoff path.
Only owned synthetic/private-lab data and the public Parakeet sample were used.
No provider/cloud requests, real credentials, host capture or installed app changes.

`light-checks.json` proves the product microphone → local ASR → real private
portal JPEG → private Secret Service → actual curl HTTPS worker → native rich
answer path. The loopback endpoint used a private CA only through child
`CURL_CA_BUNDLE`; production HTTPS validation and redirect rules were unchanged.
Full and truncated responses copied exactly; completed questions were counted in
private SQLite. Cancellation occurred during an observed local ASR phase and
before the fixture sent its first byte. Permission cancel/restart and denial
made no additional provider request. Normal dictation then transcribed public audio.
`dark-large-checks.json` covers full/partial/cancel-ASR/denial and dictation with
native 14pt settings. Copy uses the actual native button and private clipboard.

`secret-service-checks.json`: owned gnome-keyring-daemon on a dedicated child
D-Bus with private XDG/control storage and synthetic credentials. Product secret
access failed for missing and locked entries. The locked test had no display;
no host keyring/bus was contacted. Its daemon was terminated and waited for.
`x11-checks.json`: product capture against owned rootful Xwayland on the lab,
not the host X server. One JPEG stayed in memory; the X server was stopped.

Renderer uses unchanged pinned Mac assets, embedded at build time with fonts.
Optional stable WebKitGTK 6.0 C ABI; no added Rust dependency or link-time GTK
floor. Ephemeral network session, transparent background, CSP denying external
resources and navigation policy denying external navigation. No runtime CDN.
Isolated official Arch package `webkitgtk-6.0 2.54.2-1` was extracted, never
installed. SHA-256:
`38806dbdad248fb6f4bc6e609b3b5bac49f2ac250c2b0514b0595fba2fee0e80`.
Detached signature verified against existing system keyring:
`83BC8889351B5DEBBB68416EB8AC08600F108CDF` (Jan Alexander Steffens).
`scripts/with_isolated_webkit.py` supplies the package's fixed helper paths in
an owned mount namespace, keeping WebKit sandboxing enabled. Software WebKit
compositing was selected only for this isolated evaluation. The system GTK and
normal application floor remain unchanged; distribution packaging must include
the complete WebKitGTK 6.0 runtime and helpers for rich Ask.

Screenshots were inspected: light/dark, large font, listening, error, full/partial,
math/table/highlighted code, permission denied, cancellation, and a 360px ordinary
window fallback. The separate narrow probe included executable HTML that would
replace the page with UNSAFE; the rich content remained intact. Escape hid it.
Native labels/buttons are exercised through AT-SPI, with real private input
serials for clipboard ownership. WebKit's internal accessibility tree reports
registry connection warnings in the isolated runtime; a complete screen-reader
acceptance is not claimed. No Mac screenshot comparison or appearance approval.

Layer-shell capable Wayland uses an interactive right-side overlay, keyboard
on demand, 480px width and source-derived bounded height. Unsupported desktops
explicitly report ordinary-window fallback. No universal always-above promise.
Physical-seat X11, multi-monitor targeting, distribution clean-install and native
hardware renderer checks remain parent release checks. TTS/Nemotron and other
non-Ask parity work are outside this increment.

Final source gates: fmt, 116 Rust tests/three existing ignores, strict all-target
Clippy, locked release, Rust 1.92 check and Windows source check passed. Python
compilation passed for all scripts. Ruff 0.17.2 passes the Ask scripts; the broader
`scripts/` scan reports 21 existing issues in untouched packaging/build/inventory
scripts (`ruff-final.log`). Those files were preserved for the parent floor pass.

Tested isolated invocation (after verifying/extracting the package):

```sh
agent-lab run koett-ask-finish -- env LD_LIBRARY_PATH=/tmp/koett-native-host-abi0/lib python3 /home/olle/dev/koett/CrossPlatform/scripts/with_isolated_webkit.py /tmp/koett-ask-webkit -- env WEBKIT_DISABLE_DMABUF_RENDERER=1 python3 /home/olle/dev/koett/CrossPlatform/scripts/test_linux_ask_acceptance.py --lab koett-ask-finish --binary /home/olle/dev/koett/CrossPlatform/target/debug/koett --output /tmp/koett-ask-fresh-acceptance --theme dark --scaled
```

The helper requires `bwrap` and a complete signature-verified extracted package.
Runtime system prerequisites are curl, Secret Service/secret-tool, GStreamer
PipeWire/conversion/scaling/JPEG plugins (plus ximagesrc on X11), and optional
WebKitGTK 6.0 with its subprocess helpers. No system installer was run. Renderer
third-party notices/licenses are embedded as inert page content with the assets.
`final-checks.json` and `final-dark-large-*` are from the final source after hiding
empty labels; the complete answer/code/table/math fits the inspected large-font
panel. Cleanup checked zero private PipeWire streams before stopping the lab.
