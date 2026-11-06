# Local Windows M4 verification — 5eb985a

Application SHA-256: `5eb985a30f13dca3ee316cb1e4bb2ce190a84acf8e35b7c7b642a445db54dbbe`.
Native test executable: `e400702de7cc2a65f6a999e8bd7e3ba4fba8ac54bdf8dd25f70c24758f393349`.
The build receipt identifies dirty source on top of 8de823a and the patched native libraries.
This is a private, locally linked development executable, not a release artifact.

- 16 targeted native Windows tests passed, each process exit 0.
- Edge received one contact-sheet image at 326503 ms, then one text insertion
  at 326751 ms: 248 ms between browser paste events. The image event exposed
  only Files; the text event exposed only text/plain. Foreground stayed
  HWND 66392 / PID 1884. The earlier plain-text-only attempt in the same
  receiver did not create drawing frames; it is not counted as image proof.
- The clipboard trace identifies an anonymous native reader (PID 0), as Edge
  calls OpenClipboard(NULL). This proves the actual receiver consumed the image,
  but cannot attribute the native read callback to a PID.
- Live coral ink was visible without focus theft. The image is a framebuffer
  crop, not a screenshot from a capture API that excludes Koett.
- Two VirtIO GPU DOD adapters report OK / error code 0. Both private displays
  are enumerated by Windows. The catalog signature is Valid, signed by
  Microsoft Windows Hardware Compatibility Publisher. No signature checks
  were disabled. The installer log shows oem2.inf added; its saved exit value
  was 5, so actual adapter status is the runtime installation evidence.
- Windows changed the primary mode to 1920×1080. It automatically selected
  175% scaling and offers no 200% option at that mode. Koett Settings Save is
  visible at 175%; this does not establish the 200% check. WinForms Screen
  cached the previous mode, so the fresh video-controller result and private
  framebuffer establish the changed resolution.
- Tray Paste last transcript still rejected safely after Explorer's overflow
  popup took foreground. No additional history row was written: hash stayed
  385AA28FA70A3575DD338C5DC5CC8CC24D398C5F823E1F2B301023194C6BFFAE.
  A foreground-event trace is being used to fix recovery; this test failed.
- The original guest settings, history and last transcript were restored and
  hash-verified before shutdown. No host clipboard, input or installed Koett
  data was used.

Chrome was not installed or tested. Edge is the authorized browser for the
Chromium paste gate; Chrome's pending terms do not block M4. M4 remains incomplete.
