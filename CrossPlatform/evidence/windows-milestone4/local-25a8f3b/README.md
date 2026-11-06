# Local Windows M4 verification — 25a8f3b

Application SHA-256: `25a8f3b6a7c36e456a455b48e1734092bc6f2b9ecad3b3a465fa3eb0725a55ff`.
This locally linked build predates rebasing onto main's recording-limit changes.

Passed on the real guest:

- Native UIA paste choices: text, image default, app override, inheritance,
  cancellation; all five assertions passed and preferences were restored.
- New checkboxes toggled in light/dark with readable screenshots. Keyboard
  navigation reached draw key/lock/selected text/paste choices/model/Save.
- Physical-key canvas and pin capture kept Edge HWND 197540 / PID 8296 focused.
- Edge received one image followed by text 318 ms later. Automatic completion
  restored PNG/DIBV5/HTML/Unicode without pressing Copy. The actual clipboard PNG
  is included; its exact byte size was 293704.
- A copy after text consumption won, retained its exact text, and did not append
  history. Repeat image/text staging in later journeys had 225/255/290 ms gaps.
- Native Windows detachment removed the secondary display. Direct mode restore
  failed in the helper; selecting Windows Project > Extend reattached it. Fresh
  native enumeration proves the original 1920x1080 + 1280x1024 layout. Three
  completed marks survived, with no extra frame from the interrupted hold.
  QEMU PCI removal remains unsupported; this is not a physical cable test.
- A pin over Koett Settings reported Context unavailable. Native editor and
  Windows Terminal reported unavailable URL/selection rather than guessing.
  An actual YouTube watch page reported its URL and Video time unavailable.
- Cancelling in another editor preserved clipboard/history/last-result hashes.

Found and fixed in the next build, not claimed passed here:

- Edge exposes a chrome button named InPrivate browsing. The old detector missed
  that label and included this public test fixture's title/URL/selection. The
  rebuilt detector matches label word boundaries outside web documents.
- Windows 11 focuses an InputSite child of a composition bridge inside the
  authenticated Explorer taskbar. The old tracker mistook it for the editor,
  so a refused tray paste left taskbar focus. The next build follows the actual
  parent chain and validates the Explorer process.

Settings/history/last-result hashes were restored, display layout reattached,
and the guest shut down before host builds. No host input or personal data was
used. Chrome and Firefox were not installed or tested; Edge is the browser gate.
