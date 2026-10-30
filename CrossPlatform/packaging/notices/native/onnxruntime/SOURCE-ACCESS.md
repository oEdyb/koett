# ONNX Runtime source access

The statically linked ONNX Runtime is version 1.27.1, source commit
`df2ba1cf8108aa63627cf4cdf8f807880b938616`:
https://github.com/microsoft/onnxruntime/tree/df2ba1cf8108aa63627cf4cdf8f807880b938616

Original license and third-party texts are in LICENSE, ThirdPartyNotices.txt
and dependencies/. The prebuilt archive itself omitted these texts. Its
identity and matching static build recipe are in provenance.json and
build-provenance/. Koett does not claim to have rebuilt that archive.

## Eigen source under MPL 2.0

Eigen portions are made available under the Mozilla Public License 2.0.
The license is retained in dependencies/eigen/COPYING.MPL2 and in
ThirdPartyNotices.txt. You may obtain the corresponding source without charge:

https://github.com/eigen-mirror/eigen/archive/1d8b82b0740839c0de7f1242a3585e3390ff5f33/eigen-1d8b82b0740839c0de7f1242a3585e3390ff5f33.zip

ONNX Runtime pins this archive with SHA1
`05b19b49e6fbb91246be711d801160528c135e34` in cmake/deps.txt.
Its CMake recipe applies both patches included here in
build-provenance/cmake/patches/eigen/ in this order:
`s390x-build.patch`, then `s390x-build-werror.patch`.
To recreate the source, extract the archive, enter its top directory, and run
`patch -p1 < /path/to/s390x-build.patch` and then
`patch -p1 < /path/to/s390x-build-werror.patch`.
These upstream source modifications are supplied under the applicable original
source-file licenses. Koett makes no additional Eigen source modifications.
The exact patch sources are also available at:
https://github.com/microsoft/onnxruntime/tree/df2ba1cf8108aa63627cf4cdf8f807880b938616/cmake/patches/eigen

The complete dependency source archive URLs, versions and hashes are in
build-provenance/cmake/deps.txt. It is a source superset; GPU/test/other-platform
entries in that file do not establish that those libraries are in this CPU
archive. Original notices are retained conservatively without claiming those
optional components are linked.
