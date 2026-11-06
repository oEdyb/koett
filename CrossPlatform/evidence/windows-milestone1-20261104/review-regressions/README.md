# Independent-review regressions

All four findings were confirmed in source and reproduced on the **old**,
unmodified CI artifact 23f8b75 in the real Windows 11 KVM guest. See JSON and
screenshots in baseline-23f8b75; this is failure evidence, not a fixed-app pass.
EXE SHA256 fdb8206d8a96f6db4d424d1fda7ca80c3e77c3225efe331038c1055a9283e19e.

## Reproduction

Host had 42 GiB MemAvailable before boot; own build jobs completed first.
Guest: 4 vCPU, 8 GiB, private guest-only input, 200% scaling. The existing lab
History DB was backed up as History.sqlite3.before-review, then replaced with
110 explicitly synthetic rows from the canonical schema. No host data changed.

- P1: the original editor was empty. With Control held, UIA Paste posted the
  recovery action; after 80 ms, guest mouse input activated a second editor.
  Control was released within the 500 ms modifier wait. Original stayed empty;
  second editor HWND 131888 / PID 3996 received `Desktop lab recovery`.
- P2: History foreground HWND 131432 / Koett PID 8536. Search held
  `selftargetmarker`. Fixture recording captured that target; recovery Paste
  changed Search to `Desktop lab recoveryselftargetmarker`.
- P2: without pointer hover, recording root exposed no action children.
  Therefore Pause/History/Cancel invocation checks were skipped, not passed.
- P2: search `Review pagination` returned 50 of 110 rows. No Load more control.

The regression client exited 1 with passed:false as expected. Per-step records
include foreground HWND/PID. The guest completed a full Windows shutdown and
the transfer server stopped after reproduction.

## Source fixes and verification

- Expected HWND now reaches the shared paste guard. After modifier release,
  the guard checks current foreground immediately before invoking SendInput.
  Clipboard retries therefore cannot bypass the final check. There remains
  the unavoidable OS scheduling interval between checking foreground and
  sending input; Windows SendInput has no destination-HWND argument.
- A foreground HWND is eligible only when its owning PID is nonzero and differs
  from Koett's PID. Capture, automatic delivery and recovery use this policy.
- Applicable virtual UIA actions no longer depend on hover. UIA dispatch may
  invoke hidden-hover actions; mouse hit testing still requires visible hover.
- Native History retains the keyset cursor. Load more appends older pages,
  keeps the selected transcript, disables during loading and at the end, and
  resets on search. Independent page/detail generations discard stale replies.

Local Linux: 398 tests passed, 19 ignored, plus the isolated child test; strict
all-target Clippy and release passed. Three portable paste-guard tests passed,
including a switch during modifier wait and zero input for changed/ineligible
foreground. Windows all-target metadata Clippy passed (native linking skipped).
Windows-only tests cover rejecting a real same-process HWND, unhovered action
masks, and History 50/50/10 pages with no duplicates and a fresh filtered query.
Those tests are compiled locally and execute in Windows CI. Inventory freshness,
formatting and diff checks pass. No new dependency or policy exception.

**The fixed linked executable has not run in the guest yet.** Run the committed
checks/review-regressions.ps1 against the next green CI artifact using the
retained 110-row fixture, then repeat ordinary exact Paste/changed-target checks.
PR remains draft pending this validation. M2 has not started. Mac checks were
unavailable on Linux; Mac/Linux application behavior was not changed.
