# Koett Linux port and history research

As of 2026-10-26. Read-only research; no application changes or new runtime test receipts.

## Confirmed scope

Olle wants broad Linux desktop support immediately, with the port and UI as the priority.
Searchable in-app history must replace the growing Markdown file on macOS and Linux.
Hold/toggle, personal word replacements, and optional media pause/resume are accepted.
Novel features can follow; none were specified for this implementation.

## Repository state

Fresh checkout: `/home/olle/dev/koett`. `origin/main` is `6a4dfb5`;
`origin/feat/windows-linux-v0.1` is `e85d0d0`. Main contains the Swift macOS app.
The draft branch contains the Rust Windows/Linux application in `CrossPlatform/`.
`origin/codex/overnight-2026-10-03` contains newer macOS recovery work and needs
comparison before touching storage. Historical worktree paths in AGENTS.md are on
Olle's Mac, not this Linux machine. Neither `docs/decisions.md` nor `docs/bugs.md`
exists in this fresh checkout.

The Rust application uses CPAL 0.18.2, ringbuf 0.5.1, sherpa-onnx 1.13.5,
ksni 0.3.6, ashpd 0.13.13, global-hotkey 0.8.0 and x11rb 0.13.2.
Linux has a tray and external JSON settings editor, with no application window
framework. Windows uses Win32; macOS uses SwiftUI/AppKit. Preserve these platform
boundaries. Earlier research suggested Slint, but the implemented port does not use it.

`CrossPlatform/src/runtime.rs` owns the recording worker and warmed model.
`src/linux.rs` owns tray, shortcut registration, portal paste, X11 paste and autostart.
Long recordings already use Silero VAD/background processing. Do not replace that path.

## Current desktop evidence

This machine runs x86-64 CachyOS, niri 26.04, Wayland. It is not a KDE session.
Read-only D-Bus inspection exposes GlobalShortcuts v2 and Clipboard v1, but
RemoteDesktop.AvailableDeviceTypes is `u 0`. The existing Wayland output constructor
requires Keyboard, so its prerequisite fails here. Exposed interfaces alone do not
prove their backend methods work.

`wayland-info` advertises virtual-keyboard, data-control and layer-shell protocols.
`wl-copy` and `wtype` are installed. These observations justify testing an optional
compositor adapter; they do not prove successful text delivery. Retain the portal
path for desktops that support it and the X11 path for real X11 sessions.

## History and recovery

`Sources/Koett/TranscriptStore.swift` and draft `CrossPlatform/src/history.rs`
append timestamp/model/text entries to `Transcripts.md`. Mac also stores raw versus
cleaned text and a durable last-result file. Rust has private failed-transcript files.
Both desktop menus open the full archive externally. Neither provides indexed
search or a bounded in-app browser. Olle's usability report establishes the need
for a different storage/query path, superseding the earlier preference to avoid a DB.

Recommend SQLite plus FTS5, stable entry IDs, paged metadata/snippets and on-demand
full text. Use the same versioned schema on macOS and Linux, with platform-native
views. Preserve the original Markdown archive and all failed-save recovery paths.
Legacy Markdown is not self-delimiting: dictated text can reproduce headings and
model labels. Preserve ambiguous source bytes and report them during import.

## Missing local work and release gates

The Mac worktree reportedly has uncommitted KTT-002 ASR-only native build work.
It is absent from the fetched draft branch. The committed static sherpa setup
still has blocked-artifact evidence for bundled TTS/eSpeak components. Recover
and review that patch or rebuild the ASR-only path reproducibly; do not claim the
remote branch includes it. Public distribution still requires final artifact,
license/notice and packaging gates. History/UI work can progress independently.

No current Linux build, microphone, automatic-paste, Mac regression or desktop
matrix test has been run. Historical Fedora/Windows success is historical evidence.
The repository pins Rust 1.98.0; this machine's default Rust is 1.94.0.

## Documentation checked

- [Voxtype source](https://github.com/peteonrails/voxtype/tree/dc499ad47444ba6ea80d8750aa3581c23120821c): hold/toggle, replacements, media control and output adapters. Transfer behavior selectively; define deterministic replacement precedence and safe resume ownership.
- [SQLite FTS5](https://www.sqlite.org/fts5.html) and [transaction semantics](https://www.sqlite.org/transactional.html).
- [GTK4 Rust API](https://gtk-rs.org/gtk4-rs/stable/latest/docs/gtk4/): proposed Linux window toolkit, upstream 0.11.5 at research time. Cached docs also exist under `/home/olle/docs/gtk4-rs/`.
- [rusqlite](https://docs.rs/rusqlite/0.41.0/rusqlite/): proposed exact Rust dependency; verify bundled SQLite FTS5 support and MSRV before adoption.
- [GlobalShortcuts portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html), including Activated/Deactivated.
- [MPRIS Player interface](https://specifications.freedesktop.org/mpris/latest/Player_Interface.html).
- Existing ashpd and zbus docs: `/home/olle/docs/ashpd/`, `/home/olle/docs/zbus/`; read the exact pinned APIs before changing calls.

Two delegated research tasks inspected the code and Voxtype documentation. They
made no changes. Parent verified the local desktop capabilities and storage code.
