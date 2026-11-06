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

Sparkle verifies the archive's Ed25519 signature before extraction, then verifies
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
6. Verify a real upgrade from the first updater-enabled app in a guest, then
   publish the authorized appcast/ZIP assets together. No live-host update was
   tested. Existing v0.3.0 users need one manual installation of an enabled build.

## Signing key

Private key material lives only in this Mac's **login Keychain**
(`~/Library/Keychains/login.keychain-db`), generic-password service
`https://sparkle-project.org`, account `com.olledyberg.Koett.EdDSA.v1`, label
`Koett update signing (Ed25519)`. The repository stores only the public key.
The seed is base64 in Sparkle's compatible 32-byte Ed25519 format. Do not export
it into the repo, a CI variable, command arguments, logs or an app bundle.

`scripts/create-macos-update-key.swift` creates/reads that item or signs an archive;
it disables Keychain interaction and fails instead of prompting. Compile it and
sign the executable with identifier `com.olledyberg.Koett.UpdateSigningKey` and
the same Developer ID identity before using the existing item's access list:

```sh
swiftc scripts/create-macos-update-key.swift -o /PRIVATE/koett-update-key
codesign --force --options runtime --identifier com.olledyberg.Koett.UpdateSigningKey \
  --sign 'Developer ID Application: Dyberg & Co AB (LF8KF3G42Q)' /PRIVATE/koett-update-key
/PRIVATE/koett-update-key --sign /PRIVATE/Koett.zip
```

For initial creation only, pass Sparkle's built tools directory instead of
`--sign ARCHIVE`. Re-running creation prints the existing public key and never
rotates/overwrites it. Sparkle's vendor command-line signer blocked in this host
session and was stopped without desktop interaction; this helper signed the
fixtures noninteractively. Sparkle's actual guest updater verified them.

## Roll back

Quit Koett while idle. In Finder, Go to Folder →
`~/Library/Application Support/Koett/Update Backups`. Keep the current application
elsewhere, copy the desired verified older app to Applications as Koett.app, and
launch it. User data stays in place. There is no automatic health rollback or
one-click rollback UI in this change. If a bad update remains in the feed it can
be offered again; remove/fix that release's appcast before rolling users back.

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
