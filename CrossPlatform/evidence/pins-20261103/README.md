# Linux pins: private lab evidence, 2026-11-04

Stock niri 26.04 nested inside private `agent-lab koett-pins` (one 1280x800
output, because nested niri has only one), debug build, a 110M model copy, a
private mic. Fixture windows log every press they receive (`lab-app.py`), niri's
focus events are logged (`lab-niri-events.py`), and `lab-pin-script.c` drives
pointer and keys. The draw key in device mode came from
`../sketch-20261102/lab-grabbed-shift.py`, started inside the lab session and named
in `KOETT_TEST_DRAW_KEYBOARD`. Earlier runs here started it from the host side, which
a Koett recording on the host could also see; do not do that.

- `final-regression.txt`: the merged design on a release build, typing free. Pin,
  double clicks, ink, capital tap, key lifted before the button, flick, jitter, drag,
  a click with no key as the baseline (which should reach a fixture), real Chrome on X11 through xwayland-satellite as on Olle's desktop, and
  four pins at each of seven rhythms. Labels were correct; a selected password gave
  no `selected:` line and never appeared in any log. One press beyond the baseline
  reached a fixture: the second click of the 180 ms double click, the known limit
  below. The text still landed in Target.
- `double-click-sweep.txt`: double clicks at 10 ms steps in the slow nested lab. One
  gap of nine, 140 ms, still let the second press through; focus returned, so
  the paste was not misdirected. Lab scripts ran at low priority on a loaded machine.
  The run stops at the lab's accessibility failure; later lines are not results.
- `device-mode-scenarios.txt`, `chrome-wayland-scenarios.txt`,
  `chrome-x11-scenarios.txt`, `frames-full2.txt`, `frames-partly2.txt`: earlier runs
  before the input hole. The frames were sampled with grim in focused-overlay mode;
  changes at x 532 to 776 are the pill. They measure the focus restore, which is
  unchanged: focus away 16 to 17 ms, nothing moving on a fully visible column, one
  short shift on a partly off-screen one.
- `two-output-kwin.txt`: a second KWin with two virtual outputs inside the lab, before
  the input hole. Koett's log showed one capture attempt per Shift-click on each
  output, so pins registered on both. That KWin has no capture protocol and the
  fixtures were not under the clicks, so this does NOT show images or stray-click
  safety on 2 outputs.

Not covered: real niri with two monitors, because nested niri has one output.
