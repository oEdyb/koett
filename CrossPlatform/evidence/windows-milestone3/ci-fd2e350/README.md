# M3 Windows software rechecks on fd2e350

All six CI checks passed for fd2e3504736926ec3c7de9939ce23ad63e87e739.
The private guest ran its linked executable, SHA-256
86d83e56fea70f800b1a9b314c61319b59c8700a91b05d6f712733b3eca70fd0.
The downloaded ZIP matches its published checksum:
fbecc98826e2a471f86d1d7665b2f35362ca12c8b8114d5ac99655587fc6780d.
Windows 11 KVM stayed at 4 vCPU / 8 GiB, nice 19. Receipts record actual
foreground HWND/PID. All audio is prerecorded official Compact 0.wav.

Passed:

- Live dictionary spelling plus literal replacement, once into Notepad. Original
  ASR remains available through native History raw Copy.
- A rule removing the entire result leaves the editor and clipboard unchanged.
- Model settings denial rolls back; cancel now explicitly reports cancellation
  and preserves the previous working model.
- Nemotron reached Ready and produced the expected quote, once. Its run log has
  no background-transcription fallback marker; this is a streaming success, not
  proof of injected Nemotron-specific fault recovery or physical capture.
- V2's corrected content assertion passed with its actual Phebe spelling.
  V2 selection survived process restart without changing the settings hash.
- V3 also passed the same WAV delivery check. Returning to Compact passed. Native Style/Structure/Context labels are correct.
- S1-mini setup keeps Notepad typing focus after the native settings action.
  Formatted CPU WAV delivery and formatter-crash raw fallback both paste once,
  keep the same foreground HWND/PID, and preserve original ASR in History.

These are focused feature checks and isolated observations, not an accuracy or
performance benchmark. Physical microphones, long live capture, broad Swedish
accuracy and CTC acoustic dictionary claims are not established. The optional
formatter uses the private self-contained CPU runtime documented in runtime-static;
no runtime installer, paid dependency or global PATH change was used.

The subsequent Tab-navigation/everyday-warning fixes are not in this executable.
They need the next linked artifact and guest recheck before M3 is complete.
