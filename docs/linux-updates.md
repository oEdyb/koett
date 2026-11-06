# Linux updates

Source and any build without all three settings below are **inert**: no thread, no
requests, **Check for Updates…** visible but disabled, and `koett check-updates` returns
`updates_disabled` with exit 1. No release, tag or feed exists yet, so nothing updates.

## Behavior once a build carries a feed

Same behavior as [Mac updates](macos-updates.md), through the versioned install layout
instead of Sparkle.

- First check a minute after launch, then every 24 hours. **Check for Updates…**,
  `koett check-updates`, the tray, the Koett menu and the GNOME extension check now.
- A check fetches one small JSON feed. An Ed25519 signature over its version, build,
  architecture, length and SHA-256 is verified before anything else in it is used.
- A newer build downloads quietly to `~/.local/share/koett/updates/`, kept only if the
  length and checksum match what was signed.
- It installs only when Koett is ready, not recording, transcribing or processing media,
  with no result pill up, no focused or modal Koett window and no unsaved transcript.
  It reserves that state first, so a new capture is refused with "Koett is updating and
  will be right back" rather than lost. A "Not pasted" pill holds the update back until
  it is dismissed or the next dictation replaces it.
- Before installing, the unpacked package must report the signed build number. Then the
  same `install.py` a person runs integrity-checks it and swaps `bin/koett` atomically.
  Koett restarts, reopening its window if it was open.
- The version it replaced stays. `koett rollback-update` swaps back and restarts, and that
  build isn't offered again; a newer one is. `koett update-status` reports state, build,
  pending version, last error and the version to roll back to.
- It keeps the running version and the one before, and removes older ones it has run or
  installed. Versions from before its first update stay.
- Only an install under `prefix/lib/koett/versions/<id>` updates. A development build or a
  distribution package is left alone, with the menu item disabled.
- Manual checks say the result once. Automatic ones are silent; `update-status` has any error.

## What switches it on

1. Make the production key on the machine that signs releases, once:
   `cargo run --release --example koett_update_sign -- keygen ~/.config/koett-update-signing/release/signing.key`
   (folder 700, file 600, refuses to overwrite). Back it up privately: losing it means
   installed builds can only be moved to a new key by a manual install. Never commit it or
   put it in CI logs. `koett_update_sign public KEY_FILE` prints the public half.
2. Build releases with these set, for each architecture:
   - `KOETT_UPDATE_FEED=https://github.com/oEdyb/koett/releases/latest/download/appcast-linux-x86_64.json`
     (or `appcast-linux-aarch64.json`)
   - `KOETT_UPDATE_PUBLIC_KEY=<the public key>`
   - `KOETT_UPDATE_BUILD=<a number higher than every earlier release>`
   Never set `KOETT_UPDATE_LOOPBACK_TESTING` in a release: it permits plain HTTP to this
   machine, and exists only for the lab.
3. Publish the archive and its feed on the GitHub release, as the release's assets:
   `koett_update_sign sign KEY ARCHIVE VERSION BUILD ARCHIVE_URL [ARCH] > appcast-linux-ARCH.json`
   where `ARCHIVE_URL` is the https link to that exact archive. Sign last, after the archive
   is final. `latest/download` follows GitHub's latest non-draft, non-prerelease release.

The first release with a feed is installed by hand; everyone on it updates from then on.
Feed names follow the Mac's `appcast.xml`, one per architecture since each is its own package.
A Linux feed can't be read as a Mac one, nor the reverse.

## Limits

Downgrades aren't offered: the build number must rise. A signed feed can be replayed by a
network attacker only as far as an older build the client already refuses, or held back to
keep clients on the current one; HTTPS and the 24-hour check don't prove freshness.
`install.py` runs from the signed archive with your user's rights, so the signing key is the
whole trust boundary. There's no delta download: each update is the full archive.
