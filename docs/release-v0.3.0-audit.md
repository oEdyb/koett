# v0.3.0 Mac first-install audit

Status: **NO-GO; audit in progress**. No GitHub release has been published.
Results distinguish the distributed app from tests of its source and dependencies.
A passing component test does not certify the corresponding first-install flow.

## Candidate and environment

- Current audit candidate: `ece9c25b3eb19acf5c11145ca6358926ca81a86a`, including
  the capture-failure warning. Version/build come directly from source: 0.3.0/3.
  SHA-256: `b1f628201e87c22531aee59b567aa951f3fc68893bf9876d8ae14e55986a2e7f`.
  Submission `a279c20d-ecec-46e2-aef5-bd8f3f723b65`: **Accepted**, stapled.
  A separate quarantined extraction passes strict codesign, stapler and spctl
  (`accepted`, `source=Notarized Developer ID`). Executable UUID:
  `F5C7EEDC-DA37-3225-B230-B9054B35E6A8`. Guest GUI results below are partial; this is not the final release artifact.
- Initial candidate: `eb6f2d6e896497d7e7e1c527aa8bba88ec23ee50`.
- ZIP: `Koett-0.3.0-macOS-arm64.zip`, SHA-256
  `48d5e1c3711ffc8cc403da235638114e9f7b5d1f07e805ff506d45ba8c07e020`.
- The initial candidate's staging metadata overrides version/build to `0.3.0`/`3`.
  [PR #92](https://github.com/oEdyb/koett/pull/92), merged as
  `61e121e13044a7eb1df8d4ebf3fb8b1a889884d3`, puts these values in source and
  includes drawing/pins in the existing Screen Recording purpose string.
- Developer ID: `Dyberg & Co AB (LF8KF3G42Q)`, hardened runtime, secure timestamp.
  Notarization submission `199a472d-2842-4b71-ba19-fa831ddc0412`: **Accepted**.
- Host: Apple M5, arm64, macOS 26.6.2 / `25G83`, 2026-11-05.
- Extracted the actual candidate ZIP into a new temporary directory, added quarantine,
  and launched its executable with `CFFIXED_USER_HOME` pointing to a fresh, short
  test directory. Foundation's resolved home and Application Support paths were
  checked before launch. No real transcript, preferences or clipboard text was
  used as a fixture. The installed app was not replaced.
- Clean Tart guest: macOS **15.7.7 / 24G720**, public test account `admin`,
  isolated from host clipboard, audio and display. The downloaded base image had
  Gatekeeper disabled; assessments were explicitly enabled before first open.
  Candidate ZIP copied to Downloads, extracted and moved into guest `/Applications`.
  Neither the installed host app nor its data was changed.
- The audit guest is stopped while a separate performance run owns the shared
  machine. Remaining runtime gates resume after its exclusive slot is released.
  Final packaging must use the final source commit supplied for the release.

## README-only route

The README says: download the latest Mac release, unzip, move to Applications,
open, allow Microphone/Accessibility, wait for the model, then toggle either
Option key to dictate. Configuration is in the menu bar.

The public `latest` download currently resolves to **v0.1.1**, which has no
drawing. The local candidate substitutes for that download in the tests below;
this is an explicit deviation from the stranger's current route.

## Results

| Check | Result | Evidence and limit |
|---|---|---|
| README download delivers drawing | **FAIL** | `gh release view v0.1.1`: current public Mac ZIP is the August v0.1.1 artifact. Publication remains deliberately pending. |
| Actual ZIP integrity | **PASS** | Extracted candidate ZIP; checksum above. App version 0.3.0/build 3 and source stamp match the candidate. |
| Developer ID signature | **PASS** | Extracted app: `codesign --verify --deep --strict --verbose=2` returns 0; arm64, team LF8KF3G42Q, hardened runtime and timestamp verified. |
| Notarization ticket | **PASS** | Extracted app: `xcrun stapler validate` returns 0; `codesign -d --verbose=4` reports `Notarization Ticket=stapled`. |
| Gatekeeper assessment | **PASS** | Quarantined extraction: `spctl --assess --type execute --verbose=4` returns 0, `accepted`, `source=Notarized Developer ID`. The clean macOS 15 guest also passed assessment with Gatekeeper enabled, then displayed the normal first-open confirmation and launched after approval. |
| Move into `/Applications` in a clean guest | **PASS** | Actual candidate ZIP extracted and moved into guest `/Applications/Koett.app`; normal first-open confirmation approved and process launched. Host installed app preserved. |
| First GUI process launch from ZIP | **PASS** | Extracted executable remains alive; isolated `--control status` returns exit 0, `ok:true`, `model:parakeet`, `phase:loading`, `pasteAccessGranted:false`. Stops at missing access; not a ready/dictation pass. |
| Permission onboarding layout/state | **PASS** | `PermissionsTests` pass; six offscreen light/dark first-launch, relaunch-needed and allowed renders inspected. These are fixtures, not system dialogs. |
| Microphone allow, deny, later recovery | **NOT TESTED** | Real guest denial settles to Open Settings; granting through Settings produces Quit & Reopen and TCC changes to allowed. Actual capture after recovery is still pending. An unexpected early microphone prompt before onboarding with BlackHole selected remains under investigation. |
| Accessibility allow, deny, later recovery | **NOT TESTED** | Guest Allow opens the actual Accessibility pane and denied/ungranted state settles to Open Settings. Natural grant plus event delivery is still pending. No new host grant was requested; a trusted flag alone does not prove delivery. |
| Screen Recording allow, deny, relaunch | **NOT TESTED** | Row/relaunch tests pass. No host capture grant or monthly picker-bypass prompt was triggered for testing. |
| Cold default model download and inference | **PASS** | Pinned FluidAudio through `parakeet-baseline`, fresh test home/model directory: download/load/warm-up succeeds; generated 3.32 s speech transcribes exactly. This bypasses the app's onboarding/UI. |
| Failed network and Retry Setup UI | **NOT TESTED** | Dependency probe against refused localhost endpoint exits 1 with `Could not connect to the server`, preserving cache. Actual app error/retry UI still needs exercise. |
| Slow network progress, interruption/resume UI | **NOT TESTED** | Loader probe progressed through a throttled private proxy, retained a partial file after interruption, and subsequently loaded/transcribed successfully. Cached inference also passes with the registry unreachable. App UI is still untested; HTTP Range reuse was not independently observed. |
| First Option-key microphone dictation and paste | **NOT TESTED** | Source capture/delivery tests pass; no generated audio has yet completed this full flow in the clean GUI. |
| Draw & Pin option included | **PASS** | Exact label exists in extracted binary, production source and actual guest menu accessibility tree. Drawing while ready is still untested. |
| Drawing and pin capture/paste | **NOT TESTED** | Draw/pin/sheet tests pass and generated contact sheet inspected. Real ScreenCaptureKit and pointer path in the clean app remain untested. |
| Repeat native paste, isolated T3 editor | **PASS** | Real macOS clipboard, hidden Electron 44.10.1 and matching T3 editor/intake: automatic image then text; two native `webContents.paste()` repeats each retain both. Details below. |
| Reported image-only repeat in full T3 chat | **NOT TESTED** | Failure not reproduced; original failing clipboard was already replaced. Full ChatComposer state and desktop Cmd+V routing were outside the isolated editor test. |
| Settings/menu interaction | **NOT TESTED** | Mac configuration is menu-based. Actual first-user selection, persistence and relaunch are pending. |
| History UI and search | **NOT TESTED** | History model/search/retry tests pass; actual clean-user interaction remains pending. |
| v0.1.1 history/preferences preservation | **PASS** | Actual old ZIP checksum matches public v0.1.1. Fixture generated by its exact `c72f834` TranscriptStore source; actual candidate imports it. Two launches yield one identical searchable archive row; original Markdown, last transcript and preferences are byte-identical. |
| v0.1.1 permission/login-item upgrade | **NOT TESTED** | Old/new Developer ID designated requirements match; no real TCC/login-item migration was performed. |
| Uninstall preserves data | **PASS** | Removing only the test app leaves its History database and original files intact. Real Finder Trash and Login Item cleanup remain untested. |
| Intel Mac | **NOT TESTED** | Binary is arm64 only, consistent with README Apple silicon requirement. No Intel machine or actual rejection dialog was exercised. |
| macOS 15 runtime | **PASS (partial)** | Actual ZIP launches on 15.7.7, shows onboarding and opens privacy Settings. Full dictation/drawing remains untested; exact macOS 15.0 was not exercised. |
| macOS 26 runtime | **PASS** | Isolated launch/status and native clipboard/editor probes above. Full clean-user dictation/drawing is still untested. |

## Repeat-paste finding

Running T3 Router was 0.1.25, source `37ace3264c90`, which includes
`1d4662546a`. Its running process held the inspected ASAR; this was not merely
an on-disk version check. The original image-only failure remains unresolved.

The production clipboard probe leaves **one item** with UTF-8 plain text,
PNG, TIFF, and HTML containing both the embedded PNG and escaped text. A generated
1024×1024 image produced 3,696,861 PNG bytes, 4,929,290 HTML bytes and 61 plain-text
bytes. After automatic delivery, two native repeats expose `text/plain`,
`text/html`, and `Files/image/png`. Prompt bytes grow 61 → 122 → 183; three images
are accepted. No synthetic ClipboardEvent or user-window input was used.

This rules out an inherent single-item macOS format incompatibility for this
fixture. It does not rule out full-composer insertion/remount/draft rollback.
T3 needs a shared paste-event ID and metadata-only tracing at capture, Tiptap
insertion/onChange and post-attachment draft commit. Record types, MIME/byte
lengths, `keepText`, cancellation flags, editable/selection availability and draft
identity; never record clipboard contents.

## Paste timing baseline

[Host probe receipt](evidence/release-v0.3.0-mac/host-probes.json).
No timing policy has changed. Two generated fixtures used production staged
delivery and the matching T3 editor/intake in hidden Electron 44.10.1:

- Small image: image-to-text event-post gap **191.2 ms**; image paste intake to
  text insertion **199.8 ms**.
- 3.70 MB PNG: event-post gap **177.6 ms**; intake-to-insertion **177.4 ms**.
- Both retain image/text through two repeats. These are two fixture observations,
  not latency percentiles, CPU-load trials or visible screen presentation times.
- A separate generated, named-pasteboard fixture measured the production image
  wait with a read after posting: **161.5–167.4 ms** across three runs.
- Reading the image before the post timestamp caches its bytes. A later consumer
  read then produces no new provider callback, so the same code waits its full
  timeout: **1,668.4–1,672.8 ms** across three runs. This reproduces a slow path,
  not the original live T3 report. Clipboard-manager activity can create this
  ambiguity; readiness waits are a separate possible contributor.

Keep image-first order, focus checks and newer-copy ownership. Shortening an
unobserved-read fallback needs slow-reader and target-consumption evidence;
normal measured consumers already hand off near 200 ms.

## Fixes found during this audit

- [PR #92](https://github.com/oEdyb/koett/pull/92): source bundle version and
  Screen Recording explanation. 426 tests, 10 skips, release build and packaging
  checks pass. No new permission requests.
- [PR #98](https://github.com/oEdyb/koett/pull/98): failed or timed-out captures
  previously disappeared while successful text delivery showed ordinary Done.
  Session-scoped failure state now keeps an explicit warning, without discarding
  surviving images/text or overwriting Not pasted. Four regression tests pass;
  430 tests (10 skips), release build and packaging checks pass. Merged as
  `ece9c25b3eb19acf5c11145ca6358926ca81a86a`. Pill geometry and permission
  requests are unchanged. The current audit candidate includes this fix;
  the initial candidate is superseded.

- [PR #106](https://github.com/oEdyb/koett/pull/106): the floating permission
  window covered the real System Settings Quit & Reopen dialog. The permission
  window now uses normal ordering. In the same macOS 15 guest, the private test
  build's window is layer 0 and the OS modal is fully visible. 430 tests (10 skips),
  release build and packaging checks pass. Merged as
  `a764b51ad20408d61c550efee8a88cab4391a501`. This fix is not in the candidate ZIP.

## Guest evidence

Local receipts are retained under
`/Users/olle/dev/koett-release-artifacts/v0.3.0/ece9c25/guest-output/`:
`os-version.txt`, `codesign.txt`, `stapler.txt`, `gatekeeper-assessment.txt`,
`first-open-dialog.png`, `microphone-denied-settled.png`,
`permission-request-settled.png`, `ax-settings-after-mic-grant.png` (covered modal),
and `window-order-fixed-relaunch-dialog.png` (fixed modal).
The last screenshot uses private source `826c76c3791ca42bddbea000ee0dd64b9becd353`,
not a rebuilt release ZIP. No full capture or model download has run in this
natural-permission audit profile yet.

No retained host `Diagnostics/Paste.jsonl` was present when checked. Historical
missing automatic paste cannot be assigned a Mac cause from absent evidence.
The held-modifier wait and visible Not pasted recovery are already implemented;
physical Option and pill-stop trials in T3/Chrome still need runtime verification.

## Release gate

**GO** only for the measured signature/notarization, isolated launch,
clipboard representation, source tests and data-preservation claims above.

**NO-GO** for a drawing-focused public release until the clean GUI first-dictation,
permission-denial/recovery, model-network/retry, drawing/pin and repeat-keyboard
paste checks pass on macOS 15 and a current macOS release. The reported full T3
image-only failure still needs reproduction or live metadata evidence. Intel
support is intentionally outside the advertised artifact.

Checks requiring the user's live screen if not reproduced in isolation: full T3
chat Cmd+V and draft state, physical Option/Shift gestures, real microphone/device
handoff, monthly ScreenCaptureKit prompt, mixed-DPI/multiple displays and player
controls. No such interaction is claimed by the generated fixtures.

Follow-up requests after this audit: Mac drawing undo, actual permission
capability checks, and measured image-to-text paste timing, each in its own change.
