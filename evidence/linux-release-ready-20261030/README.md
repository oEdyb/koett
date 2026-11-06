# Linux preview release candidate verification

> Historical evidence and commands use the [0.3.1 source snapshot](https://github.com/oEdyb/koett/tree/c1e9d812d68fb0c8f035d0041a6d1da29f96c041). See [the evidence archive](../../docs/evidence-archive.md) for private reproduction.

`checks.json` records the final source gates and review corrections. The two
frozen binaries share the final production Rust changes but differ by compiler
and distribution libraries. Local packages were built from the documented dirty
candidate; they are private acceptance inputs, not clean-source public releases.
Final GitHub CI must build and receipt the committed revision before publication.

The archive check ran inside Ubuntu 22.04 with stock GTK 4.6 and no desktop,
audio device, GPU or host input socket mounts. It verified canonical notice
membership/hashes before loader/install/reinstall/backup/tamper checks. The
separate private GTK 4.6 journey is in
`../linux-release-packaging-20261030/parent-gtk46/`; real v3 CPAL and exact native
Copy evidence is in
`../../CrossPlatform/evidence/linux-release-native-copy-20261030/`.

Mac runtime execution remains deferred. These checks do not establish universal
Linux desktop support or clear redistribution of the optional converted Nemotron
weights/test audio. No model binaries are in the application package.
