# Linux updates

Archive installs update themselves from 0.3.1 on. The first one is installed by hand, because
earlier versions have no updater. `.deb`, `.rpm` and Arch packages share the same binary but belong
to their package manager, so the updater stays off there, as it does for development builds and
system-wide installs: **Check for Updates…** is visible but disabled, there are no requests, and
`koett check-updates` returns `updates_disabled` with exit 1.

## Behavior

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

## Releasing

CI builds carry the feed, the public key and the build number, set in the Linux job of
`cross-platform.yml`. The signing key is on the machine that signs releases, at
`~/.config/koett-update-signing/release/signing.key` (folder 700, file 600). It never goes in the
repo, CI or a log. Losing it means installed builds can only move to a new key by a manual install.
`koett_update_sign public KEY_FILE` prints the public half, which is what the workflow has.

For each release:

1. Raise `KOETT_UPDATE_BUILD` in the workflow above every earlier release. The Mac's build number
   is separate but they currently match, which is easy to keep: 0.3.1 is 4 on both.
2. Build with `-f clean=true`, and take the archives from that run.
3. Sign each archive last, once it's final. `ARCH` is `x86_64` or `aarch64`:
   ```sh
   cargo build --release --locked --example koett_update_sign
   target/release/examples/koett_update_sign sign \
     ~/.config/koett-update-signing/release/signing.key ARCHIVE VERSION BUILD \
     https://github.com/oEdyb/koett/releases/download/vVERSION/ARCHIVE_NAME 90 ARCH \
     > appcast-linux-ARCH.json
   ```
4. Publish each archive and its `appcast-linux-ARCH.json` as assets of the release. Clients read
   `https://github.com/oEdyb/koett/releases/latest/download/appcast-linux-ARCH.json`, which follows
   GitHub's latest non-draft, non-prerelease release, so a feed answers 404 until then.
5. Every Latest release has to carry all the feeds and the assets they name: both Linux ones and the
   Mac's `appcast.xml` and ZIP, even for a release that only changes one platform. Otherwise
   `latest/download` answers 404 and that platform stops updating.
6. Sign the feed again before it expires, with no new build needed, and replace the asset. Use 90
   days. An expired feed stops updates, with the reason in `update-status`; installed apps keep
   working. Publishing a new release also renews it.

The feed covers version, build, expiry, architecture, length and checksum. The archive's URL isn't
signed, because the checksum already decides which bytes are accepted. Schema:
`{"schema":1,"version","build","expires","arch","url","length","sha256","signature"}`.

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

Known rough edges, reviewed and left for later because each is rare and recoverable:

- A quit that takes over 30 s can still close the old app later, after the update was already undone.
  The next start is on the old version.
- With a full disk, saving state fails, and that also blocks a rollback, including the watcher's.
- A `rollback-update` answered `busy` stays queued and can run later.
- The watcher counts a model that loads into an error as not started, so it can roll back a good
  build when the fault is elsewhere. It's skipped until a newer one arrives.
- If restarting fails after the watcher moved the launcher back, the launcher stays moved and
  nothing is running. The next start is on the old version.
