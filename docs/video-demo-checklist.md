# Video demo checklist

## Mac

### Demo-safe path

1. Use the installed Apple Silicon app with Parakeet v2, Raw formatting and Toggle
   mode. Wait for Ready before the first take; keep heavy builds stopped.
2. Try the intended microphone and target app before filming. Tap either Option,
   speak, tap again, and wait for the result before starting the next dictation.
   Taps during transcription are ignored; they never queue a late recording.
3. Keep the cursor in the same target until text appears. Use a plain text editor
   for the first take. Never demonstrate a command that could run accidentally.
4. Keep filmed takes short. A lost microphone stops recording and preserves audio
   in Failed Recordings. Reconnect it, check the menu selection, then start a new
   recording. No frames after a second shows No audio in the same 48 px pill;
   silent audio arriving from a working microphone is normal and stays quiet.
5. If delivery fails, use Copy/Paste Last Transcript or History. Check the text
   actually arrived; a posted Command-V is not proof the target consumed it.
6. Import the shared 20-term seed while Koett is quit. Import merges existing
   terms. Never add `scroll` as a form of Scrawl: ordinary scrolling must survive.

### Rough edges

- **Force quit or crash during recording:** no automatic recording recovery exists
  on this branch. A temporary WAV may be unfinalized or incomplete. Normal Quit
  refuses while recording or processing; stop and wait before quitting.
- **Long dictation:** 10- and 30-minute synthetic WAV probes exercise local ASR,
  not 40 minutes of live microphone capture. Beyond 15 minutes the URL path
  currently skips dictionary boosting, silence trimming and progress callbacks.
  The spinner can look stuck. Avoid that path in a filmed demo.
- **Focus and paste:** do not switch apps during transcription. Command-V posting
  cannot establish receipt, and the current Mac delivery has no target receipt.
  History and the last transcript are the recovery path.
- **Experimental Nemotron:** channel mixing is covered through the live audio
  store, but real multichannel hardware and end-to-end streaming recognition
  remain unverified. Use Parakeet v2 on camera.
- **Optional features:** S1-mini, Ask, media transcription and multi-monitor drawing
  need their own preflight if shown. Do not turn them on just before the take.
- **Permissions:** keep the stable installed signing identity. Do not reset access
  or launch a differently signed build before filming.

### 2-minute check before recording

1. Open the menu: Ready, Parakeet v2, Raw, Toggle, intended microphone.
2. In the app being filmed, dictate one sentence including Koett and Claude.
   Check first/last words, a single paste, and the saved History entry.
3. Tap Right Option once to start and once to stop a second short sentence.
   Option-click elsewhere once and confirm it does not start recording.
4. Copy the last transcript once. If drawing is part of the video, draw one mark
   on the actual display and check its pasted image in a harmless draft.
5. Leave the cursor in the destination, close menus, wait for Ready, and film.

Measurements and limits: [Mac microphone hardening evidence](mac-mic-readiness.md).

## Linux

Written for Olle's desktop: niri, two monitors, Parakeet v2, toggle mode. Other
desktops behave differently; see Rough edges.

### Demo-safe path

1. Before filming, stop heavy jobs. Delay grows sharply under load: 0.6 s for a 7 s
   clip at load about 2, but up to 3 s at load 90 and 12 s for a 63 s clip at load 97.
2. Keep Parakeet v2. 110M sometimes glues a repeated fragment onto a word
   ("likeike"); v2 almost never does.
3. Dictate into one app you have already tried today. Set Paste shortcut to match:
   Ctrl+V for browsers and most apps, Ctrl+Shift+V for terminals.
4. Press the shortcut, talk, press it once more. Text appears, then the pill shows
   a short Done with Copy on the right.
5. For the name, say "Koett" or "Claude": the seed list fixes the usual mishearings.
   "Scrawl" often still comes out as "scroll", on purpose (see Rough edges).
6. For drawing, hold Shift while recording. Stay on one monitor layout.

### Rough edges

- **Crash or kill:** a killed recording's text is gone; audio is only in memory.
  Don't demo this.
- **Target changes while transcribing:** nothing lands in the wrong app. The pill
  shows `Not pasted · saved` with Paste and Copy, and Paste sends it to the app
  focused at that moment. On KDE, GNOME and other Wayland desktops this happens every
  time, because Koett can't check the target there.
- **Clipboard:** a delivered dictation replaces what you had copied.
- **"Scrawl":** "scroll" stays as it is, because it was real scrolling in 8 of 14
  cases.
- **One paste shortcut for all apps:** Ctrl+V does nothing useful in a terminal.
- **Cancel:** use the pill's hover Cancel or `koett cancel`. Esc does nothing on
  Wayland.
- **Monitors:** a monitor plugged in during a recording gets no drawing overlay
  until the next dictation.
- **Just after launch:** the model takes a few seconds to load. A press then opens
  the window with "Koett is still getting ready".
- **`koett paste-last`:** posts to the unchanged niri/X11 target; unknown or changed focus copies with a manual-paste error. Posting does not prove consumption.
- **Dictionary cost:** a 7 s clip took 0.60 s from stop to saved, against 0.32 s with
  an empty dictionary (6 runs each, load about 2). Memory settles at about 1.2 GB
  against 1.06 GB, and peaks near 2 GB when the dictionary loads.

### 2-minute check before recording

```sh
koett menu-status            # busy false, active_engine ...tdt-0.6b-v2-int8
grep Exec= ~/.config/autostart/com.olledyberg.Koett.desktop  # .local/bin/koett, not libexec
uptime                       # load well under the 12 threads
```

1. Dictate one sentence into each app you plan to film, including "Koett" and
   "Claude". Check the text and that it appears once.
2. Hold Shift and draw once on the monitor you're filming, and across the seam if
   you plan to show that.
3. Open History and confirm those sentences are saved.
4. Copy anything you need elsewhere: the demo will overwrite the clipboard.
