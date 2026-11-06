# Mac feed activation verification

Source `dbe53ecb067433a27c95a53cfd2bb79092dddc42`, macOS 15.7.7 guest,
Developer ID-signed lab builds 8000 and 8001. The guest used a separate lab
Ed25519 key and localhost feed. No production key signed a lab archive.

- A missing feed (HTTP 404) left build 8000 ready; actual guest recording started.
- Signed feed/archive downloaded while recording; installation remained pending.
- Cancel returned to idle; Sparkle installed and launched build 8001.
- Restored the strictly signature-verified build 8000 backup. With build 8001
  still served, two launches retained skipped build 8001 and made no further
  archive request. Rollback is a manual backup restore, not a health watchdog.
- The verifier accepted valid lab feed/archive signatures, rejected a changed
  feed and rejected a changed archive.
- Existing Latest asset resolved through the immutable v0.3.0 URL to HTTP 200.
  Latest appcast resolved to v0.3.0 and 404; future v0.3.1 ZIP returned 404.
  This proves the redirect contract before publication, not final asset delivery.

The host app was untouched. Guest app, data and preferences were restored,
and the guest and local server stopped. Fixture source, binary/archive hashes,
executed drivers and redacted control receipts accompany this file.

Local gates: 13 impacted Swift tests and 6 appcast Python tests passed; supply-chain
inventory and plist/diff checks passed. A laboratory release binary was built.
This is not final production notarization or a live GitHub updater test.

After the new source freeze, build the notarized DMG and ZIP, sign the archive
and appcast with the production Keychain helper, and verify both signatures.
After publication, resolve the real Latest appcast and immutable archive URLs,
compare downloaded bytes/hashes and verify signatures again.
