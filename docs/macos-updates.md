# Mac updates

Sparkle **2.10.1** is pinned in Package.swift/Package.resolved. This source has
**no SUFeedURL**, so it starts no updater, creates no update backups and makes no
update requests. Check for Updates is visible but disabled. The frozen v0.3.0
ZIP and installed app are unchanged; they do not acquire updating from this PR.

## Behavior once enabled

The signed bundle supplies an HTTPS appcast URL and an Ed25519 public key.
Sparkle checks in the background and downloads updates automatically. A manual
Check for Updates uses Sparkle's normal foreground interface. Existing Sparkle
preferences are respected. There is no new permissions/opt-in prompt.

Before starting Sparkle, Koett strictly verifies its own Developer ID signature
and copies the current app to `~/Library/Application Support/Koett/Update Backups`.
The copy must verify with the same code-directory hash, bundle identifier and
team LF8KF3G42Q. Failure stops updating and appears in the menu tooltip/status.
The two newest verified copies are retained, including the previous app after a
successful update. Settings, keys, history and models are not included or edited.

Signed feeds are required (`SURequireSignedFeed=true`) with unsigned fallback disabled
(`SUSignedFeedFailureExpirationInterval=0`). Sparkle verifies the feed before parsing
update metadata, then verifies the archive's Ed25519 signature before extraction and
the app's Apple signature. Installation waits until Koett is ready, not recording,
transcribing or processing media, not relaunching, not showing its result/recovery
pill, and has no volatile transcript saves. The app reserves that idle state
before invoking Sparkle, blocking new captures/model switches. It rechecks before
handoff. Existing termination protection also refuses to quit during work.

The standard updater may show an error for a manual check; background failures
are available through the menu tooltip and local-control status. A downloaded
update is retained while busy. No recording limit is shortened to install it.

## Enable a future release

This is release work requiring separate authorization; none of it ran here.

1. Choose a stable HTTPS appcast asset URL on GitHub Releases, for example
   `https://github.com/oEdyb/koett/releases/latest/download/appcast.xml`. Every
   release using that URL must include its appcast asset. Never point production
   at a draft or the local test server.
2. Put that URL in **SUFeedURL** in Packaging/Info.plist; retain SUPublicEDKey.
   Keep **KoettLocalUpdateTesting absent**. It only allows literal loopback HTTP
   in explicitly signed guest fixtures. There is no defaults/environment override.
3. Increment CFBundleVersion monotonically and set the public version. Build a
   clean pinned source, assemble resources and run `scripts/embed-sparkle.sh APP
   IDENTITY` **before signing the outer app**. It embeds the framework, preserves
   the Downloader's required entitlements, signs Installer.xpc, Downloader.xpc,
   Autoupdate and Updater.app before the enclosing framework, and includes licenses.
   Sparkle's downloaded helpers are ad-hoc signed; preserving those signatures is
   insufficient for notarization. Follow its [manual signing instructions](https://sparkle-project.org/documentation/sandboxing/).
   `install-macos.sh` already calls it. Other release packaging must do so too.
4. Sign with Developer ID LF8KF3G42Q, notarize, staple, validate the app, then zip
   that final stapled app. Sign the **exact final ZIP** with the update key. Any
   rezip or stapling afterward invalidates the archive signature.
5. Build/sign the helper below and run `koett-update-key --sign FINAL.zip`. Use its
   `sparkle:edSignature` and `length` in the appcast enclosure, with the exact
   CFBundleVersion, minimum macOS 15 and immutable GitHub Release ZIP URL. SHA256
   remains a separate public release checksum, not a substitute for Ed25519.
   Sign the final appcast with `koett-update-key --sign-feed appcast.xml`; keep
   SURequireSignedFeed and zero failure-expiration enabled. Publish the exact signed
   XML bytes and archive together only when authorized.
6. Verify a real upgrade from the first updater-enabled app in a guest, then
   publish the authorized appcast/ZIP assets together. No live-host update was
   tested. Existing v0.3.0 users need one manual installation of an enabled build.

## Signing keys and lab isolation

The unused original production key was retired after it signed local test builds.
No enabled build shipped with it. Its private item is retained, not deleted or exported.
The current production key lives only in this Mac's **login Keychain**
(`~/Library/Keychains/login.keychain-db`), generic-password service
`https://sparkle-project.org`, account **`com.olledyberg.Koett.EdDSA.production.v2`**,
label **Koett PRODUCTION update signing v2 (Ed25519)**. Packaging/Info.plist stores
only its public key. The distinct lab account is `com.olledyberg.Koett.EdDSA.lab.v1`.
Neither private key is in a file, repository, guest, CI variable, log or app bundle.

Compile `scripts/create-macos-update-key.swift` and sign its executable with
identifier `com.olledyberg.Koett.UpdateSigningKey`, using Developer ID Application:
Dyberg & Co AB (LF8KF3G42Q). `--create` returns/creates the production public key;
`--lab --create` returns/creates the lab public key. Keychain interaction is disabled;
a locked or denied item fails instead of prompting. No private-key export is supported.

Production `--sign FINAL.zip` refuses local/test flags, wrong public keys, insecure
or non-GitHub feed URLs, and apps that do not pass strict signing, stapled-ticket
validation and Gatekeeper's Notarized Developer ID assessment. Production
`--sign-feed appcast.xml` accepts HTTPS GitHub Release enclosures only. Use inline
release notes. It appends Sparkle's signature block to a fresh unsigned XML file;
any change afterward requires regenerating and signing a fresh feed. The appcast,
archive and optional external notes are separate authenticated objects; this helper
deliberately does not support external release-note files.

Labs must pass **`--lab`** for both archive and feed signing, embed the lab public
key, and set `KoettLocalUpdateTesting=true` in the locally signed bundle. The lab
key must differ from Packaging/Info.plist. Staging scripts require the explicit
`KOETT_LAB_UPDATE_SIGNER` executable and cannot fall back to production mode.
No production signature may be made for any lab archive or feed.

## Roll back and stay on the old build

After the installer finishes, quit Koett while idle. Never replace an app while
its installer is running. In Finder, Go to Folder →
`~/Library/Application Support/Koett/Update Backups`. Keep the current application
elsewhere, copy the desired verified older app to Applications as Koett.app, and
launch it. User data stays in place.

Before handing a verified update to installation, Koett durably records its target
build. Merely finding or dismissing an offered update does not mark rollback intent.
A failed intent write blocks installation and normal Quit until it can be saved. A lower build
on the next launch identifies rollback or an unsuccessful installation. That build
and earlier builds are excluded from automatic **and manual** checks, even if the
bad update remains in a correctly signed feed. The exclusion survives restarts.
A genuinely newer build is allowed. To retry the excluded build intentionally,
choose **Allow Skipped Update (BUILD)**, or run `koett --control allow-skipped-update`,
then check again. No other check silently clears the exclusion.

There is no automatic health rollback or one-click app replacement. The old public
v0.3.0 app has no updater and therefore cannot silently reinstall an update.
A recording requested during installation reservation shows one short existing-pill
notice: “Installing update · Try again shortly”. It never opens the microphone and
the notice does not block the install itself.

## Evidence

See [guest verification](macos-updates-verification.md). It covers a loopback
feed, two locally Developer-ID-signed builds, rejected archive and application
signatures, actual recording/transcription deferral and a retained rollback copy.
It does not certify a published feed, notarized future asset, live-host update,
network interruption recovery or all macOS versions.

## References

- [Sparkle setup](https://sparkle-project.org/documentation/)
- [Programmatic setup](https://sparkle-project.org/documentation/programmatic-setup/)
- [Updater delegate and installation hooks](https://sparkle-project.org/documentation/api-reference/Protocols/SPUUpdaterDelegate.html)
- Pinned `SUUpdateValidator.m` and `SPUUpdaterDelegate.h` in Sparkle 2.10.1.
