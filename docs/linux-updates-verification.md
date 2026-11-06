# Linux updates: verification

Lab only, on 2026-11-06, at source `50e406d`. No release, tag or live feed was made, and
nothing was installed on the host. Three system-runtime packages were built from the same
source: lab builds 100 and 101, and one with no settings. A local Python server played the
feed, and real speech went through the real model.

The packages used a receipt copy with the shared-context hash filled in, because this PC's
native libraries predate that field. That only passes the packaging gate for this lab; the
code under test doesn't touch it.

## Nested niri: 66 checks, all pass

- **Inert build:** reports disabled, `check-updates` and `rollback-update` return
  `updates_disabled` with exit 1, no request reached the server and no updates folder appeared.
- **Refused, install unchanged, nothing kept:** flipped signature, wrong key and an edited
  build number, each before the archive was requested. An archive whose bytes differ from what
  was signed was fetched, then discarded. A correctly signed feed claiming build 102 for a
  package that is really 101 was unpacked, then refused. An older build wasn't fetched.
- **Update during a recording:** `check-updates` sent while recording. State went downloading,
  then pending. 79 samples up to the transcript save showed the same process, the same
  versions and the same launcher. The speech was saved and pasted, then build 101 installed
  and restarted by itself: one process, a new pid, the old version kept, the archive removed.
  Dictation worked on the new build.
- **Rollback:** refused while recording (`busy`). Once idle: back on build 100, launcher swapped,
  one process, dictation works, 101 isn't offered or downloaded again.
- **Automatic:** with an old check time and no commands, build 101 was running 60.7 s after launch.

## KWin: a Not pasted pill holds the update

Paste fails there, so the pill stays up. The update stayed pending for 25 s with the pill on
screen and the process and launcher unchanged, and a new capture was still admitted. Cancelling
that capture cleared the pill, and build 101 was running 1.3 s later.

That run first took more than 25 s to come back: the restart script's `kill -0` counts a zombie
as alive, and the lab, like some launchers, didn't reap its child. It now reads the process state.

## Private GNOME Shell 51.1: 7 checks, all pass

**Check for Updates…** is listed, greyed out on an inert build while other items stay enabled,
enabled on a lab build, and clicking it reached the feed. The first version of this check found
the item stuck grey after a restart, because the action group's cache described the old process.
The extension now asks the running app each time the menu opens.

## Not verified

- A real GitHub Releases download and its redirect, since no feed exists. Only HTTP on loopback ran.
- The KDE tray and Koett window menu items beyond the action wiring and tray test.
- arm64, X11, and a window left open across an update's restart.
- Pruning beyond two versions has unit tests only.
