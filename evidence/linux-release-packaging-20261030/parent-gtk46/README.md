# Supplemental Ubuntu 22.04 native UI proof

The extracted system-runtime archive from source `0d519a04` plus the final `0.2.0-linux.1` version, installer containment guard and history-selection fix ran under stock GTK 4.6.9, Xvfb and Openbox in a private Docker container. No host display, clipboard, input, audio devices or GPU were mounted. The container was limited to two CPU cores and 4 GiB RAM; it has exited.

`results.json` records the successful complete journey: bounded 50/15-row history paging, search/selection, exact native Export, Settings activation/reuse, microphone/shortcut/replacement/media persistence, Paste shortcut Save/reopen, actual local model initialization/Retry and empty history.

The frozen harness chose Ctrl+Alt+D instead of Ctrl+Alt+Space: the latter is now reserved by Ask. Assertions were preserved. Private screenshots used PIL on the container's Xvfb display. The minimal image lacks Japanese fonts; glyph appearance is not a text-integrity assertion.

Limits: the successful run skipped clipboard assertions. Subsequent experimental X11 Copy extensions did not complete; they are not accepted evidence. Physical microphone, Wayland/portal delivery, ARM64 and broad desktop compatibility were not tested here. The linked binary used the existing native build volume and Rust 1.94, not a clean native rebuild. Release CI must independently build the final revision.

Final supplemental rerun: all original assertions passed again after these changes. Binary SHA-256: `4e0724b53445b9fcbd4b5693817803570e6f5fcb693aab1da7224be937df62d1`. Final screenshots replace earlier captures.
