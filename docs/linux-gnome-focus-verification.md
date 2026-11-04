# GNOME original-window identity

## Result

An optional Shell 51 extension now supplies the actual focused window to Koett's
existing paste guard and per-app image chord selection. This is the first GNOME
integration PR, separate from its still-missing recording pill, drawing/input,
capture and pointer-context work. Setup is explicit, never auto-enabled; see
[the integration guide](../CrossPlatform/integrations/gnome/README.md).

The operation authenticates the caller's Unix PID against Koett's current GTK
application bus owner, rechecks that owner, and excludes own/desktop/dock windows.
Lock/session mode, overview and Shell modals yield no target. The Rust client
queries anew, pins the unique Shell bus owner, never recycles window tokens and
has a 250 ms deadline/bounded queue. Failure returns no cached target. Existing
Copy/History/manual paste remains the fallback. Focus checking is not an atomic
paste receipt and cannot prove arbitrary application consumption.

## Fresh native evidence — 2026-11-04

GNOME Shell/Mutter 51.1 ran headlessly with a real virtual monitor inside the
single private `koett-gnome-sol` lab. Sixteen native packages were extracted into
an unprivileged read-only `/usr` overlay, with archive SHA-256 matched against
the corresponding installed pacman repository metadata. No system package,
host extension, host setting or host desktop input was changed. Session bus,
Wayland and accessibility sockets were private to the nested runtime.

Two actual GTK application processes used the same title. The production Rust
focus function returned distinct tokens/app IDs, then returned the original token
after real private Alt-Tab. Shell overview returned no target and closing it
restored the correct identity. An unrelated D-Bus caller returned no metadata
both before and while Koett's application name was owned by the native Rust probe.
Disabling the real extension removed the endpoint; the next production check
returned no target. Re-enabling it restored the same live-window token.

Ordinary calls took 1–2 ms. Suspending only the verified private Shell process
forced a no-target return at 250 ms. After resuming it and switching windows,
the next check returned the new focus, not a late/cached old result. Ten native
samples passed. No virtual Shift or host clipboard/pointer/key input was used.

Rust suite: 398 passed, 19 ignored, plus the native ABI subprocess check. The
new ignored GNOME test then passed explicitly against the running compositor.
`cargo fmt --check`, warnings-denied all-target clippy, release build and diff
check passed. MSVC check remains blocked on this Linux host by missing Microsoft
`lib.exe`; it is not reported as passed. No independent model review was run.
Evidence: `CrossPlatform/evidence/gnome-focus-20261104/`.

## Limits and next work

The headless session has no lock-screen object. Native lock/unlock, real-login
lifecycle, screen reader, GNOME portal grants and actual image/text consumption
remain acceptance work. Lock/modal rejection exists in source but was not live
verified. The GTK fixture reported private AT-SPI activation warnings; these do
not affect Mutter window identity and provide no accessibility-provider proof.
No screenshot, ink, pointer-context or per-app image consumption is inferred.
Only Shell 51 is declared compatible. Other Shell versions require verification.

The production extension exposes no Eval, focus-changing, input, screenshot or
file-writing method. No browser, model or live user data was used by this test.
The host app was separately upgraded to merged main `d899540` while idle; running
binary SHA-256 matched and all four data/settings files stayed unchanged. The
host niri compositor was not replaced; tiled exact-link tests require the
explicit optional niri adapter. Stock niri keeps the documented fallback.

## Reproduce

Check free memory, then use the documented four-job/nice/12 GiB build scope from
`CrossPlatform/`. Copy evidence helpers into `target/gnome-sol/`. Stage the listed
native packages under its `root/` without installing them, merge host/package
schemas into `schemas/`, and install only the production extension under the
private lab's XDG data directory. The archived paths are local evidence paths;
adjust them to the current checkout before starting.

Start `koett-gnome-sol` and run `start.py` inside it. It checks the exact lab name,
uses a private session bus and headless Wayland display, and writes its environment
to `runtime/session-env.json`. Build the Rust tests, then run `probe.py` from the
lab controller. The probe checks the private runtime/display and Shell cgroup
before sending private remote-desktop input or suspending that compositor.
It runs the production ignored Rust probe with a 90-second bound and tears down
its fixture processes. Stop only this private lab after collecting evidence.
