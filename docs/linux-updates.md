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
- A watcher from the old version then waits up to three minutes for the new one to have its
  window ticking and its model ready, and to stay up five more seconds. If it crashes or
  hangs, the watcher stops it, moves the launcher back, marks that build skipped and restarts
  the old version, which says why once. Any failure before the restart also leaves the old
  version in charge: the desktop and icon files are written before the launcher moves, and
  the way back is saved before anything else.
- The version it replaced stays. `koett rollback-update` swaps back and restarts, and that
  build isn't offered again; a newer one is. `koett update-status` reports state, build,
  pending version, last error and the version to roll back to. Both work with no app
  running, from the state file, so a release that won't start can still be undone.
- It keeps the running version and the one before, and removes older ones it has run or
  installed. Versions from before its first update stay.
- Only an install under `prefix/lib/koett/versions/<id>` that this user can write updates. A
  development build, a system-wide install or a distribution package is left alone, with
  the menu item disabled.
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
   `koett_update_sign sign KEY ARCHIVE VERSION BUILD ARCHIVE_URL VALID_DAYS [ARCH] > appcast-linux-ARCH.json`
   where `ARCHIVE_URL` is the https link to that exact archive. Sign last, after the archive
   is final. `latest/download` follows GitHub's latest non-draft, non-prerelease release.
4. Sign the feed again before it expires, with no new build needed. Use 90 days. An expired
   feed stops updates, with the reason in `update-status`; installed apps keep working.

The first release with a feed is installed by hand; everyone on it updates from then on.
Feed names follow the Mac's `appcast.xml`, one per architecture since each is its own package.
A Linux feed can't be read as a Mac one, nor the reverse.

## Limits

Anyone who controls the feed or the network can serve any signed feed that hasn't expired.
Koett only takes a build newer than its own, but within that window that can be an older
release than the latest: at build 100 with 105 out, it can be handed a signed 101, and
then be held there. The signed `expires` limits this to the feed's lifetime, 90 days as above,
after which the old feed is refused. A minimum build wouldn't help, since a replayed feed
carries its own old minimum. A feed that's withheld can't be told from no new release until
the last one seen would have expired, and Koett doesn't remember that, so it just stays put.
`install.py` runs from the signed archive with your user's rights, so the signing key is the
whole trust boundary. There's no delta download: each update is the full archive.

Koett is gone for the seconds a restart and model load take, so a shortcut pressed in that
gap starts nothing. A logout or power loss during the watcher's wait can leave the new
version unproven; `koett rollback-update` undoes it. A restart that fails because the session
ended marks that build skipped too, until a newer one arrives. Each install step has a time limit, so a hung installer can't leave
dictation blocked. A manual check blocks nothing, but a rollback asked for during a download
answers `busy`.
