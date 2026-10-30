# Koett for Linux and Windows

Local voice-to-text with configurable shortcuts, models and microphones. Linux has
native GTK controls, a recording waveform, searchable history, Copy and Export.
Windows remains a source-build preview. [Mac download](https://github.com/oEdyb/koett/releases/latest).

## Install

Choose the matching **Linux system-runtime** archive from
[Releases](https://github.com/oEdyb/koett/releases). Extract it, then run these
commands from the extracted folder:

```sh
./koett --check-runtime
./install.sh
```

Open Koett from your app launcher. The first launch downloads the default English
model. It stays loaded while Koett runs. The installer uses `~/.local`, retains
your settings/history and previous app version, and never stops a running Koett.
Restart after an update to load the new version.

Linux preview builds use the distribution's GTK and audio libraries. They are built
on Ubuntu 22.04 with GTK 4.6 as the API floor. Run the runtime check even if your
packages are installed. It reports missing libraries and optional tools without
changing your system. See the archive's `SYSTEM-RUNTIME.md` for dependencies and
known toolkit limits. The separate `current-pc` bundle is specific to its build
machine and should not be used as a portable Linux download.

For an isolated install, use `./install.sh --prefix '/a private folder'`.
Python 3.10+ is required. Verify the archive against its published SHA-256 file.

## Use

- **Record:** press `Ctrl+Shift+Space`, speak, then press it again. Hold mode is
  available. Your desktop may manage the shortcut itself.
- **Models:** Menu → Preferences → Model. Choose compact English, Parakeet v2,
  multilingual v3 or experimental Nemotron. Missing models download with visible
  progress and Cancel. The default remains 110M English.
- **Settings:** choose your microphone, check its input meter, set shortcuts and
  add word replacements. Optional media pausing is off by default.
- **History:** search recent and older dictations, select an entry, Copy or Export.
  Copy Last and unsaved-text recovery remain available after failed delivery.
- **Formatting:** Raw is the default. Optional S1-mini cleanup runs locally and
  falls back to raw text if cleanup fails. System-runtime builds need
  `llama-server` on PATH before enabling it.

Wayland uses the desktop's GlobalShortcuts and RemoteDesktop portals when
available. Otherwise bind `koett toggle` in your compositor; hold mode uses
`koett start` and `koett stop` with press/release bindings. Koett does not require
an input group or privileged keyboard daemon. On niri/wlroots, install
`wl-clipboard` and `wtype`; KDE/GNOME use the portal path where supported. X11
uses native global shortcuts and XTest.

Settings → Text → Paste shortcut selects Ctrl+V or Ctrl+Shift+V for all output
backends. Choose the chord your app accepts. Sending it cannot prove the app
consumed the paste; history and Copy Last preserve the result.

Optional **media transcription** uses the focused accessible browser document or
a manual HTTP(S) URL. Finite direct media, HLS and DASH are supported through
`~/.local/bin/yt-dlp` and `/usr/bin/ffmpeg`. Live, protected and login-required
sources are outside the supported scope. A separate result has preview and Copy.

Optional **Ask** sends your question and one screen image to your configured
provider. It needs `secret-tool` and a session Secret Service. Spoken replies use
Cartesia when enabled. Rich answers need complete WebKitGTK 6 with its helpers;
plain text remains available without it. Dictation needs no account or provider.

## Local data

Private `History.sqlite3` lives in Koett's XDG data directory. Search loads at most
50 snippets per page, then selected text separately. Legacy `Transcripts.md` imports
in the background and stays unchanged. Unclear archive boundaries remain searchable
as Imported archive with an unknown date; original bytes are preserved.

Completed text survives failed paste through history, Copy Last and recovery.
Failed history writes use private fallback files. Audio is retained only if local
recognition fails. Provider keys stay in Secret Service, not the settings file.

## Build

Use the pinned Rust 1.98.0 toolchain, Python 3.10+ with tarfile's data filter,
CMake, Ninja and a C++ compiler. Install GTK4/ALSA development headers plus the
X11/Wayland development dependencies. From `CrossPlatform`:

```sh
python3 scripts/build_asr_native.py --jobs 2
cargo build --release --locked --bin koett
python3 scripts/package_linux.py --flavor system-runtime --binary target/release/koett --output dist
```

The native builder verifies pinned sherpa-onnx source, disables TTS, and records
its ASR-only link inputs. The Rust build rejects missing or mismatched receipts.
The pinned Linux archives use different C++ string ABIs on x86-64 and ARM64;
the builder selects and verifies each one. Rebuild older ARM native caches with it.
`packaging/linux/Dockerfile` defines the Ubuntu 22.04 build floor. A build from a
newer distribution alone is not a portability check.

For a machine-specific bundle, preserve the three explicit runtime inputs:

```sh
python3 scripts/package_linux.py --binary target/release/koett --output dist \
  --gtk-runtime GTK_PREFIX --asr-runtime ASR_PREFIX --llama-runtime LLAMA_PREFIX
```

Use `--native-build-receipt PATH` when the native libraries linked into the
supplied binary were built outside the default `native-build` directory.

## Verify

Run the Rust checks from root `AGENTS.md`, then the model and archive probes:

```sh
cargo run --release --locked --bin koett-engine -- --self-test
python3 -m unittest discover -s scripts -p 'test_package_linux*.py'
python3 scripts/test_package_linux_archive.py --archive YOUR_ARCHIVE.tar.gz
```

On this workstation, native tests run only in an invisible private desktop:

```sh
agent-lab start koett
/usr/bin/python3 scripts/test_linux_ui.py --binary target/release/koett --model MODEL_DIRECTORY
agent-lab stop koett
```

The caller owns lab cleanup. `--record` uses the private session's audio source.
Tests must not connect to the visible desktop, host clipboard or physical audio.
A loader check or model self-test does not establish compositor/permission support.
Mac execution and native Windows acceptance are separate from Linux verification.
