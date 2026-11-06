# Native installers

Native installers are built by the manual release workflow. They are not published
until a release is explicitly approved. Checksums accompany every installer.

## macOS

Open the signed, notarized DMG and drag Koett to Applications. Open Koett from
Applications, then grant Microphone and Accessibility; drawing also needs Screen
Recording. Upgrade by replacing the app while it is closed. Sparkle continues to
manage updates to that app. Remove the app to uninstall; personal data remains.

## Linux

Choose the package matching the distribution and CPU:

- Ubuntu/Debian: open the `.deb` with the software installer, or run
  `sudo apt install ./koett-VERSION-REVISION-amd64.deb` (`arm64` on ARM).
- Fedora: open the `.rpm` with Software, or run `sudo dnf install ./koett-VERSION-REVISION-x86_64.rpm`.
- Arch: run `sudo pacman -U ./koett-VERSION-REVISION-x86_64.pkg.tar.zst`.

Launch **Koett** from the applications menu. The package manager installs the
launcher, icon and application under `/usr`. Install the newer package in the same
way to upgrade, then restart Koett. Remove with `sudo apt remove koett`,
`sudo dnf remove koett`, or `sudo pacman -R koett`.
Settings, downloaded models, History and recovery text remain in your user data.
Turn off Start at login before uninstalling to remove your optional login entry.

### Update ownership and existing private installs

Native packages are owned by the package manager. Koett's private updater does not
replace system package files: `/usr/lib/koett` is deliberately outside its
versioned private-install layout. Upgrade these installations through the package
manager. Installing a local package does not add an external software repository,
so it does not by itself arrange automatic package-manager updates.

The archive installer remains available for private installations; its signed
updater continues to own only that installation's version tree. To switch from a
private archive installation, quit Koett, turn off its Start at login entry and
remove the old application files using the archive uninstall instructions before
installing the native package. Keep your settings/models/History. A private
launcher in `~/.local/bin` can otherwise shadow the system CLI, and a private
applications entry can shadow the new menu entry.

Packages do not change keyboard groups, udev rules, microphone selection or
compositor shortcuts. Existing keyboard-access setup remains separate. New users
still explicitly opt into drawing and grant its permissions. GNOME extension and
portal setup remains as described in the Linux README.

If a named microphone cannot start while another app uses it, choose **System
default microphone**. Native ARM64 desktop use remains untested until there is
hardware evidence; CI packaging alone is not desktop acceptance.

## Windows

Open **Koett-Windows-x86_64-Setup.exe** and follow Setup. Installation is per user,
requires no administrator access and creates a Start menu entry. Close Koett
before running the newer installer to upgrade. Remove it through Settings → Apps
→ Installed apps. Settings, models and transcripts are retained.

The Windows installer is unsigned until a signing certificate is configured.
SmartScreen may show “Windows protected your PC” and an unknown publisher, or an
organization policy may block it. Verify the download source and checksum before
choosing whether to allow it. Checksums do not replace signing or establish
SmartScreen reputation. A certificate can be added to the packaging path later.

## Distribution decisions

No external repositories are submitted automatically.

- Recommend keeping direct installers as the initial distribution path.
- Consider a Homebrew cask and AUR package after the installer releases settle.
- Consider winget after Windows signing is funded and configured.
- Defer Flathub until sandboxed hotkeys, paste and drawing permissions can support
  the same features without weakening the sandbox.

No account registration, purchase or submission is part of building installers.
