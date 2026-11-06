# Windows desktop foundation — implementation checkpoint

Base: origin/main 6275649, including PR #50. Milestone 1 is **not runtime-verified**.

## Host and guest

2026-11-04: Linux x86_64, virtualization flags on 12 CPUs, /dev/kvm accessible.
At initial check: 60 GiB total, 42 GiB available, 9.4 GiB free; swap 60 GiB
with 4.3 GiB used. No guest started. QEMU remains absent from PATH.
CachyOS package metadata confirms `qemu-desktop edk2-ovmf swtpm` is sufficient;
qemu-desktop includes qemu-base, which includes qemu-img and qemu-system-x86.
No libvirt needed. Host package installation awaits the user's sudo input.

Official Windows 11 Enterprise evaluation x64 en-US ISO downloaded to
~/.cache/koett-windows/windows11-enterprise-eval.iso (excluded from git).
SHA256 matches Microsoft's published hash:
bc3f24086ebadc94489066b5ad78089e2cf5c3491e90e790bb81a2b199c10e38
Source: https://www.microsoft.com/en-us/evalcenter/download-windows-11-enterprise
Download: https://go.microsoft.com/fwlink/?LinkId=2382600&clcid=0x409&culture=en-us&country=us
Hash: https://go.microsoft.com/fwlink/?LinkId=2382606
No license purchased, image booted, or installation performed.

Guest remains capped at 4 vCPU / 8 GiB, disk under ~/.cache/koett-windows/,
shutdown after testing, isolated guest console only. No host pointer or installed
Koett data touched. UEFI/TPM/Secure Boot setup and installation remain next.

## Implemented source, awaiting Windows execution

- DirectComposition surface and visual, D2D device-context drawing, DirectWrite;
  hardware D3D11 with WARP fallback. Respect BeginDraw offsets, end updates on
  errors, rebuild surfaces on size changes and renderer on device/drawing failure.
- 300x48 DIP capsule, system light/dark palette, 33 ms measured RMS sampling,
  one-second reduced-motion redraws, bounded rolling columns with fractional
  scrolling, frozen strip during transcription.
- Hover Cancel/Pause/Resume/History, spinner, Done preview/Copy, persistent Not
  pasted/Paste/Copy and rescue error. Non-rescue errors have a 5-second timeout.
  Done lasts 1.75 seconds plus 0.25-second fade; hover holds, exit waits 0.8s;
  Copied leaves after 1s plus fade. 0.24s cubic result morph; reduced motion skips
  morph/fade. Expired results cannot reappear on later hover.
- Explicit WM_MOUSEACTIVATE rejection, nonactivating window positioning and
  pointer transparency outside interactive states. Never restore target focus.
  Changed foreground is reported as not pasted. Preserve last text and tray Copy.
- Native History with search/list/read-only detail/Copy/save-file Export.
  HistoryStore queries run on a worker, stale results discarded by generation.
  First 50 matches only; chronological paging belongs to the later history pass.
- Native Settings foundation: startup checkbox + transactional Save, shortcut
  display and existing shortcut recorder. Native controls, system font family,
  DPI-scaled geometry/font, dialog keyboard navigation. Other preference sections
  remain later milestones. Native tray actions duplicate pill actions for keyboard
  access. Existing Explorer restart and stable tray GUID remain.
- Startup registration is changed only by explicit user settings, not rewritten
  on every launch. Existing OS-disabled startup state is not forcibly repaired.

## Verification

- meter-tests.log: 5 standalone Rust 2024 tests pass, including silence, invalid
  samples, 20-minute bounded storage and fractional scrolling. Linux execution.
- state-tests.log: 4 timing tests pass: Done/fade, hover exit, persistent rescue,
  Copied with reduced motion. Linux execution; no GUI involved.
- windows-clippy.log: Windows Rust metadata checking for all targets, warnings
  denied. DOCS_RS=1 skips sherpa native linking; LIBSQLITE3_SYS_USE_PKG_CONFIG=1
  and PKG_CONFIG_ALLOW_CROSS=1 use host SQLite metadata instead of compiling
  bundled Windows C. **No linked Windows executable produced locally.**
- linux-tests.log: latest native Linux suite 370 passed, 14 ignored; isolation
  test also passed. Uses existing pinned ASR-only cache. No Linux UI run.
- linux-clippy.log and linux-release.log: earlier checkpoint native Linux Clippy
  passed and release build passed in 1m13s. Later changes are Windows-only.
- cargo fmt --check, git diff --check, zsh -n install-macos.sh passed.
  Both packaging plists parse with Python plistlib; plutil/Swift unavailable here.
- Failed CI job 126660560675 log explicitly reported stale inventory.json.
  Regenerated inventory/SBOM/gap outputs; generator --check and all 10 inventory
  tests pass. Policy unchanged. Existing Windows CI builds/links/packages Koett.

## Remaining real-desktop gates

No screenshots or foreground HWND/PID evidence exists: **no Windows execution**.
Validate composition with layered pass-through HWNDs and WARP in the guest first.
Typing focus, click-through, action targeting, Explorer restart, startup off/on,
light/dark, reduced motion, Narrator, 100/150/200% and mixed-monitor DPI remain open.
Custom pill actions still need UI Automation exposure; native tray provides
keyboard alternatives but does not establish Narrator parity. Ink/drawing and
media-result integration belong to later milestones. High contrast, full native
window polish, paging and resize/accessibility behavior need guest inspection.
Do not infer visual fidelity, responsiveness or accessibility from compilation.

Decisions: keep native Rust/Win32; no stack substitution, version upgrade,
Mac/Linux source change, model change, release or install. Keep draft until all
Milestone 1 runtime gates pass; no all-platform parity claim.

API sources:
https://learn.microsoft.com/en-us/windows/win32/directcomp/initialize-directcomposition
https://learn.microsoft.com/en-us/windows/win32/api/dcomp/nf-dcomp-idcompositionsurface-begindraw
https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles
