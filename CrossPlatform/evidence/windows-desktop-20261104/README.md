# Windows desktop guest — first execution

Milestone 1 remains incomplete. Do not mark PR #52 ready from this receipt.

## Environment

CachyOS packages confirmed installed: qemu-desktop 11.1.3-1, edk2-ovmf 202608-1,
swtpm 0.10.2-1.1. Plain QEMU/KVM, q35, host CPU, exactly 4 vCPU / 8192 MiB,
96 GiB sparse qcow2 under ~/.cache/koett-windows/, UEFI secure-boot-capable
firmware and TPM2 emulator. Secure Boot key enrollment still needs verification.
No libvirt, public VNC/RDP, host desktop input or installed Koett data access.
Host MemAvailable was 34.3 GiB immediately before launch; no own native builds
were running. Guest shutdown was requested from Windows and QEMU exited cleanly.

Official Microsoft Windows 11 Enterprise evaluation installed unattended with a
separate local KoettLab account. Interactive bridge confirms elevated=false.
Guest hardware audio was absent on the first boot; subsequent starts attach an
HDA device and record sound to a local WAV instead of host speakers.
The initial clean Windows disk is retained as windows11-clean.qcow2. Diagnostic
work uses one overlay guest disk so the clean-machine gate can be repeated.

## Actual Windows result

Downloaded artifact Koett-Windows-x86_64 from run 41857465924.
Artifact branch head: 00e17c7ff3502d2c292f689700609a65c08b5a42.
Receipt tested merge source: ffefbf82183a18a46bd80869a0ec07e2a6c65f36.
ZIP SHA256 verified against receipt:
7da63c9e02e9dd9385f16f1067301c82546c287dbfa0ef1ae224c3a9c301ff92

First launch FAILED in the Windows loader: VCRUNTIME140_1.dll was not found.
first-launch.png / loader-error.png show the real guest error. first-launch.json
records foreground HWND 197406 / WindowsTerminal PID 3028 at capture, executable
path and attempted Koett PID 2476. This proves a clean-machine dependency failure,
not working desktop behavior. A separate process query confirmed the loader
process was still present while showing its error dialog. No ASR or pill ran.

## Fix in progress

Pinned sherpa source defaults to SHERPA_ONNX_USE_STATIC_CRT=ON. Rust previously
used its default dynamic MSVC CRT. PE imports confirm VCRUNTIME140.dll and
VCRUNTIME140_1.dll. Enable Rust +crt-static on Windows MSVC, including ARM64,
and retain Linux/macOS flags. No runtime installer or redistribution license
change is required by this approach.

New PE import validator rejects non-system regular/delayed DLL imports, including
both missing VC runtime DLLs. It reproduces the failure on the original artifact;
its four tests pass. CI checks the linked executable and packaged archive, and
tests the validator without weakening existing supply-chain gates.
Final static-runtime executable has not yet been linked or run here.

Remaining: successful clean launch, pill/typing focus, native windows, tray and
Explorer restart, startup off/on across login, light/dark, reduced motion,
Narrator and 100/150/200% scaling. No pass is claimed for those steps yet.
