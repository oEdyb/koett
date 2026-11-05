# Linux v0.3.0 first-install audit

**Decision: NO-GO. No tag or release was created.**

Audit date: 2026-11-05 UTC. This is an executed partial audit and an explicit
remaining acceptance matrix, not a certificate that v0.3.0 is ready.

## Build identity and evidence

Tested the release-style system-runtime archives from
[CI run 41936332855](https://github.com/oEdyb/koett/actions/runs/41936332855),
source `eb6f2d6e896497d7e7e1c527aa8bba88ec23ee50`:

- `Koett-Linux-x86_64-system-runtime-eb6f2d6e.tar.gz`
- `Koett-Linux-aarch64-system-runtime-eb6f2d6e.tar.gz`

These are CI candidates, not a published v0.3.0. They predate the merged paste
fix. None of their runtime results establish acceptance of that fix or of the
final tag. The old upgrade input was the actual v0.2.0-linux.1 x86-64 release.
Source audit started at `d9215903c5f07403f60c1cd92679280056725cdc`;
subsequent source checks included main through `61e121e`.

[Sanitized receipts](evidence/release-v0.3.0-audit/receipts.json) record hashes,
commands' observed results and limits. [Privacy findings](evidence/release-v0.3.0-audit/privacy-scan.json)
list filenames and counts without copying matched private content or OCR text.
Raw local diagnostic logs are deliberately excluded from this public report.

PASS means the stated observation was exercised on that artifact. FAIL means a
requirement is demonstrably unmet. NOT TESTED means no valid acceptance result;
a broken lab does not become a product failure or a pass. Historical receipts
elsewhere in this repository are not new results for this candidate.

## Isolation

No installed Koett, visible desktop, host clipboard, host microphone recording,
physical keyboard permissions or user configuration was changed. Tests used
fresh homes/prefixes, disposable containers, private buses/audio and invisible
compositors. X11 negative tests used UID 1000 and Xvfb, no network, GPU or audio
devices. Container setup/installer tests also used container root; those are
identified below and do not prove an ordinary user's entire desktop setup.

KDE used a new agent-lab session with a new HOME. Its portal services inherit the
lab's own data directories, so app identification/permission acceptance was not
valid for that alternate HOME. Do not interpret the resulting portal registration
errors as a proven defect on a normal installed KDE session.

No native release build was made locally. The probe compile checks used nice 15,
two jobs and an 8 GiB memory scope, with output in a cache directory, never /tmp.

## Architecture and desktop coverage

| Architecture / desktop | Result | Evidence and exact limit |
| --- | --- | --- |
| x86-64 archive | PASS | SHA-256 matches sidecar; native receipt source matches candidate SHA. |
| ARM64 archive | PASS | SHA-256 matches sidecar; source matches; extracted file/privacy inventory inspected. |
| ARM64 execution, every desktop | NOT TESTED | No ARM64 machine or registered ARM64 emulator available. CI inference is separate evidence, not desktop acceptance. |
| x86-64 KDE Wayland | PASS, limited | Fresh default model downloaded; active engine reached Ready at 10 seconds. Empty History and Settings opened. |
| KDE permissions/paste/drawing acceptance | NOT TESTED | Portal approval harness timed out; alternate-HOME portal identity mismatch invalidates the setup. Needs a clean full session/user with matching portal environment. |
| x86-64 niri nested private session | PASS, limited | Actual microphone-fixture ASR consumed by GTK entry and kitty, Unicode and focus guard tested. Model was supplied, not downloaded from zero in this session. |
| x86-64 GNOME Wayland | NOT TESTED | Headless GNOME container created a socket but shell initialization failed and the app could not open the display. No valid desktop acceptance. |
| x86-64 plain X11 | PASS, limited | Fresh unprivileged Xvfb sessions, X11 shortcut backend, offline/no-mic/512 MiB startup probes. Xvfb is not a full desktop or hardware session. |
| GNOME/X11 drawing | NOT TESTED | Source documents no drawing overlay there. Must verify usable ordinary dictation and clear unavailable-drawing feedback on final artifacts; no drawing-support claim. |

## Installation and adverse conditions

| Check | Result | Evidence and exact limit |
| --- | --- | --- |
| README from zero | FAIL | Missing Python in bare Ubuntu makes launcher exit 127. Base package names and optional gtk4-layer-shell prerequisite were not explained sufficiently; this PR adds them. Only a fresh rerun after merge closes this item. |
| Missing prerequisite rejection | PASS | Bare Ubuntu without Python exits before installing. Extracted-package verifier also exercises a required-loader rejection with no prefix created. This is not a complete missing-package permutation matrix. |
| Install after runtime packages | PASS | Ubuntu 22.04 disposable container: preflight, installer and installed menu-help succeed. GTK 4.6.9. Container root, no desktop acceptance. |
| Fresh unprivileged install | PASS | UID 1000 installs into its empty HOME in X11 probes and runs the packaged application. |
| Archive integrity and spaces in prefix | PASS | Existing archive verifier checks extraction, notices, isolated install in a prefix with spaces/dollar, tamper rejection and missing-loader rejection. |
| Reinstall / running old process | PASS | Verifier preserves the old process, version target and backup on reinstall. Does not prove quitting/reopening through a real desktop launcher. |
| Upgrade from v0.2.0-linux.1 | PASS, limited | Actual old archive installed then candidate into same isolated prefix; two version directories retained and synthetic settings marker survives. |
| Upgrade with real-shaped settings and populated History | NOT TESTED | Marker preservation is not a migration test. No personal data used. Need synthetic old-version dictation/search/export data and restart checks. |
| No GPU | PASS, limited | Model loads in container without GPU devices with software rendering. No CPU-only inference latency or long recording measured. |
| No microphone | PASS, limited | Fresh X11 app remains alive and displays a device-format error with Retry after Start. No unplug/replug/recovery sequence. |
| Small disk / upgrade ENOSPC | PASS, limited | Fill disposable 192 MiB filesystem after old install. Upgrade fails with ENOSPC; old launcher and synthetic data survive; no staging directory remains. Does not exercise model-download ENOSPC. |
| 512 MiB RAM | PASS, limited | Fresh unprivileged app loads supplied compact model and survives Start's no-mic error. No inference, drawing or sustained memory pressure acceptance. |
| Other low-RAM models / OOM recovery | NOT TESTED | No validated minimum RAM claim. Need real inference and a model that exceeds the limit, plus restart/recovery. |
| Manual uninstall | PASS, limited | Remove launcher, desktop entry, icon and versions inside disposable prefix; synthetic user data remains. Instructions added here; Start at login revocation and keyboard ACL removal not exercised. |
| Fresh v0.3.0 download / final artifact identity | NOT TESTED | Final SHA and release assets not chosen; do not substitute these earlier candidates. |

The complete base-runtime package command was actually run in Ubuntu 22.04,
followed by `--check-runtime`, `install.sh`, `menu-help` and
`test_package_linux_archive.py`. GTK notice validation requires all 698 indexed
files. Both archives include 713 regular files, including notices and manifests;
no settings file, transcript database or transcript archive was found by name.

## Model, permissions and workflow matrix

Unless a row explicitly says otherwise, it is NOT TESTED on ARM64 and on the
final candidate, and does not establish coverage on another desktop.

| Check | Result | Evidence and exact limit |
| --- | --- | --- |
| First compact-model download | PASS | New KDE HOME using standard XDG paths; busy samples become an active compact engine at 10 seconds. No prepopulated model in that HOME. |
| Failed network | PASS, limited | Fresh unprivileged X11, no network/model: visible DNS/download error, Retry available, app alive. No successful retry after reconnect. |
| Slow/interrupted network, cancel/resume | NOT TESTED | No traffic-shaped complete download or interrupted partial-file recovery run. |
| Microphone permission grant/deny | NOT TESTED | Private synthetic audio and no-device tests do not emulate portal permission decisions. |
| Keyboard-access setup from a new user | NOT TESTED | No real keyboard device/group/udev ACL grant and fresh login tested. Host membership must not stand in for this. |
| Keyboard denied, partial access, fallback focus | NOT TESTED | Need devices with controlled ACLs inside a VM/private seat; no host permissions changed. |
| Screen capture permission grant/deny | NOT TESTED | KDE portal lab setup blocked acceptance; GNOME lab invalid. |
| Ordinary dictation into GTK | PASS | niri harness plays pinned sample into private audio; GTK consumes exact ASR result once. |
| Terminal paste | PASS | niri kitty Ctrl+Shift+V consumes exact Unicode once; typing and Enter remain intact. Plain text only. |
| Browser paste | NOT TESTED | GTK entry is not browser acceptance. Need a real browser with a fresh profile and both image-aware and plain text fields. |
| Electron paste | NOT TESTED | Need actual Electron app/editor and integrated terminal targets; no generic GTK substitution. |
| Focus changes before paste | PASS | niri harness moves focus to Settings; terminal unchanged, final text saved and clipboard-only delivery used. Older candidate; repeat with merged fix. |
| Drawing off by default | PASS, limited | Visible unchecked control on the Draw on screen page in new KDE Settings; plain niri dictation succeeds. |
| Drawing enabled and disabled through Settings | NOT TESTED | Settings opens and default observed; AT-SPI toggle automation failed. No Save/reopen claim. |
| Hold Shift and move / release / repeated holds | NOT TESTED | No fresh-artifact drawing gesture acceptance in this audit. Historical drawing evidence is insufficient. |
| Shift+click pins / selected-text context | NOT TESTED | Need clean browser/app contexts, empty selection and accessibility-denied cases. |
| Ctrl+Alt+D lock / exit / undo / stop | NOT TESTED | Need full gesture and clipboard result checks on clean supported desktops. |
| Draw/pin image and text delivery / repeat paste | NOT TESTED | Must inspect consumed image and exact text in browser, terminal image target and Electron. No plain-text test proves this. |
| Settings | PASS, limited | Opened native Settings and Draw on screen page. Save/reopen, shortcuts, microphone switching and all other pages remain NOT TESTED. |
| History | PASS, limited | New empty History visible; niri fixture dictations saved. Search, selection, Copy, Export and upgrade migration remain NOT TESTED. |
| Uninstall / upgrade with desktop autostart | NOT TESTED | File-level test does not prove autostart disable/reenable and desktop launcher refresh. |

The niri run used `CrossPlatform/scripts/test_linux_paste.py --niri` against the
extracted archive launcher and the newly downloaded compact model, inside
agent-lab's private audio/compositor. It is a regression test, not the whole
README-from-zero journey. Its four assertions are reproduced in the receipts.

## Public repository and visitor readiness

| Check | Result | Evidence and exact limit |
| --- | --- | --- |
| Published download has drawing | FAIL | Latest Mac v0.1.1 and Linux v0.2.0-linux.1 predate drawing. No release made during audit. |
| Drawing README accuracy | FAIL, pending PR | [PR #90](https://github.com/oEdyb/koett/pull/90) documents controls and honest current download status. Not assumed merged. |
| Install/update/uninstall docs | FAIL, fix proposed here | Add base package names, gtk4-layer-shell requirement and app-only removal steps; require final README walkthrough. |
| Issue intake | FAIL, fix proposed | No issue template on audited main. [PR #94](https://github.com/oEdyb/koett/pull/94) adds a safe bug form and private security link. |
| Security/contact | PASS, limited | SECURITY.md directs private reporting; repository API confirms reporting enabled. Issues enabled for public support. No test vulnerability submitted. |
| Dependency notices | PASS | 698 canonical files validate; eight notice tests pass for #94. Archive verifier checks notices exactly. Not an independent legal opinion. |
| Personal paths in packaged notices | FAIL, fix proposed | Both architectures and old release contain build-local paths in GTK provenance. #94 redacts prefixes and updates the index. Previously published files remain unchanged. |
| Text/source private-content scan | FAIL | 2,646 tracked files scanned; 125 text files contain personal absolute home paths. 52 match author-name patterns, including public attribution/test data. No private-key header or selected provider-key-shaped match. Not a clean privacy certificate. |
| Committed screenshots | FAIL | OCR covered all 770 tracked images; three contain home-path matches. One browser-address-bar match was visually confirmed. Other image contents require human review; OCR cannot prove absence of personal History text. |
| Personal History text in source/images | NOT TESTED completely | Filename references are mostly code/docs, not evidence of disclosure. No tracked database/transcript artifact by known filename; full semantic/manual screenshot review not completed. |
| Secrets across full Git history | NOT TESTED | Current-tree heuristic scan is not a complete secret scan. Repository secret scanning and push protection are enabled; no full-history detector receipt. |
| Clone size/time | PASS, measured only | Full public clone: 2.67 s on audit network, 33,070,644 bytes Git files and 59,784,104 bytes working tree. One machine/network; not a universal performance guarantee. |
| CI minimum-Rust gate | FAIL, fix proposed | [Failing job](https://github.com/oEdyb/koett/actions/runs/41940339809/job/126959217376): probe lacks shared focus_tokens module. [PR #95](https://github.com/oEdyb/koett/pull/95) fixes import; local Rust 1.92 and pinned all-target checks, strict lint, format and two token tests pass. Native linking/inference gates remain CI work. |

No evidence images were removed or altered. Redacting current files does not
remove existing Git history; do not rewrite public history as an incidental
cleanup. Public copyright attribution and application IDs are not secrets and
must not be stripped as if they were private names.

## Release gate

Keep NO-GO until all of the following have fresh evidence:

1. Choose the final approved SHA after paste and audit fixes. Build both Linux
   architectures and the Mac ZIP from it. Pass required native CI gates and
   verify receipt/source/hash identity for every downloaded artifact.
2. Complete real clean-user KDE, niri, GNOME and plain X11 journeys. Cover every
   workflow row above, including supported-desktop drawing/pins/lock, final
   paste fix, browser/Electron/terminal consumers and unsupported-overlay fallback.
3. Exercise slow/failed/recovered downloads, model ENOSPC, actual CPU-only and
   low-RAM inference, keyboard/screen permissions, populated upgrade and uninstall.
   Obtain ARM64 desktop evidence or explicitly narrow the release support claim.
4. Close repository privacy findings, manually review committed screen content,
   and produce full-history secret-scan evidence. Do not publish new private
   snippets in the cleanup PR or in issue reports.
5. Rerun only the final public README against the final downloaded packages,
   with no preexisting model, credentials, group membership or private setup.

Decisions: retained the off-by-default drawing behavior; used existing CI
archives rather than a locally advantaged binary; kept partial checks scoped;
proposed small independent fixes; did not tag, publish, replace an installed app,
modify keyboard permissions, or treat historical evidence as fresh verification.
