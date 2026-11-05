# Linux v0.3.0 first-install audit

**Decision: NO-GO. No tag or release was created.**

Audit date: 2026-11-05 UTC. The functional results below are candidate evidence,
not acceptance of a final release. PASS applies only to the stated observation;
NOT TESTED includes invalid or incomplete harnesses. A broken lab is not a
product failure, and a startup test is not a complete first-install journey.

## Candidate identity and isolation

Latest repeated clipboard/drawing checks used the release-style system-runtime
archives from [CI run 41943679213](https://github.com/oEdyb/koett/actions/runs/41943679213),
source `f2c3238da6b0c8134c76eba410bc292f2d63f1d5` (the paste-latency merge candidate):

- `Koett-Linux-x86_64-system-runtime-f2c3238d.tar.gz`
- `Koett-Linux-aarch64-system-runtime-f2c3238d.tar.gz`

Both sidecar checksums and native receipt identities were checked. These archives
predate the merged provenance-path fix and still contain those paths. **Do not
publish them.** The final approved commit and freshly built assets remain a gate.

Earlier follow-up checks used source `54dc92865bc5b8cb94060b441e1c6e9406ac8cf0`
from [run 41940218945](https://github.com/oEdyb/koett/actions/runs/41940218945).
Production changes between those two candidates affect `linux.rs`,
`linux/clipboard_owner.rs` and `linux/output.rs`; model download, capture and draw
key modules are unchanged. Model/network evidence below refers to that earlier
candidate explicitly. Initial installer/adverse-condition checks used
`eb6f2d6e896497d7e7e1c527aa8bba88ec23ee50` from
[run 41936332855](https://github.com/oEdyb/koett/actions/runs/41936332855).
The upgrade input was the actual published v0.2.0-linux.1 x86-64 archive.

[Initial receipts](evidence/release-v0.3.0-audit/receipts.json),
[initial privacy scan](evidence/release-v0.3.0-audit/privacy-scan.json) and
[follow-up receipts](evidence/release-v0.3.0-audit/followup.json) distinguish these
sources. Public receipts contain hashes and observations, not raw screenshots,
OCR output, local home paths or History text.

No installed Koett, visible desktop, host clipboard, host microphone recording,
physical keyboard permissions or host group membership was changed. Tests used
fresh homes, private buses/audio, disposable containers and invisible compositors.
A named, grabbed virtual keyboard supplied drawing gestures; this does not prove
physical-keyboard setup on a normal login. Builds/checks used nice 15, two jobs,
an 8 GiB scope and cache output outside /tmp. No installed app was replaced.
The desktop lab and test containers were stopped before the performance audit
received the exclusive desktop slot.

## Desktop and architecture coverage

| Check | Result | Evidence and limit |
| --- | --- | --- |
| x86-64 and ARM64 archives | PASS | Latest archives match their sidecar hashes and source receipt. |
| ARM64 native CI | PASS | Both candidate runs passed the native ARM64 job, including native linking, inference and archive validation. CI is not desktop acceptance. |
| ARM64 desktop, all environments | NOT TESTED | No ARM64 desktop hardware/emulator journey. Release notes must state ARM64 desktop use is untested. |
| KDE Wayland lab validity | PASS | Rebuilt with identical fresh HOME/XDG for compositor, portals and app. Native permission dialogs work. The earlier alternate-HOME identity mismatch was a lab defect. |
| KDE from zero | PASS, limited | Archive installer into fresh HOME; first model download reached Ready; Settings and History exercised. Instrumented private seat, not a new physical-machine login. |
| niri | PASS, limited | Actual nested niri, private audio, real Chrome and kitty consumers. Model supplied in this session. Static screenshots work; screencast capability is not implied. |
| GNOME | PASS, limited | Rebuilt real GNOME 51.1 shell with private system bus/logind fixture and software renderer. App UI, Ready model and fixture ASR saving one row work. Drawing unavailable without disrupting dictation. |
| GNOME complete install/permissions/paste | NOT TESTED | Private logind fixture is not a display-manager login; full permission decisions, shortcut setup and portal paste remain open. |
| Plain X11 | PASS, limited | Fresh UID-1000 Xvfb startup, shortcut backend, offline and no-mic error handling. Full desktop workflow and unsupported-overlay acceptance remain open. |

KDE 6.8.2 screenshot capture initially returned a generic backend error because
the synthetic HOME lacked its Pictures directory. Creating the normal Pictures
user directory fixed the lab; no application workaround was added. Permission
Deny and Allow were exercised separately. This does not establish behavior on
every KDE distribution or on a real user's existing portal permissions.

## Functional workflow

Unless stated otherwise, desktop results are x86-64 only. None applies to the
unselected final tag. Latest means the `f2c3238d` candidate above.

| Check | Result | Evidence and limit |
| --- | --- | --- |
| Drawing off by default | PASS | Fresh KDE Settings shows unchecked drawing; ordinary fixture dictation works. |
| Enable through Settings | PASS | Native pointer toggles drawing; Save and reopen retain the enabled state. Other settings pages/mic switching are not exhaustively accepted. |
| Hold Shift and move; repeated holds | PASS | KDE and niri real pointer/key gestures produce separate sketches and captured PNGs. Uses grabbed test keyboard, not a physical keyboard. |
| Shift+click pins | PASS, limited | Latest KDE Chrome/Electron each receive a two-annotation sheet plus linked text and window context. Latest niri Chrome also consumes a sketch and pin. Selection-denied and all context permutations remain open. |
| Ctrl+Alt+D lock; undo; exit; stop | PASS, limited | Earlier candidate: actual chord locks; two drag strokes form one sketch; right-click undo and chord unlock; image/text consumed. Latest KDE repeats chord/drag/undo/unlock with one captured group and saved result; latest lock-specific consumer export was not separately retained. |
| Browser image/text paste | PASS, limited | Latest real Chrome fresh profile receives PNG then linked text. Repeat Ctrl+V exposes coherent HTML/plain/image bundle with identical image bytes and text. Custom paste target, not every browser editor. |
| Electron image/text paste | PASS, limited | Latest actual Electron app receives PNG then linked text; repeat paste preserves exact bundle. Custom editor fixture, not acceptance of every Electron application. |
| Terminal image/text paste | PASS, limited | Latest niri kitty raw TUI consumes a 30,655-byte 1600×906 PNG through its image-aware Ctrl+V handler; Ctrl+Shift+V text matches saved result. Restored clipboard PNG/text match exactly. A shell alone does not accept images. |
| niri fallback and clipboard restoration | PASS, limited | Actual Chrome consumption and earlier repeat-paste bundle checked; latest terminal verifies restored image/text. Does not measure stop-to-paste latency or sustained soak. |
| Focus change before paste | PASS, earlier candidate | Settings steals focus; terminal remains unchanged; final transcript saved and clipboard-only delivery used. Latest latency fix needs its independent performance/focus checks. |
| Screen capture grant/deny | PASS, limited | Valid KDE native dialogs: denial/timeout preserves dictation text; grant plus normal Pictures directory permits captures. GNOME permission UI remains open. |
| Keyboard denied | PASS, limited | Revoke access to the owned virtual keyboard before latest app launch: drawing inactive, no image, plain dictation still pasted to Chrome. Restore ACL afterward. No host physical device changed. |
| Keyboard group access | PASS, kernel scope | Disposable user cannot read before setup; fresh process with private keyboard group can read, cannot write; owner/input group unchanged; restoration removes access. Actual udev hotplug and logout/login not tested. |
| Microphone permission grant/deny | NOT TESTED | Synthetic private audio and no-device errors do not prove hardware/portal microphone permission decisions. |
| History search | PASS | Missing query gives No matches; fixture query returns saved rows. |
| History Copy and Export | PASS, limited | Both produce exact text from saved synthetic database rows; native export chooser writes a file. The two actions selected different rows, so same-selected-row equality is NOT TESTED, not claimed. |
| Populated old-version History migration | NOT TESTED | Existing upgrade test preserves a marker, not a populated synthetic old database/search/export journey. |

Fixture ASR occasionally changes punctuation or repeats a syllable. Clipboard
comparisons use the actual saved result rather than pretending the recognized
phrase is a perfect transcript of the source audio.

## Installation, downloads and adverse conditions

| Check | Result | Evidence and limit |
| --- | --- | --- |
| README base requirements | FAIL, fix proposed here | Initial bare Ubuntu missed Python and optional layer-shell instructions. This PR supplies package names. Full final-README walkthrough remains open. |
| Runtime package install | PASS, initial candidate | Ubuntu 22.04 base package command, runtime preflight, installer, menu-help and archive verifier pass. Container root for base package setup, not desktop acceptance. |
| Fresh unprivileged install | PASS | UID 1000 installs into empty HOME and launches packaged app. KDE fresh HOME also uses archive installer. |
| Archive installer checks | PASS, initial candidate | Integrity rejection, required loader rejection, prefix spaces/dollar, reinstall/backup and running-old-process preservation. |
| Upgrade from v0.2.0-linux.1 | PASS, limited | Actual old archive then candidate in isolated prefix; version directories and synthetic settings marker retained. Real-shaped populated settings/History still open. |
| Failed network | PASS | Fresh no-network X11 shows error and Retry while staying alive. |
| Slow/interrupted network recovery | PASS, earlier follow-up candidate | Container link shaped to 150 ms/2 Mbit/s; partial download appears. 100% packet loss gives receive-body timeout and Retry, removes partial files. Restore link and Retry: Ready in 14.4 s, no partial files. No app-level cancel/resume claim. |
| No GPU | PASS, limited | Software-rendered GNOME with no GPU devices performs actual CPU fixture ASR and saves one row. No latency/minimum-spec claim. |
| No microphone | PASS, limited | Fresh X11 stays alive with device-format error and Retry. Unplug/replug recovery remains open. |
| Small disk upgrade | PASS, initial candidate | Disposable 192 MiB filesystem ENOSPC: old launcher/data survive, no staging leftovers. Model-download ENOSPC remains open. |
| 512 MiB RAM startup | PASS, limited | Compact model loads and no-mic error remains recoverable. |
| 512 MiB inference / OOM recovery | NOT TESTED | Attempted private audio graph times out creating its stream; no saved transcript. Invalid inference harness, not evidence of app OOM or acceptable inference. |
| Manual uninstall | PASS, file scope | Launcher/desktop/icon/version removal in disposable prefix preserves data. Autostart and keyboard ACL revocation not exercised. |
| Final download/installer/identity | NOT TESTED | Final commit, Mac ZIP and rebuilt Linux assets not yet selected or approved. |

## Public repository and privacy

| Check | Result | Evidence and limit |
| --- | --- | --- |
| Published download includes drawing | FAIL | Existing stable Mac and Linux downloads predate drawing. No release created. |
| Drawing-first docs | FAIL, pending merge | [PR #90](https://github.com/oEdyb/koett/pull/90) documents opt-in, controls and honest current-download status. |
| Support/security intake | PASS | [PR #94](https://github.com/oEdyb/koett/pull/94) merged; bug form and private security link included. Reporting enabled; no report sent. |
| Notice validation | PASS | Merged notice inventory validates all 700 indexed files; eight notice tests pass. Not a legal opinion. |
| Packaged personal paths | FAIL for tested archives | Merged #94 redacts provenance, but old candidate archives still contain original paths. Fresh artifacts required. |
| Current-tree path/screenshot cleanup | FAIL, pending merge | [PR #101](https://github.com/oEdyb/koett/pull/101) redacts 126 text files and removes three inspected screenshots. Original count was 125; later document added one. |
| Full-history secret scan | PASS, detector scope | Gitleaks 8.30.1: 367 fetched public commits, five findings across four evidence JSON files; original values checked, all library/source SHA-256 digests. No credential confirmed. Not proof of no sensitive prose. |
| Historical screenshots | PASS, limited review | OCR of 656 unique reachable image blobs flags only same three path screenshots. Those three visually inspected: synthetic browser geometry, empty History/settings error, synthetic drawing target. Account/home/project/test paths exposed; no credential or personal transcript visible in them. |
| Personal transcript or other sensitive historical content | NOT TESTED exhaustively | No additional credential/private transcript confirmed; OCR and secret detectors cannot certify every pixel or prose passage. No history rewritten. |
| Clone size/time | PASS, measurement only | Initial full public clone 2.67 s; Git files 33,070,644 bytes, worktree 59,784,104 bytes. One network/time, not a guarantee. |
| Probe compilation regression | PASS | #93 merged; #95 closed as identical duplicate. #94 subsequently passed all six checks and merged. Earlier failing import is no longer an open source defect. |
| Linux displayed release version | FAIL, fix proposed | [PR #103](https://github.com/oEdyb/koett/pull/103) changes root package/inventories to 0.3.0; candidate still displays 0.2.0-linux.1. |

[Privacy detail in PR #101](https://github.com/oEdyb/koett/pull/101) records exact
removed filenames, visible contents and detector limits. Stop committing raw
evidence by default: keep reviewed small receipts/synthetic fixtures in Git;
use private artifact storage for raw captures. Public CI artifacts need the same
sanitation as Git. Existing evidence is not deleted wholesale, and history
rewriting remains a separate owner decision.

## Remaining release gates

1. Merge reviewed docs/privacy/version fixes; finish independent dictionary
   performance gate (at most 10% overhead or slow acoustic pass disabled by
   default), stop-to-paste measurements, drawing/pin overhead and live soak.
2. Select the approved source commit only after fixes land. Rebuild Linux x86-64,
   Linux ARM64 and Mac ZIP from that identity, verify all checksums/notices and
   native CI, and repeat changed-path checks on those exact assets. Mac signing,
   notarization and screen permission acceptance require their own receipt.
3. Complete final README from zero with physical login/keyboard permissions,
   full GNOME/X11 ordinary-dictation/paste journeys, populated old-version
   migration, autostart/uninstall, model ENOSPC and valid low-RAM inference.
   Explicitly state ARM64 desktop use remains untested unless hardware runs it.
4. Resolve every remaining NOT TESTED item by valid evidence or an explicit
   support limitation; do not silently turn a lab probe into full acceptance.
5. Obtain explicit authorization for the tag and publication. Attach the Mac ZIP
   alongside both Linux archives and matching checksums. No tag before approval.

Decisions: drawing remains opt-in; candidates and final release are distinct;
raw evidence remains private; no dictionary defaults, installed app, physical
keyboard access or Git history were changed. The performance audit now owns the
exclusive lab slot; remaining documentation work needs no desktop load.
