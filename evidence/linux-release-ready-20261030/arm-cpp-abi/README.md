# ARM native C++ ABI correction

The failed ARM job passed compilation/tests/lint, then aborted during real model
self-test. Exact published archive inspection confirms ARM uses the new string
ABI while x64 uses the old one; the builder previously forced the old ABI on both.
The actual Status::EmptyString symbol distinguishes the pinned runtime layouts.

The builder now selects the matching ABI per supported target and checks the
actual archive before emitting a valid marker. Unknown or mixed symbols fail;
failed staging revokes its prior marker. Existing x64 input hashes and marker
remain identical. The earlier test asserting ABI0 for ARM encoded an incorrect
assumption and is corrected against the verified archive rather than relaxed.

Whether this correction resolves the ARM abort still requires native CI.
model_ready alone does not locate the failure: WAV reading/freeing precedes
recognizer initialization. Self-test stage markers and a failure-only GDB stack
separate those boundaries if the next attempt fails. Required inference and
success-only release upload remain enforced; diagnostic artifacts are separate.

Archive hashes, upstream static producer identities and actual guard results are
recorded alongside. Two independent source reviews found no actionable issue.
No ARM execution, physical audio, desktop interaction or installed-app update
was performed locally.
