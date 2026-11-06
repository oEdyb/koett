# M3 Windows guest checks at 6165f3f

Windows 11 KVM, 4 vCPU / 8 GiB, nice 19. Every captured step records the
foreground HWND/PID. Prerecorded official Compact `test_wavs/0.wav` is the input;
these are focused feature checks, not microphone or performance benchmarks.
The result JSON records the executable SHA-256 and individual observations.

- Compact and v3 switched through native UIA controls, reached Ready, and pasted
  one full transcript into Notepad. Returning to Compact also passed.
- V2 reached Ready and pasted once, but the initial harness incorrectly required
  Phoebe rather than its actual Phebe. That assertion has been corrected; a
  rebuilt re-run remains required. This is not a claimed v2 speech accuracy pass.
- Raw-mode formatting options saved; model change during recording was visibly
  refused and preserved the active model.
- Correction Save preserved original history/dictionary, explicit everyday
  acceptance required Add anyway, Dismiss preserved the dictionary, and denied
  correction writes preserved all three stores. These four checks used keyboard
  selection: pure UIA selection exposed stale labels. Its source fix awaits a
  rebuilt executable and strict UIA re-run.

Settings/correction files were restored by each script. The controlled fixture
history remains installed during ongoing testing; its original has a separate
backup and must be restored before shutdown. M3 remains incomplete.
