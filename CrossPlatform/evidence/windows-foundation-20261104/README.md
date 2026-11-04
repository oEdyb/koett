# Windows desktop foundation — partial implementation

Base: origin/main 6275649, including PR #50. Milestone 1 is **not complete**.

## Host and guest

2026-11-04: Linux x86_64; hardware virtualization flags present on 12 CPUs;
/dev/kvm readable/writable. Host has 60 GiB total, 42 GiB available,
9.4 GiB free and 60 GiB swap (4.3 GiB used). No guest was started.
`qemu-system-x86_64` and `virsh` are absent from PATH. Installing the host
virtualization packages requires Olle's sudo input; no installation attempted.
No image downloaded, license purchased, host pointer used, or installed Koett data touched.
Planned guest remains one Windows 11 guest, 4 vCPU / 8 GiB, disk under
~/.cache/koett-windows/, shut down after testing. OVMF/TPM and official image
still need provisioning once the host tools exist.

## Implemented in this slice

Replace synthetic GDI waveform with cached Direct2D DC rendering and DirectWrite
clock. Render target recreated after drawing failure. Scale 300x48 DIP geometry
and rounded region for the target monitor. Explicit WM_MOUSEACTIVATE rejection
reinforces existing nonactivation styles. Keep capsule visible during transcription.
Bounded 64-column measured-RMS meter, 150 ms columns, Mac dB mapping and smoothing.
System light/dark palette selection and reduced-motion one-second redraws are wired
(the 33 ms meter sampling continues); neither was inspected on Windows.
Existing stable tray identity, Explorer restart handler and startup menu preserved.

This is an intermediate renderer, not the finished approved composition pill.
DirectComposition, sub-column scrolling, hover/pause/cancel controls, done/recovery/error states and transitions,
and native Settings/History are still outstanding. No all-feature parity claim.

## Verification

- linux-tests.log: native Linux suite: 370 passed, 14 ignored; isolation test
  additionally passed. Uses existing pinned ASR-only native cache. No Linux UI run.
- meter-tests.log: standalone Rust meter tests, 4 passed. Does not test Win32 rendering.
- windows-clippy.log: all-targets Windows Rust metadata checking, warnings denied.
  DOCS_RS=1 skips sherpa native linking; LIBSQLITE3_SYS_USE_PKG_CONFIG=1 and
  PKG_CONFIG_ALLOW_CROSS=1 bypass bundled SQLite C compilation using host metadata.
  This is **not a linked Windows executable** or a native library verification.
- windows-check.log: ordinary cross check failed because Windows SDK C headers
  are absent (stdlib.h), after selecting clang-cl and llvm-lib.
- windows-metadata-check.log: earlier successful metadata check, before unused
  imports were removed; final Clippy is the warning-free check.

No real Windows screenshots or foreground HWND/PID measurements exist because
no Windows guest ran. Tray/restart, startup off/on, light/dark, reduced motion,
Narrator, 100/150/200% scaling and typing focus remain **unverified**.
Mac checks need macOS; no Mac files changed. CI must run native Windows build/tests.

API references consulted:
https://learn.microsoft.com/en-us/windows/win32/api/d2d1/nf-d2d1-id2d1factory-createdcrendertarget
https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles

Decision: retain an explicitly partial draft instead of claiming milestone completion
from Rust metadata checks. No stack substitution, dependency version upgrade,
Linux/Mac source change, model change, release or install.
