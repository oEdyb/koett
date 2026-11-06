# Parallel audio and empty-result acceptance

Current unsigned local debug GUI SHA-256:
`e2a22b96a42ada15f94c88178e166f6dadfd9a8f10435439ef2c1f7c899d3e4a`.
`build-receipt.json` maps the exact source and test/console executables.

The Windows CI Test step at `a49ab9b` failed two tests: the existing preview
ownership assertion raced with the new standalone capture test. Both touched the
same global recording counter; the preview panic could also poison its mutex.
The original assertions are unchanged. A test-only lock now serializes those two
users of that global state; normal app capture is unchanged. No test was ignored
or removed, and no CI policy or workflow was changed.

`before-parallel.json` reproduces the primary failure in the guest with eight
threads: 21 passed, one failed and one pre-existing hardware/lab test ignored.
`after-units.json` records the same audio filter passing at eight threads:
22 passed, zero failed, that same one ignored. Including the other affected native
filters, 45 tests passed. Windows strict Clippy and native linking passed; Linux's
existing preview ownership regression also passed (one test, 0.51 seconds).

The empty parked-result route also needed drawing/cancel-hotkey cleanup after
returning Ready. It now calls the existing discard cleanup. Its native regression
registers the cancel hotkey ID with a private Ctrl+Shift+F11 chord, completes an
empty result, and verifies that unregistering the now-absent ID fails. Drawing
locks are false, the app is Ready, and the previous transcript remains intact.
It does not capture a person's actual Escape key during the test.

The parent receipts retain the earlier 40 serial tests and real GUI Quit, drawing,
Edge, error-recovery and preview-management checks. This last change affects only
empty-result cleanup and test synchronization, with exact current native checks
above. There is no new physical mic or browser-binary acceptance claim.

`after-shutdown.json` records original configuration restored, startup off,
no app/Narrator and requested shutdown; host status confirmed the VM off. The
obsolete CI run was cancelled after its Windows failure; a single combined push
carries both corrections. Remaining signed/ARM64/hardware release gates are
unchanged. No benchmark or full local suite ran.
