# Koett for Windows and Linux

This is the small native Windows and Linux app. It records with a configurable
toggle shortcut, transcribes locally, pastes into the focused app, and saves
every transcript.

The default shortcut is `Ctrl+Shift+Space`. The 110M English model downloads
once on first use and stays loaded while Koett runs. Long recordings process in
the background while you speak.

Windows has a notification-area menu and recording pill. Linux has a tray menu,
uses desktop portals on Wayland, and uses native X11 shortcuts and paste on X11.
The Linux build supports glibc 2.35 or newer and needs the system ALSA runtime.

Build the app with Rust 1.92:

```sh
cargo build --release --locked --bin koett
```

The separate `koett-engine` binary remains available for WAV and microphone
benchmarks.
