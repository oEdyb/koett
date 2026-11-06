# Koett

Dictate while drawing on your screen. Hold Shift and move to sketch, or Shift+click to pin what you mean. Koett pastes a numbered image sheet and your words into your app.

Voice-to-text runs locally. Drawing is optional and off by default; normal dictation needs no screen access.

Choose your microphone, set your shortcut, and use hold or toggle recording. Dictation stays local. No account or telemetry.

**Mac:** English · Apple silicon · macOS 15+. **Linux:** native GTK preview, with English and multilingual models; drawing requires a supported desktop and its capture setup. **Windows:** native preview; installer signing is not yet configured.

**Download status:** v0.3.0 includes drawing in Mac ZIP and Linux archive downloads. Native installers below are built by manual CI until a release publishes them.

## Get started

- **Mac:** download the signed, notarized **DMG**, open it and drag Koett to Applications. Open Koett and allow Microphone and Accessibility.
- **Linux:** download your distribution's **.deb**, **.rpm** or **Arch package**, install it with the package manager, then open Koett from Applications. See [native install and upgrade](docs/native-installers.md#linux).
- **Windows:** run **Koett-Windows-x86_64-Setup.exe**, then open Koett from Start. The installer is currently unsigned; [SmartScreen and uninstall details](docs/native-installers.md#windows).

[Downloads](https://github.com/oEdyb/koett/releases) · For the currently published v0.3.0, unzip the Mac ZIP and drag Koett to Applications; Linux archive users follow the [archive install steps](CrossPlatform/README.md#archive-install).

Press the dictation shortcut, speak, then press it again. Mac uses either Option key; Linux and Windows use Ctrl+Shift+Space. Koett pastes and saves your text. Models, microphones and shortcuts are in Settings (menu bar on Mac).

On Linux, if the microphone will not start, set **Microphone** to **System default microphone**; a named device cannot be shared with apps such as Discord in this version.

[Source setup](CrossPlatform/README.md#build) · [CLI](docs/transcribe-cli.md) · [Issues](https://github.com/oEdyb/koett/issues) · [Contributing](CONTRIBUTING.md) · [Apache 2.0](LICENSE)

Pull requests don't run CI automatically. A maintainer runs it on outside PRs before merging.

From **0.3.1**, Mac installs and private Linux tar installs include signed automatic updates. **0.3.0 has no update feed**: install 0.3.1 manually first. Debian, RPM, Arch and Windows installations do not auto-update; use the package manager or run the newer installer. See [update ownership](docs/native-installers.md#update-ownership-and-existing-private-installs).

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
