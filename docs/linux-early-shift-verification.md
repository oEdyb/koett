# Early Linux draw-key clicks

## Behavior

The device-mode drawing overlay now protects clicks on the first clean draw-key
sample. A clean sample requires the key to have been seen released in this
recording and no other key/modifier to have spoiled the hold. The 120 ms delay
still applies to ink, not click protection. Before ink arms, the overlay keeps
the normal pointer cursor and never takes keyboard focus.

An early click can become a pin. Arming ink does not reset an in-progress click.
Early pointer movement is tracked so a quick drag cannot be mislabeled as a pin;
ink is not emitted before the hold arms. Typing or a shortcut spoils the hold,
returns pointer input and cancels an unfinished pin. The hold stays spoiled
until a real release, including the Shift tail of Ctrl+Shift+Space. Existing
15-second idle recovery, keyboard-removal handling, opt-in focus fallback and
physical-device-only filtering remain in place. No key-event stream is read;
held-key samples are still wiped after polling.

## Verification — 2026-11-04

Base: main `1c247fb` including #70 and #71. Rebased after monitor hotplug merged;
layout invalidation resets early click protection as well as ink and the hold
gate. The Rust, native Shift and hotplug checks were rerun together.
No dependency, Windows/shared-engine changes or host installation.

- Rust: 392 tests passed, 17 ignored, plus native ABI subprocess check.
  New tests cover protection before ink, typing cancellation until release,
  and a key already held when a recording begins.
- Formatting, clippy with warnings denied, release build and diff check passed.
- MSVC check is blocked by missing Microsoft `lib.exe` on this Linux host.
- Before the change, all three nominal 60 ms Shift-clicks reached a real GTK
  button beneath the overlay. The baseline helper includes a 100 ms settle
  after posting; its recorded completion time is not the click onset.
- With the change, 16 real clicks requested at 40/60/80/100 ms created one pin
  each and produced zero underlying GTK button activations. The fixture records
  CLOCK_MONOTONIC just after flushing each click, separately from its settle.
- A click pressed before ink arming and released afterward stayed one pin.
  A quick drag entirely before arming produced no pin/ink or underlying click.
- A real native GTK TextView received `AA`: one capital early in a hold and one
  after the ink delay. No drawing was created. Letter events used one persistent
  private KWin test-input connection so Shift stayed down through the letter.
- Shortcut-tail clicks passed to the app without drawing, including after Ctrl
  was released while Shift stayed down. Idle timeout and removal of the grabbed
  keyboard also returned the pointer. The final screenshot was inspected.

The model is a copied 110M model, audio is an isolated silent source, and capture
uses a known-PNG `grim` fixture. These tests prove input routing and bookkeeping,
not real screenshot cropping, image-paste consumption or dictation accuracy.
Only a grabbed named test keyboard and private KWin input socket are used;
no host keyboard, focus, pointer, clipboard or live models are manipulated.

## Remaining limit

This removes the deliberate 120 ms click gap. It does not make client-side
sampling and Wayland input-region commits atomic with the compositor's next
button event. Device state is polled on the existing 16 ms drawing tick, and
GDK applies the requested input region on a later surface commit. A click that
beats those steps can still reach the application. The fast-window diagnostic
in the evidence probes 0/10/20 ms separately: 3/3 clicks leaked near 0.8 ms,
2/3 near 10.9 ms and 0/3 near 20.9 ms in this run. These are measurements,
not a guaranteed deadline; they are not counted as the passing 40–100 ms sweep. The closest current alternative is to pause briefly after
pressing the draw key before clicking. Closing the remaining window requires
compositor input integration/acceptance; it is not declared impossible on Linux.

The parity row remains partial for that race, physical/multiple keyboards,
keyboard layouts/IME and other compositors. Tiled niri context geometry and
GNOME Shell overlay/input work are still the next deferred integrations.

Sources: [GTK surface input regions](https://docs.gtk.org/gdk4/method.Surface.set_input_region.html),
[Linux held-key state](https://docs.kernel.org/input/event-codes.html), and the
pinned GTK/GDK 0.11.5 bindings. No library upgrade.

## Reproduce

Evidence: `CrossPlatform/evidence/early-shift-20261104/`. Copy the scripts to
`CrossPlatform/target/shift-sol/` and a verified copied model into its `model/`.
Build `input.c` using the installed agent-lab's `build/fake-input-{client.h,code.c}`
and `-lwayland-client -lxkbcommon`, writing the executable beside the scripts.
Do not replace the installed input helper. The keyboard helper refuses any
runtime except this private lab and requires `KOETT_TEST_DRAW_KEYBOARD`.

Check free memory first. Build/test from `CrossPlatform/` with
`CARGO_BUILD_JOBS=4`, `nice -n 10` and a user scope with `MemoryMax=12G`.
From the repository root:

```sh
agent-lab start koett-shift-sol
systemd-run --user --scope -p MemoryMax=12G nice -n 10 \
  agent-lab run koett-shift-sol -- python3 \
  "$PWD/CrossPlatform/target/shift-sol/check.py" "$PWD"
# Same command with diagnostic appended measures the remaining fast window.
agent-lab stop koett-shift-sol --force
```

The `baseline` mode expects the old main binary and intentionally asserts that
all three clicks leak. Run it before applying this change. The native fixture
requires local agent-lab/KWin, Python GTK/Pillow/dbus, PulseAudio compatibility,
udev/uinput access for its exclusively grabbed named test keyboard and the
copied model. It refuses host input; no global shortcut is synthesized.
