# Native installer verification

## Linux package layout and lifecycle

The native package builder wraps the same verified system-runtime payload in
Debian, RPM and Arch metadata. Packages own `/usr/bin/koett`, `/usr/lib/koett`,
the desktop entry and icon. User data is outside those paths.

Local checks on 2026-11-06 used the published v0.3.0 x86_64 payload from source
`115d6907a76c3c0b3f2696797a1b5bd0ddce67c9`, SHA-256
`192f412774250a2e2f57e2535625180ef98223f751503cdee31a90a10e301417`.
They test the new package layout with real application bytes; they are not a
claim about a later CI artifact.

- **PASS — Ubuntu 22.04 `.deb`:** actual dpkg install, root-owned launcher/icon,
  ordinary-user desktop-entry launch through GIO, model Ready, revision 1 → 2
  upgrade, second desktop launch, synthetic History/settings preservation,
  database integrity, package-manager ownership and uninstall retaining data.
- **PASS — Fedora 45 `.rpm`:** actual DNF install with dependency resolution,
  and the same ordinary-user launch/upgrade/uninstall checks. Dependency names
  use shared-library capabilities so Wayland package splits do not break them.
- **PASS — Arch `.pkg.tar.zst`:** actual pacman install and the same lifecycle
  checks in an Arch container with GTK 4.24.1.
- **PASS — targeted builder tests (6):** release identity retained, altered payload
  rejected, absolute desktop command and non-versioned system layout checked.
- **PASS — supply-chain inventory:** `generate_supply_chain_inventory.py --check`.

All three desktop journeys used an isolated Xvfb/session bus and generic UID
10055. They opened the installed desktop entry rather than the extracted binary.
Models were copied and read-only. Root was used only for the package manager and
lab setup. No personal installation, desktop, clipboard or microphone was used.
These are packaging/lifecycle checks, not new dictation or performance cohorts.

## Update ownership

The existing Linux updater accepts only an executable in
`PREFIX/lib/koett/versions/ID/libexec/koett`. The system package path
`/usr/lib/koett/libexec/koett` cannot match this layout. Packages therefore retain
exclusive ownership; archive/private installs retain their existing updater.
This packaging change does not change updater code, keyboard groups, udev rules,
compositor bindings or permissions. Those permissions were not freshly granted
on physical hardware during these package checks.

## Wayland delivery

**PASS — installed Arch package on private niri 26.04:** ordinary UID1000 ran
`/usr/lib/koett/libexec/koett` from the native package, using the packaged launcher
and distro layer-shell/clipboard tools. A copied compact model and private
synthetic microphone recording produced a numbered 30,095-byte 1600×906 PNG and
linked text consumed by Chrome. The received image was visually inspected;
linked text matched the synthetic History row. The isolated lab was stopped.
The lab needed niri's CLI (as a normal niri desktop has) and read access to its
exclusively grabbed disposable virtual keyboard. No physical input was accessed.

## Remaining checks

- Final manual-CI native assets need their own identity and lifecycle receipts.
- ARM64 desktop hardware remains untested; packaging on a native CI worker does
  not establish desktop acceptance.
- GUI software-center installation is not promised or exercised; native package-manager
  installation and the installed application launcher were exercised.
- Mac DMG and Windows setup acceptance are recorded separately by their owners.
