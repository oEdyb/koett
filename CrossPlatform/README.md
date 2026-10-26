# Koett for Windows and Linux

Koett records speech, transcribes locally, and pastes the result into your app.
The Linux window provides recent transcripts, search, Copy, Export and Settings.
The tray and desktop shortcut remain available while the window is closed.

The default shortcut is `Ctrl+Shift+Space`. The 110M English model downloads
once on first use and stays loaded while Koett runs. Long recordings process in
the background while you speak.

Linux requires glibc 2.35+, GTK 4.6+ and ALSA. PipeWire's ALSA bridge and PulseAudio's
ALSA plugin work with the existing capture path. GNOME does not need a tray
extension to use the window. Windows keeps its notification-area menu and pill.

## Use

- Open Koett from the app launcher. Record starts capture; Stop transcribes it.
- Settings selects the microphone, recording mode, shortcut and personal word
  replacements. Pause media while recording is optional and off by default.
- Search filters the full local archive. Select an entry to read, copy or export.
- If automatic paste fails, use Copy last transcript or the saved history entry.

Wayland desktops use their shortcut and remote-desktop portals where available.
Compositors without these portals can bind `koett toggle`, or `koett start` and
`koett stop` for hold mode, using their native shortcut settings. On niri/wlroots,
install `wl-clipboard` and `wtype` for clipboard/virtual-keyboard delivery. Koett
does not require an input group or privileged keyboard daemon. KDE/GNOME use the
permission-based portal path rather than assuming `wtype` works there. X11 uses
native global shortcuts and XTest paste.

The portal paste chord is Ctrl+V. Terminals requiring Ctrl+Shift+V need manual
paste from the clipboard. Sending a shortcut cannot confirm that the destination
consumed it; saved history and Copy last transcript remain available.

## History

New results live in private `History.sqlite3` in Koett's platform data directory.
SQLite indexes text and loads at most 50 snippets per page. Koett imports the old
`Transcripts.md` in the background and leaves it unchanged. Older text whose entry
boundaries cannot be established remains searchable as Imported archive with an
unknown date; original bytes remain preserved. Identical separate recordings
remain separate. Save failures retain the existing recovery-file path, with a
copyable in-memory result if storage also fails.

## Build

Use the pinned Rust 1.98.0 toolchain and Python 3.10+ with tarfile's data filter.
Install CMake, Ninja, a C++ compiler, GTK4/ALSA development headers and the X11/
Wayland development dependencies. Build ASR-only native libraries first:

```fish
python3 scripts/build_asr_native.py
cargo build --release --locked --bin koett
```

The builder verifies the pinned source archive, disables TTS, and stages only the
ASR link inputs. Its Linux C++ ABI setting matches the pinned ONNX Runtime archive.
The vendored Rust FFI source is unchanged; only the native-link build boundary is
patched. A normal Rust build refuses mismatched/missing native receipts.

`packaging/linux/Dockerfile` defines an x86-64 Ubuntu 22.04 build environment with
pinned base image, Rust and rustup. Build against that floor when producing a
candidate for other distributions; a current-distro build is not a portability
check. Keep separate Cargo target/native directories when using another toolchain.

Package a completed binary without installing or publishing it:

```fish
python3 scripts/package_linux.py --binary target/release/koett --output /tmp/koett-package
```

The package contains an installer, desktop entry, source/binary receipt and SHA-256
checksum. The installer defaults to the user's local bin/application directories;
`KOETT_BIN_DIR` and `XDG_DATA_HOME` support isolated install checks.

## Verification

Run the Rust checks in root AGENTS.md plus the native model probes:

```fish
cargo run --release --locked --bin koett-engine -- --self-test
cargo run --release --locked --bin koett-engine -- --long-self-test 420
/usr/bin/python3 scripts/test_linux_ui.py --binary target/debug/koett --model MODEL_DIRECTORY
```

The UI script uses real GTK/AT-SPI widgets and a synthetic archive. On this Linux
workstation, GUI tests run in the private, invisible `agent-lab koett` desktop;
input, audio, clipboard and portals must stay inside the lab. Stop it after testing.
`--record` adds a two-second capture using that session's selected input. Native desktop
permissions, focus/paste behavior and runtime support still require their actual
desktop tests; successful compilation is not evidence for those paths. Mac runtime
verification is separately deferred; no public release is implied by a local build.

The separate `koett-engine` binary remains available for WAV and microphone
benchmarks.
