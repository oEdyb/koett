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
