# M3 regressions on the main-updated b964d47 executable

Windows 11, 4 vCPU / 8 GiB, nice 19. Foreground HWND/PID and executable SHA-256
are in each receipt. Guest-only UIA and keyboard input; host input is untouched.

Passed: all four strict UIA correction checks (no keyboard selection fallback),
four Words CRUD/category checks, five selection/rename/collision/stale-edit/New
entry checks, denied model settings-write rollback, and forced Koett process
termination reaping its owned formatter child within three seconds.

Model cancellation preserved settings but failed the visible-status assertion:
the window stayed on Loading model after the candidate was discarded. A Windows
presentation fix and regression test follow this receipt; a linked re-run is
required. UIA inspection also found Style had Structure as its accessible name;
control creation order is corrected for native label association, pending re-run.

Not all M3 gates have passed. Settings were restored by each edit/failure script.
Original history remains backed up while the controlled test history is in use.

All three new windows also passed geometry/DPI checks at actual Windows display
scales 100/150/200%, in light and dark modes (18 captured steps). Theme and 200%
scale were restored. Formatter setup kept Notepad focus after the user resumed
typing; native button invocation itself activates Settings as expected.

The focused speech-rule check found a genuine failure: dictionary spelling
changed Phoebe to M3Phoebe, and original ASR was retained/copied from History,
but the literal replacement never ran. The Windows final-delivery-stage fix
requires a rebuilt re-run; its failure receipt is intentionally retained.

A second guest run rechecked full search export: 110 matches exported from 50
loaded rows, raw/final Copy, selected/filtered export, file-cancel preservation
and legacy raw fallback all passed. Missing formatter runtime, denied formatter
settings writes and setup cancellation all kept Raw mode; prepared children were
reaped. The first run's original settings/history were restored with matching
hashes and full shutdown before the normal native checks; the second run uses
another temporary fixture with the same original backup preserved.

Narrator was running during keyboard checks. The check found a real Tab trap
in multiline Words and correction fields; the native message loop now routes
plain Tab/Shift+Tab through enabled form controls. Style's known old label also
failed, as expected. A rapid UIA selection followed by cached Add invocation
adopted an everyday form before its warning updated; adoption now checks that
the current selection's warning was presented first. Both failures are retained
here, with fixes and Windows regression tests awaiting the next linked artifact.
Narrator's spoken output is not independently transcribed.
