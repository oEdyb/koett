# Koett

Dictate while drawing on your screen. Hold Shift and move to sketch, or Shift+click to pin what you mean. Koett pastes a numbered image sheet and your words into your app.

Voice-to-text runs locally. Drawing is optional and off by default; normal dictation needs no screen access.

Choose your microphone, set your shortcut, and use hold or toggle recording. Dictation stays local. No account or telemetry.

**Mac:** English · Apple silicon · macOS 15+. **Linux:** native GTK preview, with English and multilingual models; drawing requires a supported Wayland desktop. **Windows:** source builds in testing; no drawing download.

**Download status:** the published Mac v0.1.1 and Linux v0.2.0-linux.1 downloads do not include drawing. The drawing controls below describe the current source build.

## Get started

1. [Download for Mac](https://github.com/oEdyb/koett/releases/latest), unzip, and move Koett to Applications.
2. Open it, allow Microphone and Accessibility, and let the model download.
3. Press either Option key, speak, then press it again. Koett pastes and saves your text.

Change your setup from the menu bar. On Mac, Recording Limit offers 10, 30, 60 (default), or 120 minutes of recorded audio. A one-minute warning appears before Koett stops, pastes and saves normally.

On Mac, drag the pill's background to move it to any display. Its position is remembered across recordings and restarts. Buttons still click normally; if the display is disconnected, the pill returns on-screen.

Mac source builds include **Check for Updates…**. [Automatic updates](docs/macos-updates.md) stay disabled until a signed release includes its update feed.

For Linux, follow the [preview install instructions](CrossPlatform/README.md#install). Models, microphone levels, shortcuts and formatting are in Settings. If the microphone will not start, set **Microphone** to **System default microphone**; in this version, a named device cannot be shared with apps such as Discord.

[Source setup](CrossPlatform/README.md#build) · [CLI](docs/transcribe-cli.md) · [Issues](https://github.com/oEdyb/koett/issues) · [Contributing](CONTRIBUTING.md) · [Apache 2.0](LICENSE)

## Draw and pin while dictating

- **Mac:** menu bar → Draw on Screen → **Draw & Pin While Dictating**. Allow Screen Recording when asked; Accessibility is also needed for paste and pin context.
- **Linux:** Settings → Draw on screen → **Draw & Pin While Dictating**. Complete the [keyboard-access setup](CrossPlatform/README.md#drawing-setup) before recording.
- Start dictation, release any shortcut keys, then **hold Shift and move** to draw. Release Shift to finish a sketch. **Shift+click** drops a pin. Each new hold starts a separate sketch. The Draw Key setting can change Shift.
- **Linux draw lock:** press **Ctrl+Alt+D** while recording, then drag with the left button to draw multiple strokes in one sketch. Press the shortcut again to finish.
- **Mac draw lock:** press **Ctrl+Option+D** while recording, then drag with the left button to join multiple strokes into one sketch per display. Press again or press **Esc** to capture and leave draw lock; another Esc cancels dictation. Right-click undoes the last stroke. With VoiceOver on, use **Ctrl+Shift+D**; the Draw on Screen menu shows the active shortcut, including any binding conflicts.

Drawing pastes a numbered image sheet followed by speech-linked text. The target must accept image paste. Images stay in memory, so copy or paste them before quitting or completing another dictation. Captured screen content is included in the image you paste.

On Mac, Chrome's English Incognito window marker suppresses page details in pins. On Linux, pins keep only the app name in private windows of the Chrome and Firefox browser families that Koett recognizes. Other browsers aren't detected, including Vivaldi, whose private windows look the same as normal ones. Their private windows still put the title, URL and selection in the pasted text. For those, turn off **Include Page Details in Pins**. The screenshot still includes the content you capture.

Linux drawing has private-lab verification on KDE Plasma 6 Wayland, niri and GNOME Shell 51 with the Koett extension. Sway and Hyprland use supported capture paths but have less verification. X11 has a lab-checked overlay that needs a compositor. Multi-monitor drawing is not hardware-verified. Dictation remains available without drawing. See [Linux requirements and limits](CrossPlatform/README.md#drawing-setup).

Linux ARM64 archives pass native CI; ARM64 desktop use is untested. GNOME Wayland automatic paste needs the explicitly enabled [GNOME integration](CrossPlatform/integrations/gnome/README.md), a desktop shortcut and portal permission.

## Third-party software

Mac automatic updates use [Sparkle 2.10.1](https://github.com/sparkle-project/Sparkle),
under MIT with bundled BSD-2-Clause and Zlib notices. Its [complete licence](Packaging/Notices/Sparkle-LICENSE.txt)
also ships inside Koett.app. See [third-party notices](CrossPlatform/THIRD_PARTY_NOTICES.md).
