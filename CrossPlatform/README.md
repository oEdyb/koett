# Koett for Linux and Windows

Dictate while drawing on your screen: hold Shift and move to sketch, or
Shift+click to pin. Koett pastes a numbered image sheet followed by your words.
Drawing is optional and off by default. Local voice-to-text also has configurable
shortcuts, models and microphones, with native GTK controls.
Windows remains a source-build preview. Native media and console commands are
documented in [Windows media](../docs/windows-media.md). [Mac download](https://github.com/oEdyb/koett/releases/latest).

The published Linux v0.2.0-linux.1 and Mac v0.1.1 downloads do not include drawing.
The drawing controls below are available in the current source build.

## Install

Choose the matching **Linux system-runtime** archive from
[Releases](https://github.com/oEdyb/koett/releases). Extract it, then run these
commands from the extracted folder. Install Python 3.10+ first; the launcher and
installer require it. On Ubuntu 22.04 the base runtime packages are:

```sh
sudo apt-get install python3 libgtk-4-1 libasound2 libudev1 libxkbcommon0 \
  libwayland-client0 libx11-6 libxcb-shape0 libxcb-xfixes0
```

Other distributions use different package names. The runtime check remains
required after installing these packages:

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
known toolkit limits. Drawing and the floating recording pill also need
**gtk4-layer-shell** (`libgtk4-layer-shell.so.0`) from your distribution; this is
a separate library from GTK4. A successful base runtime check does not prove it
is installed or that your desktop supports drawing. niri/wlroots drawing also
needs `grim` for screen capture. System GTK 4.24.1 needs upstream accessibility
fix `21d3eb7` or its distribution backport. The separate `current-pc` bundle is
specific to its build machine and should not be used as a portable Linux download.


For an isolated install, use `./install.sh --prefix '/a private folder'`.
Python 3.10+ is required. Verify the archive against its published SHA-256 file.

## Update or uninstall

To update, verify and extract the new archive, run its runtime check and installer
with the same prefix, then quit and reopen Koett. Installing does not stop the
old process. Previous application versions and your data are retained.

To uninstall the default installation, turn off **Start at login** in Settings,
quit Koett and remove these application files:

```sh
rm -f "$HOME/.local/bin/koett" \
  "$HOME/.local/share/applications/com.olledyberg.Koett.desktop" \
  "$HOME/.local/share/icons/hicolor/scalable/apps/com.olledyberg.Koett.svg"
rm -rf "$HOME/.local/lib/koett"
```

For a custom prefix, replace `$HOME/.local` with that prefix. Settings, models
and transcripts are kept. Remove any compositor shortcuts you added separately.
Uninstalling does not revoke optional keyboard permissions; the
`koett-keyboard` group and its udev rule are separate system configuration.

## Use

- **Record:** press `Ctrl+Shift+Space`, speak, then press it again. Hold mode is
  available. Your desktop may manage the shortcut itself.
- **Models:** Settings → Model & formatting. Choose compact English, Parakeet v2,
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
`koett start` and `koett stop` with press/release bindings. Normal dictation does not require
a keyboard-access group or privileged keyboard daemon. Drawing has a separate
opt-in setup below. On niri/wlroots, install
`wl-clipboard` and `wtype`; KDE/GNOME use the portal path where supported.
Automatic paste requires a supported focus check. GNOME Wayland needs the
explicitly enabled [Koett Shell extension](integrations/gnome/README.md).
The Linux archive includes its files; copy them as described there, log out/in,
and enable the extension. GNOME 46 supplies focus checks; GNOME 51 also supplies
the recording pill and drawing. Other Shell versions need verification. If GlobalShortcuts
is unavailable, add a custom GNOME keyboard shortcut running `koett toggle`.
Allow both **Remote Interaction** and **Clipboard Access** in the Remote Desktop
prompt. Without the extension or portal
permission, use History/Copy and paste manually. X11 uses native global shortcuts
and XTest. X11 also has the recording pill.

Settings → Dictation → Paste shortcut selects Ctrl+V or Ctrl+Shift+V for all output

backends. Choose the chord your app accepts. Sending it cannot prove the app
consumed the paste; history and Copy Last preserve the result.

Optional **media transcription** uses the focused accessible browser document or
a manual HTTP(S) URL. Finite direct media, HLS and DASH are supported through
`~/.local/bin/yt-dlp` and `/usr/bin/ffmpeg`. Live, protected and login-required
sources are outside the supported scope. A separate result has preview and Copy.

## Drawing setup

Requires a Wayland desktop with layer-shell: KDE Plasma 6, niri, Sway or
Hyprland. KDE Plasma 6 has end-to-end drawing verification; the other desktops
have less verification. GNOME Wayland needs the Koett extension, and X11 needs a compositor.
Use `grim` for capture on niri/wlroots, or a working Screenshot portal on KDE;
allow screenshot access when asked. Multi-monitor drawing is not hardware-verified.

1. Open **Settings → Draw on screen** and enable **Draw & Pin While Dictating**.
2. If keyboard access is missing, click **Copy setup command**. Install your
   distribution's `acl` tools (`setfacl`), run the copied command in a terminal,
   then log out and back in. Reopen Settings and check keyboard access is ready.
3. Start dictation, release the recording shortcut keys, then hold **Shift** and
   move to draw. Release it to finish. **Shift+click** pins a point. Change
   **Draw key** to Ctrl or Alt if needed.
4. For several strokes in one sketch, toggle **Ctrl+Alt+D** while recording,
   then drag with the left button. Toggle again or press Esc to finish the sketch.

The setup creates a `koett-keyboard` group and a udev rule granting that group
read-only keyboard access. Other apps running as your user can use that access
too. Koett checks physical keyboard state only during drawing dictations and
does not read typed text. Without access, drawing stays off unless you explicitly
enable **If keys can't be read, let drawing take the keyboard**; that fallback
pauses typing while recording. Normal dictation needs none of this setup.

With draw lock on, clicks do not reach other apps and no pins are dropped.
Right-click undoes the latest stroke. Ctrl+Z also undoes a stroke, but reaches
the focused app too. Bind `koett draw-lock` in your desktop if you need to keep
the lock shortcut out of the focused app. **Menu → Drawing → Undo Last Drawing**
removes the latest drawing or pin during recording.

**Include Selected Text in Pins** is on by default. Pin context depends on what
the target app exposes through accessibility. The page-link setup command in
Settings enables additional browser context; it is separate from keyboard access.

The target must accept image paste. The image sheet is pasted first, then
speech-linked text; paste support and focus checks vary by desktop as described
above. Images stay in memory until another completed dictation or app exit.
Captured screen content is part of the pasted image. Normal dictation and
drawing recognition stay local.

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
its ASR-only link inputs. The Rust build rejects missing ASR-only receipts and
mismatched capability markers. Without a capability marker, developer builds
use spelling-only dictionary correction; packaging requires a receipt matching
the reviewed contextual decoder. Rebuild old native caches before packaging.
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

Linux ARM64 archives pass native build and inference CI; ARM64 desktop use is untested.
