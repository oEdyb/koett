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
