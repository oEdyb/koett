# Koett fixes

## 2026-10-26: Linux native ONNX environment abort

The first ASR-only source build compiled and linked, then aborted in
`OrtApis::CreateEnv` with `free(): invalid pointer`. GDB located it in the
ONNX execution-provider discovery regex, before model decoding. `nm -C` showed
that the pinned prebuilt ONNX Runtime archive instantiated the original
libstdc++ regex ABI, while sherpa instantiated `std::__cxx11::regex_traits`.
Building sherpa and its C++ dependencies with `_GLIBCXX_USE_CXX11_ABI=0` on
Linux matches the prebuilt archive and removes the mixed-ABI regex collision.
See [GCC PR118408](https://gcc.gnu.org/bugzilla/show_bug.cgi?id=118408) and
[GCC's ABI guidance](https://gcc.gnu.org/onlinedocs/libstdc++/manual/using_dual_abi.html).

Verification: Ubuntu 22.04/GCC11 source build and current CachyOS runtime both
pass the official 7.435-second model sample. The actual Koett long-input probe
also passes 420 seconds at 44.1 kHz with its repeated anchors and isolated short
utterance. Keep the model self-test in build gates; compilation cannot detect
this failure. No model or recognizer configuration change was required.

## 2026-10-26: history queries repeatedly scanned the legacy archive

An initial implementation imported from `open_default`, so every search/get
recomputed the old archive's content hash. SQLite-only open and explicit
background startup import keep queries independent of legacy file size.
Regression tests verify that opening/querying does not read the old archive.
Unparseable valid UTF-8 archive chunks remain searchable with unknown dates,
while exact source bytes remain available and unchanged.

## 2026-10-26: appended legacy archives duplicated earlier search rows

Content hashes alone could not supersede the earlier searchable representation
when a file grew. Source/version ownership and an atomic verified import now
reconcile obsolete imported-only rows. Real SQLite regressions cover append,
retry, interruption, other sources and native ID protection. Independent review
found no remaining high-confidence issues in that bounded storage scope.

## 2026-10-26: portal paste permission lacked the application name

Shortcut registration and paste used separate D-Bus connections. Only the former
registered Koett's identity, so KDE called the paste requester unidentified.
Registering the same app ID on the paste connection makes the private KDE prompt
name Koett. The portal clipboard owns CLIPBOARD only, so its paste chord is Ctrl+V;
terminals requiring another chord retain manual-paste recovery.

## 2026-10-26: inventory license order changed between runs

The vendored package and repository both have a file named LICENSE. Sorting a set
by filename alone left ties dependent on Python's hash seed. Sorting emitted file
records by name and content hash gives deterministic inventory output, verified
with multiple hash seeds.

## 2026-10-26: Linux recording feedback and Settings presentation

The Linux candidate omitted the contracted nonactivating recording pill and could
leave an existing Settings dialog behind another window. Optional GTK layer-shell
feedback now shows live levels/elapsed time above applications without accepting
keyboard or pointer input; Transcribing remains visible until completion. Missing
layer-shell support retains in-window feedback rather than pretending to provide
an always-on-top window.

Settings reuses its dialog and forwards the user's Wayland activation token to
that window. GTK can remove the token from the environment before Rust main;
reading GDK's cached startup token fixes that case. Recording commands retain
plain IPC bytes and never carry activation authority. Aggregate interactive-window
focus protects paste while either History or Settings is active.

Private KWin and exact Niri 26.04 proofs cover focus-preserving capture, typing,
levels, transcription feedback and Settings activation. The existing Settings
window foregrounds through an actual private Niri keybinding; merely spawning
from IPC was insufficient activation evidence. GTK4.6 fallback, tests, lint and
baseline release build pass. See the follow-up in the Linux verification report.

Read-only inspection of recent user history confirms malformed words and uneven
punctuation, not just a language mismatch (the user confirmed English). Successful
recording audio was not retained, so intended words and root cause cannot be
reconstructed. An isolated larger English v2 trial establishes compatibility and
higher RAM/CPU cost, not improved personal accuracy. The selected model remains
unchanged. No private transcript text belongs in repository evidence.

## 2026-10-27: selected ALSA input disappeared while microphone stayed present

Koett reported `Microphone unavailable: alsa:usbstream:CARD=Xstrm` while the
Xstrm device and EasyEffects source remained present and Klippt captured from
EasyEffects. This proves a selected-input resolution failure, not exclusive
microphone contention. Restored the prior system-default selection with a private
settings backup; all other settings remain unchanged. Runtime reloads input
settings before each capture. No physical capture test was performed; user retry
is still needed to confirm recovery. Klippt was left running.

## 2026-10-27: paste chord changed with Linux output backend

Portal output owned CLIPBOARD and sent Ctrl+V, while external/X11 output used
Shift+Insert and PRIMARY. A backend change could therefore change terminal/GUI
behavior. A validated explicit Ctrl+V/Ctrl+Shift+V preference now drives every
Linux backend. PRIMARY is preserved, target guards and independent history remain.
Private real GTK/kitty tests cover exact text, typing, Enter and clipboard-only
recovery after focus changes. This does not infer which shortcut arbitrary apps
accept; Settings exposes the choice. See the verification report for limits.

## 2026-10-27: guarded X11 delivery hid clipboard-only recovery

The focus guard correctly skipped automatic paste after the target disappeared
or changed, but returned success. It now reports that clipboard-only outcome to
the typed recovery path, preserving the guard and saved text while exposing Copy.
Regression coverage checks matching, changed and missing targets and the emitted
recovery update. Settings validation evidence now checks the exact empty Phrase
entry receives focus rather than accepting any focused editable field.

## 2026-10-27: thinking pauses incorrectly produced an input warning

The new five-second silence warning treated normal dictation pauses as a problem.
Removed silence from capture feedback for both Settings preview and the recording
pill. Low amplitude does not establish microphone failure. Real device/stream
errors remain visible, and sustained excessive input retains its separate warning.
Regression coverage verifies even ten minutes of silence produces no warning.

## 2026-10-28: upstream recording-meter audio fix integrated

Upstream `1f04616` moved macOS duration and power reads out of the microphone
file sink mutex into atomics. The former meter reader could collide with the
real-time callback's nonblocking lock acquisition and drop audio. Its regression
test and source fix are merged here together with `origin/main` at `10a2a76`.

The Linux path already uses atomic RMS/peak reads, a ring-buffer capture callback
and a UI-local elapsed clock. Independent source inspection found no equivalent
meter lock contention; no speculative Rust patch was added. This finding does
not rule out unrelated capture failures. Swift execution remains unverified on
this Linux workstation; run the upstream regression on macOS before release.

## 2026-10-28: shortcut warnings replaced the recording phase

X11 shortcut registration/validation failures used the same Error phase as an
engine failure. During capture or transcription, a shortcut conflict could make
the UI display an enabled Retry rather than Stop or disabled Transcribing.
Shortcut failures now use a nonfatal notice, preserving the engine phase and
the shared tray/window action mapping. Regression coverage exercises unavailable
and invalid shortcuts during both capture and ASR. Internal UI control messages
also no longer leak into the tray's displayed phase.
