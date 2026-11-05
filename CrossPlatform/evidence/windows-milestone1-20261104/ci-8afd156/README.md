# Final M1 review-fix retest

The unmodified CI artifact for head 8afd156cc3a42ed8d42e52e215ac7397c16957da
([run 41917706221](https://github.com/oEdyb/koett/actions/runs/41917706221),
artifact 13367051602, tested merge e9dbad5d2d0283b568d18a2e20afbfc129c795e4)
passed all four independent-review regressions in the real Windows 11 guest.

ZIP SHA256: 77c1751116a505e7232d4139456b1c02756f9b91d68e22e85d67d15487f3670e.
EXE SHA256: 53df56021a21411426535337df16457a887075a45b312206763ea172d596454b.
Host had 38 GiB MemAvailable before boot, no own builds. Guest 4 vCPU/8 GiB.
The first serial request preceded the bridge; inspection confirmed no install
or process before retrying. The verified installed executable ran as PID 5156.

- Modifier-wait foreground switch: both isolated editors remained empty;
  second editor HWND 131468 / PID 8552 retained foreground.
- Same-process guard: History Search retained exactly `selftargetmarker`;
  foreground History HWND 262172 / Koett PID 5156.
- No hover: Cancel, Pause, History exposed; Pause/Resume, History and Cancel invoked.
- History pagination: the retained 110-row synthetic fixture loaded 50/100/110.
- Ordinary UIA recovery Paste inserted exactly `Desktop lab recovery` into a
  verified blank editor. All samples retained editor HWND 66348 / PID 4772.
- Switched-target refusal left both buffers unchanged, second editor HWND
  66402 / PID 3028 retained foreground.

All three regression scripts returned passed:true. Per-step screenshots and
foreground HWND/PID are included. Earlier receipts cover the rest of the M1
matrix; these are silent desktop fixtures, not microphone/ASR results.

Guest completed full Windows shutdown; transfer server stopped. No host data
changed. PR #52 merged as fcaffcc9893c1d9bbf5d1e2ed5af3cc1b4f115b9 after the
successful retest; main had advanced without conflicts. These evidence files
are committed on the subsequent M2 branch to preserve the tested/green M1 head.
