# M3 final Windows acceptance

All six [CI checks](https://github.com/oEdyb/koett/actions/runs/42028723040)
passed on c7e237685ab548c8b4b527257dc6ed985978140a. Windows executed 231 tests
(12 explicitly ignored), including the shared Swift/Rust dictionary fixture,
native multiline-tab navigation and everyday-warning/cancel regressions.
Windows unit execution is distinct from the local Windows metadata compilation.

The private Windows 11 KVM guest ran its linked executable, SHA-256
 ddf86b8deb4cebf253f6eb6de4d3accb855abd2c1240246a0574b22f51e1e382.
ZIP SHA-256: 77ef5e63d0bbc2545f033a8c2d1ff2f631e60c9b695d3f2513bce0f3d18b9593.
The guest stayed at 4 vCPU / 8 GiB, nice 19; no host input was controlled.
Every step has a screenshot and foreground HWND/PID receipt.

Final affected-case rechecks passed:

- Narrator running with forward keyboard navigation through every enabled
  Words/Model/Formatting/correction action; native accessible labels are correct.
- Shift+Tab moves from Save to the multiline field and out to its previous field,
  in both Words and Fix a word. Fields no longer trap keyboard navigation.
- Rapid UIA everyday selection followed by cached Add invocation does not adopt
  before the current warning is presented. It visibly asks for review first.
- Normal strict UIA correction Save, explicit Add anyway, Dismiss and denied-write
  preservation still pass; correction does not rewrite history or implicitly learn.

Combined with the [fd2e350 rechecks](../ci-fd2e350/README.md),
[b964d47 desktop/failure matrix](../ci-b964d47/README.md), and
[c5a243b import/export checks](../ci-c5a243b/README.md), M3's software acceptance
is complete. The earlier failure receipts are retained as regression evidence.
No product code changed after this final executable; subsequent commits add
receipts, docs and reusable check scripts only.

Limits: prerecorded CPU input is not physical microphone or long live-capture
proof. Broad Swedish accuracy, injected Nemotron-specific recovery faults and
CTC acoustic dictionary claims are not established by these focused checks.
Narrator keyboard/UIA exposure was checked; spoken pronunciation was not
independently transcribed. S1-mini used a private self-contained CPU runtime,
not a bundled Windows runtime; bundling/signing/clean-install remain M6 work.
No big benchmark or new default was introduced. Mac/Linux app behavior is
unchanged by the Windows-conditional shared Rust changes.

The original guest history and settings were restored with matching SHA-256
hashes (shutdown-restoration.json); prepared formatter children were absent.
A full guest shutdown followed, verified by guest_control reporting not running.
