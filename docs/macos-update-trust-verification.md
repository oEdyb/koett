# Mac update trust and rollback verification — 2026-11-06

> Historical evidence and commands use the [0.3.1 source snapshot](https://github.com/oEdyb/koett/tree/c1e9d812d68fb0c8f035d0041a6d1da29f96c041). See [the evidence archive](evidence-archive.md) for private reproduction.

All UI, recording and installation checks ran inside the disposable macOS guest.
Nothing was published. Production has no feed URL. The installed host v0.3.0 app
and frozen ZIP were not replaced. Fixtures use a distinct lab key, never the new
production key.

## Results

- **PASS:** 19 impacted Swift tests, zero failures, including rollback persistence,
  explicit retry, numeric version ordering, one notice per reservation, inert
  configuration, installation deferral/races, backup refusal and local control.
- **PASS:** release build from `bfe09f3`; `final-release-build.log` records completion.
- **PASS:** Sparkle 2.10.1 rejected an unsigned feed and a signed feed modified after
  signing, before either caused an archive download. Its actual guest error was
  “The update feed is improperly signed and could not be validated.”
- **PASS:** a valid lab-signed feed and archive installed build 7001 from 7000.
  An actual guest recording held installation pending; Stop, transcription and
  normal result delivery completed before idle installation.
- **PASS:** restoring verified backup 7000 while the valid 7001 feed remained live
  set `skippedBuild=7001`. Two launches stayed on 7000 without another payload
  request. Explicit `allow-skipped-update` cleared the ceiling and allowed 7001.
- **PASS:** the retired, production v2 and lab public keys are distinct. A historical
  signature verifies with the retired public key and fails with production v2.
  Production signing refused a local fixture archive and loopback feed.
- **PASS:** all identified copies of the old lab ZIPs/apps were removed from the
  host artifact tree and disposable guest; deletion manifests retain paths/hashes.

The signed-feed/deferred-recording matrix used source
`617b19120b3911942ab11a24731e4a9f45a2ad41`. A final correction at `bfe09f3`
records durable intent at installation callbacks, rather than when an update is
merely offered. That change received the same 19 targeted tests, a release build,
and the separately recorded narrow final-intent guest recheck below. Earlier
receipts are retained with their original source binding, not relabeled.

The final-intent recheck used the same lab-only setup with fresh builds 7100 and
7101: valid upgrade, restored backup held across two launches with the bad build
still served, explicit retry, and exact final executable/source binding all passed.
See `final-intent/receipt.json` and `final-intent/provenance.json`.

## Key and fixture cleanup

Original `B.zip` was at
`/Users/olle/dev/koett-release-artifacts/v0.3.1/auto-updates/B.zip` (build 4011),
with copies under `tested-61ce32e/` and `initial-b175688/`. All three were deleted,
along with only the other old lab archives/apps identified by the manifests.
Public evidence and the retired private Keychain item were retained. User data,
settings, History, the installed app and frozen release ZIP were not deleted.

The production private key is a generic-password item in the Mac login Keychain,
service `https://sparkle-project.org`, account
`com.olledyberg.Koett.EdDSA.production.v2`. The lab account is
`com.olledyberg.Koett.EdDSA.lab.v1`. No private material is in these receipts.
The new production key made no lab signatures. New fixture copies remain local
and use only the lab key.

## Limits

No live feed, published release, new notarization, host update, automatic health
rollback or arbitrary macOS-version matrix was tested. The reservation notice's
once-only behavior and retained installation reservation are unit-tested; its
brief visual appearance during an actual install handoff was not captured.
Manual restore must happen after the installer finishes, as documented.

The exact guest drivers, source/binary bindings, requests, key proof and cleanup
manifests are in [the evidence directory](https://github.com/oEdyb/koett/tree/c1e9d812d68fb0c8f035d0041a6d1da29f96c041/docs/evidence/macos-update-trust-20261106).
Sparkle's [current settings documentation](https://sparkle-project.org/documentation/customization/)
specifies signed-feed support since 2.9 and zero as the value that disables
signature-failure expiration. Both flags are enforced before enabling Koett's updater.
