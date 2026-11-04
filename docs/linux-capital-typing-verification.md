# Capital typing and provisional ink

## Cause and change

A Shift hold can pass 120 ms before a capital letter is pressed. Mouse drift
can exceed the 12 logical-pixel width/height save gate in that interval. The
three-pixel preview threshold is smaller still. A deliberate Shift-click is a
pin and does not need to pass the ink distance gate, so an observed numbered
mark alone cannot distinguish a deliberate pin from unintended ink.

Previously, detecting another key spoiled the draw hold and released the
pointer, but also closed its unfinished ink as a completed sketch. The fix
discards unfinished ink before ending an interrupted hold. Already completed
sketches and explicit click pins stay. Shift released normally still completes
a short sketch with the same 120 ms arm and 12 px extent thresholds.
Keyboard access remains state-only: no key event history, text or key logging.

## Regression checks

The native test in `CrossPlatform/evidence/capital-typing-20261104/check.py`
uses the existing named private Shift lab, grabbed virtual keyboard and real
GTK editor. It moves the private pointer 12, 16 and 24 px after a 160 ms hold,
then types a real capital `A` into the editor. Before the fix, all three cases
retained one unwanted sketch. The fixed path must retain none and still type
`AAA`. Separate 12/16/24 px sketches completed by releasing Shift must all remain.
A deliberate early click followed by capital typing must retain its pin.
A Rust boundary test also keeps the existing 12 px save rule explicit.

Use the existing early-Shift lab helpers/isolated copied model described in
`linux-early-shift-verification.md`. Copy this test to
`CrossPlatform/target/shift-sol/capital-check.py` and run it through the same
four-job/nice/12 GiB scope in `koett-shift-sol`; append `baseline` only for the
old binary. Do not use host input, clipboard or a live model directory.

## Fresh result — 2026-11-04

Native regression passed: all three typing-interrupted sketches removed, actual
`AAA` entered, all three 12/16/24 px Shift-release sketches retained, and a
pin followed by another real capital retained. The final editor showed `AAAA`
and the one deliberate pin; its screenshot was inspected. Background capture
was a private fixed-image fixture, so this is gesture/editor acceptance, not
fresh compositor screenshot acceptance. No host input/clipboard/model was used.

The existing early-Shift native suite also passed: 16 clicks at 40–100 ms
with no underlying clicks, real capitals, shortcut tails, idle timeout and
keyboard removal. This preserves the separately documented pre-sample race.

Rust: 397 passed, 18 ignored, plus native ABI subprocess check. Formatting,
all-target clippy with warnings denied, release build and diff check passed.
MSVC check remains blocked by missing Microsoft `lib.exe` on Linux. Independent
review and physical hardware acceptance remain open. No host install was done.

## Limits

A clean Shift hold plus pointer drift, with no observed typing, is indistinguishable
from an intentional no-button sketch. Key state is sampled every 16 ms; taps
entirely between samples cannot be observed. This fix does not claim continuous
key-event monitoring, a universal early-click shield or a diagnosis of a
particular live pin without its event evidence. Real short sketches and deliberate
pins remain supported rather than raising distance/time thresholds to hide it.
