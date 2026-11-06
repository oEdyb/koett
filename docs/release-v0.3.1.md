# Koett 0.3.1 candidate

Koett 0.3.1 adds native installers and expands drawing, recording and media tools across platforms.

- **Mac:** open the signed, notarized DMG and drag Koett to Applications. The recording pill can be moved, and drawing has a multi-stroke lock with undo.
- **Linux:** install a `.deb`, `.rpm` or Arch package with your package manager. The package includes the launcher and icon, preserves personal data on upgrade/uninstall, and keeps package-manager files separate from the private updater.
- **Windows:** run the native Setup.exe, then open Koett from Start. The per-user installer needs no admin access and supports removal through Installed apps. Drawing, pins, dictionary and media tools are included.
- Drawing and recording feedback now support GNOME Shell 51 through the bundled integration, and X11. KDE Plasma 6 and niri remain supported. GNOME 46 supports guarded text paste; newer drawing/pill APIs are unavailable there.
- Recording sounds now default to **off** on Mac, Linux and Windows. Mac and Linux have a Settings volume slider; Windows has volume choices in Dictation options. Volume defaults to 50%; Mac allows 0–100%, and Linux/Windows allow 1–100% while sounds are enabled. The start cue has been softened.
- Choose a recording limit of 10, 30, 60 or 120 minutes. The default is 60 minutes; a warning appears before recording stops and text is saved.
- Transcribe local media files and supported URLs. Dictation, drawing and media recognition remain local; optional media conversion tools and formatting runtimes have their own setup requirements.

### Install and update limits

- Linux ARM64 is a CI-tested preview; ARM64 desktop use remains untested on hardware.
- Windows setup is unsigned. SmartScreen may report an unknown publisher or block it; organization policy may prevent running it. Checksums detect corruption but do not replace signing.
- On Linux, a named microphone cannot be shared with Discord or another app in this version. Choose **System default microphone** if recording cannot start.
- Native Linux packages update through the package manager. Installing a local package does not add a repository or arrange automatic package updates. Linux tar installs include signed automatic updates. Mac installs include Sparkle automatic updates. Native Linux packages and Windows do not auto-update. Existing 0.3.0 installs have no feed and must install 0.3.1 manually to get these updates.
- Drawing captures screen content into the pasted image. Receiving apps must support image paste; Copy and History preserve text when automatic paste fails.

Hardware-specific gaps and the exact source/asset identity are recorded in the accompanying verification receipt.

## Candidate preparation

Build all assets from one frozen source commit: Mac version 0.3.1 (build 4), Linux and Windows version 0.3.1. Run the manual cross-platform workflow on main with `clean=true`. Build and notarize the Mac DMG from that same commit. Keep final asset hashes and verification receipts outside the frozen source.

Stage and verify a private draft first. Enable the signed production feeds for Mac and Linux tar installs. The draft must contain the notarized Mac DMG and Sparkle update ZIP/appcast, Linux tar archives and signed feeds for both architectures, Linux Debian/RPM/Arch packages, and Windows Setup.exe, with checksums. Verify each source revision and download every attached asset back to verify its checksum. A successful CI run does not prove desktop or hardware acceptance. Before publishing as Latest, prove a real higher-version fixture update and rollback in isolated Mac and Linux tar installations. Verify all assets and feed targets, then remove the superseded draft. After publishing, download every public asset back, compare checksums and verify all latest/download feed redirects. A missing feed must leave the installed app usable.

Outstanding hardware limits include Linux ARM64 desktop use, subjective sound-cue acceptance, and physical Windows hardware, signing, Internet SmartScreen and Chrome acceptance. Final Mac hardware limits come from the Mac artifact verification receipt. These are not represented as passing checks.
