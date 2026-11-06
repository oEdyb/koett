# Local Windows M4 verification — 8b0e469

Application SHA-256: `8b0e46921360a53def8b87f297f4a13d30ded4035d4956a7e8ffab736bf89550`.
Private locally linked development build; the receipt includes its dirty-source
manifest, base commit and patched native library hashes. This is not a release.

Passed on the actual guest:

- Real tray recovery restored Edge HWND 197208 / PID 732, inserted exactly one
  `Desktop lab recovery`, and did not append history. The trace confirms no
  later notification-area focus transfer. Switching to another editor rejected
  recovery and inserted nothing. That rejection left the hidden owner focused;
  a further source fix restores the current editor before rejecting the old
  target and still needs a rebuilt rerun.
- WAV dictation, physical-key pins, draw lock and right-click/Ctrl+Z undo;
  Escape unlocked without cancelling. Screenshots show the locked stroke and
  its removal. Pause/resume held Shift without rearming; foreground stayed Edge.
- A portrait secondary display at x=-1024 had a 96-DPI canvas while the primary
  had 168 DPI. Native display enumeration and both actual HWND bounds/DPI are in
  the receipt. Coral ink is visible in the secondary QEMU scanout. Its sideways
  raw framebuffer follows virtual output rotation; Windows reports portrait.
  Pointer movement used guest SetCursorPos while the virtual physical keyboard
  was held through private RFB/QMP, never the host desktop. No real monitor is proved.
- The retained three-frame contact sheet pasted as one image, followed by text
  248 ms later. The recording reached the existing ten-minute automatic stop;
  its WAV source contains only seven seconds, so this is not a live-capture test.
- A guest copy 40 ms after observing image consumption won the race: the final
  clipboard was `M4 newer copy wins`, Koett showed Not pasted, foreground stayed
  Edge HWND 327700 / PID 732, and the history hash stayed unchanged.
- Edge's default rich editor consumed one image and exact notes from a single
  user paste. HTML was 2481791 bytes; no custom image/text insertion was enabled
  for this receiver. PNG, DIBV5 (Format17), HTML and Unicode were advertised.
- History's raw toggle and Copy returned original speech rather than annotations.
- Two seconds of silent WAV with no images left clipboard, history and last
  transcript hashes unchanged. The same silent WAV with a pin delivered one
  image, then `- #1 (0:02) marked`, 232 ms later.
- Another physical key ended ink while Shift remained down: canvas disappeared
  and the letter A reached Edge. Cancel then left clipboard and both saved-file
  hashes unchanged, with no focus theft.
- New Settings controls expose native UIA names and actions at 175%. The nested
  paste menu displayed labels but exposed blank submenu item names. A two-step
  native menu replaces it in source; it is not called verified here.

QEMU rejected PCI GPU hot-unplug (`Device virtio-gpu-pci does not support
hotplugging`). Virtual Windows display detachment and physical monitor/cable
hotplug are separate checks; no successful unplug is claimed.

The screenshots are JPEG copies of guest framebuffer captures to keep evidence
small. Original PNGs remain in the private cache. App settings/history/last
transcript and display layout were restored before authenticated guest shutdown.
Chrome was not installed or tested. M4 remains incomplete.
