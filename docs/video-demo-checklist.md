# Video demo checklist

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
- **`koett paste-last`:** the command copies; only the pill and menu press paste.
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
