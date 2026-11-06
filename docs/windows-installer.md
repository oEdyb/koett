# Windows setup

The x64 Windows 11 installer is a per-user NSIS EXE. Open the setup, choose
Install, then Finish to launch. Koett appears in Start and Installed apps;
removal there keeps settings, transcripts and models. Close Koett before an
upgrade. Setup preserves the existing startup preference and refuses an older
version. It never requests administrator access.

NSIS supplies a native Win32 wizard and a registered uninstaller without a
signing certificate prerequisite. Unsigned MSIX would instead require a trusted
certificate or additional sideload setup. The archive remains available for
advanced/private installations. No automatic Windows updater, scheduled task or
service is introduced.

## Build

Use the pinned NSIS 3.12 compiler with the verified `package_windows.py` payload:

```powershell
python scripts/package_windows_installer.py --payload dist/Koett-Windows-x86_64 --output dist/Koett-Windows-x86_64-Setup.exe --compiler 'C:\Program Files (x86)\NSIS\makensis.exe'
```

The builder rejects a different compiler, altered/unlisted payload files,
non-x64 payloads, unsafe version values, and output inside the payload. Original
NSIS notices travel with the installed payload. Compression uses zlib.

The existing hash-verified version manager stages upgrades before updating the
current pointer. It refuses running applications, modified files and unknown
version files. The installer and uninstaller share a local setup mutex. Only
owned files, the current-user Start link and the Koett uninstall registration
are removed. User application data is outside the installation tree.

## Signing and distribution

These artifacts are unsigned. Windows cannot identify a verified publisher;
Internet downloads can show SmartScreen warnings or be blocked by policy.
A private lab download without Mark of the Web does not test public download
reputation. Checksums provide integrity, not publisher identity.

A future signing pipeline must sign application executables before the payload
manifest is generated, pass `--uninstaller-sign-command` to sign the embedded
uninstaller, then sign the final setup before checksums and receipts are made.
The signing hook accepts a trusted command prefix and appends the generated
uninstaller path. No certificate, timestamp service or signing account has been
provisioned or tested.

Release publication, acquiring a signing certificate, winget submission and
Store distribution remain separate decisions. Recommended order: provision
Authenticode signing before a public installer release; consider winget after
that release has stable URLs and checksums. No Store submission is required for
the standalone installer.
