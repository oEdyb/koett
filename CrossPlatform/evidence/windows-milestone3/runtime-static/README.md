# Private Windows formatter runtime

The official b12125 CPU archive (SHA256
968091ccb8c230e0cf7e0eed386310703bebae4a4fd4375f0c57db53958307d8)
exited 0xc0000135 on the clean Windows guest: its PE imports include
VCRUNTIME140.dll. No runtime installer was run.

Instead the private lab built official llama.cpp source commit
7d516e25ead4653169236b3f90d13f7bd8629724 (b12125), MIT licence:
https://github.com/ggml-org/llama.cpp/tree/7d516e25ead4653169236b3f90d13f7bd8629724
Source tarball SHA256 8cdb0ee7df2e699a57e22cf5c1c1048f6e9a529e562ac5b79497cb6cae138c06.
Zig 0.16.0 / Clang 21.1.0, target x86_64-windows-gnu, static linking,
CMake/Ninja Release, four jobs on four CPUs, nice 10 / MemoryMax 12G.
GGML_NATIVE/OpenMP off; AVX2/F16C/FMA on; tests/examples off, server/tools on.
All build output lives under ~/.cache/koett-windows/llama-static. The resource
preprocessor wrapper removes CMake's standalone --, unsupported by zig cc.

Produced EXE SHA256
fa5af8d48ae509444b54cc390309381d4efb88cc7badecb1a2430c239876dda3.
The existing PE dependency checker returned system_only:true. The real guest
ran --version, exit 0: version 0.9.0-dev, build 0/commit unknown (archive has no
.git metadata), Clang 21.1.0 for Windows AMD64. The source hash above identifies
this build. This only proves executable loading, not formatting correctness.

The pinned existing S1-mini GGUF was transferred from a private prior test cache
and matched 3b41ebe2502cbd03e811d5d16b022f5ab551eda58d62597d152f89535003c634.
Only a guest-local tool directory and cache were written. No system install,
global PATH, host Koett data or distribution packaging changed. No binary,
model or source archive is committed. Formatting success/raw fallback/focus and
process-lifetime verification still require the product executable.
