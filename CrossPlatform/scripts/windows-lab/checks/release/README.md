# Windows preview acceptance fixtures

These scripts are receipts for the isolated `KOETT-LAB` / `KoettLab` guest.
Never run profile or storage fault injection on a personal installation. Prefix
requests with the lab `checks/helpers.ps1`, `checks/media/media-helpers.ps1`, then
this folder's `manager-helpers.ps1` or `options-helper.ps1` as appropriate.
The offline voice test also uses `checks/dictation-helpers.ps1`.

Prepare the private current/previous package fixtures and WAV before invoking
manager checks. Their executable/source hashes and exact results are in
`evidence/windows-milestone6/gui-1520556/README.md`. HTTP folders are local cache
fixture names, not public download URLs. Keep the original profile backup until
fresh-profile finish verifies all restored hashes. Junction tests remove the
junction itself before moving the original versions directory back.

The accepted order was: native runtime/recovery/keyboard tests; options, cues and
image-only pin; versioned install/upgrade/rollback and refusal cases; disconnected
OS restart/startup; fresh app data with a preseeded model cache; original-profile
restore; path/junction and uninstall checks; display/profile restore and shutdown.
No script controls host input. No real microphone or ARM64 execution is implied.
