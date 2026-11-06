# Mac update guest verification — 2026-11-06

Tested runtime source: **7b7e590ff76b4995602645d56ab2c82c113f3ae9**.
Later documentation/evidence commits do not change this tested product code.

`swift test --filter 'UpdateConfigurationTests|UpdateInstallGateTests|UpdateBackupStoreTests|DictationLifecycleTests|RecordingOverlayTests|LocalControlTests|TerminationProtectionTests|Relaunch'`
with an isolated `CFFIXED_USER_HOME`: **48 tests, zero failures**. This covers
feed/key validation, disabled startup, AppKit automatic menu validation, install
reservation/race/abort behavior, session admission, existing recording lifecycle,
quit/relaunch recovery and local JSON control. It is not a full-suite run.

`swift build -c release --product koett`: **PASS**, 47.31 seconds. Shell syntax,
Info.plist lint and `git diff --check`: PASS. The bundled framework and outer app
passed `codesign --verify --deep --strict`. Sparkle licenses are embedded.

## Guest results

Tart `koett-limit-11min`, macOS 15 (exact version in `inert-final.txt`), disposable
admin data. Guest-only BlackHole input/output and public synthesized phrase;
actual recorder start/stop, ASR, clipboard delivery and Sparkle installation.
No shipping-pipeline bypass, host screen, host microphone, host input, host
clipboard, installed app replacement or large benchmark. The feed server bound
**127.0.0.1:18766 inside the guest**, never a live GitHub feed.

Two separately Developer-ID-signed local app fixtures, versions **0.3.1/4010** and
**0.3.1/4011**, used the same release executable/source with distinct signed bundle
metadata. Their final signed executable hashes differ because their app signatures
differ. Fixture hashes, signed ZIP hash, public signature, exact driver/collector
sources and receipts are in [the evidence directory](evidence/macos-updates-20261106/manifest.json).
Local ZIPs/apps remain outside git under the artifact root in that manifest.
They were not notarized or published release assets.

- **PASS — bad archive signature:** flip an Ed25519 signature byte in the feed.
  Sparkle downloads the ZIP, reports signature rejection, leaves build 4010 and
  its executable hash unchanged. No pending installation remains.
- **PASS — bad application signature:** modify build 4011's sealed Info.plist
  without re-signing the app; sign that new ZIP correctly with Ed25519. Independent
  CryptoKit verification confirms the archive signature is valid. Strict Apple
  signature verification fails, and Sparkle refuses installation; build 4010 stays.
- **PASS — recording deferral:** hold the feed response until actual recording
  starts, then serve the valid update. Status shows `pending=true`, `installing=false`,
  phase recording and unchanged build/hash after downloading. It remains that way
  after a further three seconds. A foreground check returns `busy`.
- **PASS — transcription deferral:** play the synthetic phrase into guest BlackHole,
  then Stop. The immediate receipt is phase transcribing with the update pending.
  After work/result handling finishes, Sparkle relaunches into build 4011 and its
  exact expected signed executable hash. No update buttons were clicked.
- **PASS — rollback:** the previous app copy strictly verifies and matches the
  complete build-4010 executable hash. After quitting, restoring that copy and
  launching it reaches ready on build 4010. The local feed is empty during rollback
  to avoid immediately offering the newer build again. No user data is removed.
- **PASS — foreground check:** the local `check-updates` action is accepted when
  idle, requests the feed again and presents Sparkle's alert with an OK button.
  This is the same updater action targeted by Check for Updates in the menu.
- **PASS — no feed:** a signed fixture with the production feed-less settings reaches
  ready with `updates.enabled=false`. The command returns `updates_disabled`.
  Feed request count and backup filenames do not advance. Unit tests additionally
  prove AppKit keeps the unconfigured menu action disabled during automatic validation.

The hash guard caught an initial stale VirtioFS copy while preparing a repeat
run. That run stopped before any acceptance claim. Final fixtures were copied
through SCP, verified by hash, and the final receipts above bind to the expected
source and executable hashes. Earlier receipts are not substituted for them.

The guest screen compositor did not render the open status menu in captured
screenshots; no visual pixel claim is made. The item title/target and automatic
validation were checked in code/AppKit tests, and its updater action was exercised
through local control in the guest.

## Limits and unchanged host artifacts

Not tested: live GitHub redirects/feed publication, an authorized notarized future
release upgrade, interrupted/slow downloads, loss of power/disk during installation,
non-writable-volume repair, real-host upgrade, Intel, or other macOS versions.
There is no automatic health rollback. An independent cross-model security review
was not run under the task's no-delegation constraint; do that before live feed
activation. Sparkle's pinned validator is used rather than a custom installer.

The actual `/Applications/Koett.app` still verifies strictly and its executable
SHA256 remains `d0181d5409c1bd52ace892573bdab80236b5e5df6fbd7bbcfc269175a6f6ddf0`.
The frozen v0.3.0 ZIP remains
`d6b1a8527a330fe58f65ea45ad485a2bddf67312d8405fdf2ceb7b8c04eb5a94`.
No tag, release, draft asset or public feed was created or changed.

Activation and Keychain ownership are documented in [Mac updates](macos-updates.md).
