# Native Mac installer

Koett's native direct-download installer is a signed, notarized, read-only DMG.
Opening it presents Koett and an Applications shortcut in Finder. Drag Koett onto
Applications, eject the disk, then open Koett from Applications. There is no shell
installer, root helper or package receipt. Existing permission onboarding remains
inside Koett. The first launch may show macOS's normal downloaded-app confirmation.

## Install, upgrade and uninstall

- **Install:** open the downloaded DMG, drag Koett to Applications, eject the DMG,
  then launch `/Applications/Koett.app`.
- **Upgrade:** wait until dictation/transcription finishes, quit Koett, open the
  newer DMG and drag Koett to Applications. Choose Replace in Finder. Launch the
  copy in Applications, then eject the image. Keep the old DMG if manual rollback
  is needed. Settings, History, model caches and Keychain items stay in place.
- **Uninstall:** quit Koett while idle, turn off Open at Login in Koett's menu or
  System Settings, then move Koett from Applications to Trash. Local data is
  retained intentionally; app removal never deletes transcripts or API keys.

The DMG is a first-install/manual-upgrade surface. Sparkle still owns automatic
updates after a separately authorized feed is enabled. Packaging does not change
its feed, public key, designated requirement, helper signatures or entitlements.
Do not use a system-owned `.pkg`, run a root postinstall, or recursively chmod the
app: Finder installs as the current user and preserves its signed framework links
and executable permissions. On managed Macs, installing to Applications may
require the administrator's authorization through Finder.

## Build an artifact without touching an installed app

Start with a clean, source-stamped release app built using the existing Mac
release procedure. Embed Sparkle with `scripts/embed-sparkle.sh` before signing
the outer app with Developer ID Application, team LF8KF3G42Q. Notarize and staple
that app. Keep build output outside `/Applications` and outside frozen releases.
The DMG packager consumes that app and never rebuilds, re-signs or edits it.

```sh
python3 scripts/package-macos-dmg.py \
  --app /absolute/staging/Koett.app \
  --output /absolute/artifacts/Koett-VERSION-macOS-arm64.dmg \
  --identity 'Developer ID Application: Dyberg & Co AB (LF8KF3G42Q)' \
  --notary-profile koett-notary
```

Python 3.11+ and Xcode command-line tools are required only on the packaging Mac.
The existing profile contains credentials in the login Keychain; none are passed
on the command line or stored in the repository. `--notary-profile` explicitly
submits this DMG to Apple's notarization service and waits for its verdict.
This is not a GitHub release/upload command. Use `--prepare-only` instead for a
local signed candidate without any external submission; its receipt explicitly
says it is **not distributable**. Nothing publishes automatically.

The packager refuses existing output paths, incorrect app identifiers, lab update
flags, wrong signing teams, unstapled apps and failed Gatekeeper assessments.
`ditto` preserves signed framework symlinks and modes; a complete file/hash/mode
manifest checks staging. `hdiutil` makes a compressed HFS+ read-only image with
ownership disabled for copying. No chmod/chown is applied to the app. The image is
Developer-ID-signed, submitted, stapled and assessed. Failures exit nonzero; only
a fully accepted artifact receives `distributable=true` in its receipt. Keep the
`.notary.json`, `.receipt.json` and `.dmg.sha256` alongside the exact final DMG.

The image includes a saved Finder layout (`Packaging/DMG/FinderLayout.dsstore`),
not an executable installer. To regenerate it, make a disposable writable HFS+
image named `Koett Layout` containing `Koett.app` and an Applications symlink.
Run `Packaging/DMG/layout.applescript` **inside a disposable Mac guest**, eject,
remount and copy its `.DS_Store` back to the tracked layout file. The packaging
script never drives Finder or changes the packaging Mac's windows.

## Handoff snippet for release automation and README owner

No shared workflow or root README is edited by this change. After the existing
Mac step has produced a verified, notarized, stapled staging app, invoke the
command above. Attach the resulting DMG, checksum and receipt only with release
publication authorization. Keep Sparkle's separately signed update archive/feed
workflow unchanged; the user-facing DMG does not activate it.

Suggested Mac README install text:

> Download `Koett-VERSION-macOS-arm64.dmg`, open it, drag Koett to Applications,
> eject the image and open Koett from Applications. Requires Apple silicon and
> macOS 15 or later. For a manual upgrade, quit Koett while idle before replacing
> the app. Settings and History are kept.

## References

- [Apple: packaging Mac software](https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution)
- [Apple: notarizing Mac software](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- [Sparkle: preserve framework links and executable permissions](https://sparkle-project.org/documentation/)

## Local acceptance — 2026-11-06

Five impacted Python packaging tests passed; no Swift source changed, so no Swift
suite or benchmark ran. The local signed candidate passed image verification and
strict image signing. A second candidate exercised the new mounted-image manifest
check successfully. No shared workflow was edited. The separate manual pre-merge
batch CI run is linked from the pull request; it is not Mac GUI acceptance.

The disposable macOS 15 guest exercised an actual mouse drag from the mounted DMG
onto Applications, launch to `phase=ready`, native Finder Trash uninstall, install
and launch of the previous public 0.3.0 ZIP, Finder replacement with the DMG app,
launch to ready again, and final uninstall. The installed app's file hashes,
symlinks and modes matched the image. It was owned by UID501, mode0755 and writable
by the guest user; Sparkle's `Versions/Current → B` and executable Autoupdate
survived. Strict codesign, stapler and app Gatekeeper assessment passed. A settings
sentinel and exact History database hash survived upgrade; uninstall kept local
data. The prior guest app/data/preferences were restored afterward.

The payload was the already notarized Sparkle-signing probe app, version0.3.1,
build5101, source `7ef87560115fcf10ab5fb616973dbe2ccc49cdb3`, executable UUID
`F949DA85-D0D2-3905-9129-03C59336B5DB`. It is a packaging fixture, not a current-main
release candidate. The previous public app was the unchanged frozen0.3.0/build3
from source115d6907. This isolates packaging changes without another product build
or broader acceptance repeat. The real installed host Koett and frozen ZIP were
not changed. No release, tag, GitHub draft/asset or live feed was created or edited.

**PASS:** after explicit Apple-only authorization, the exact DMG was accepted by
Apple (`50f1ed70-a67a-47d3-937c-e7c6d19c976d`), stapled and validated. Strict image
codesign, `hdiutil verify`, stapler and Gatekeeper (`Notarized Developer ID`) pass.
Final SHA256: `c4dd4ba5c3dbe33e115c2166ef0f2044e36bfadc8dd9b274487ace1f01909834`.
Artifact filename: `Koett-0.3.1-macOS-arm64.dmg` (retained locally, not published).

The final DMG received a download quarantine marker inside the guest. Image
Gatekeeper/signature/ticket checks passed there, then another actual Finder drag
installed the exact payload. The normal “downloaded from the Internet” Open
confirmation said Apple checked it for malicious software. Open was clicked in
the guest; the installed app reached `phase=ready`. Its app signature/ticket and
Gatekeeper assessment passed, UID501/mode0755 remained correct. The previous guest
app/data/preferences were restored and the guest stopped. The final check's source,
artifact hash, exact driver and Open-confirmation screenshot are retained.

This notarized **fixture** proves the packaging path, not a current-main release
freeze or release approval. Rebuild the real release app from the agreed final
source before making its public DMG. First-launch permission denials, a pristine TCC database, Intel, managed/non-admin
install and a live Sparkle upgrade were not retested by this packaging change.

Exact driver, receipts, hashes and inspected Finder screenshot:
[evidence](evidence/macos-dmg-installer-20261106/).
