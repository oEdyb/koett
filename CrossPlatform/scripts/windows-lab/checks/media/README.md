# Windows media guest checks

These scripts run only through the isolated guest bridge; never run fault
injection against a personal installation. Start from the M5 lab directory and
prerecorded Compact fixture described in the acceptance receipt.

Prefix requests with `checks/helpers.ps1` and `media-helpers.ps1`; the offline
voice check also needs `checks/dictation-helpers.ps1`. Fixture setup installs the
locally built Koett binaries and private tools in `C:\Users\KoettLab\KoettM5Media`.
Source and receipt hashes are in `evidence/windows-milestone5/media-1010c5b/`.

Storage probes temporarily replace only guest lab folders/attributes and restore
them in `finally`. Scaling changes actual Windows Display Settings and restores
the prior scale/theme. The player checks require the fixture HTML page playing
in Edge. Focused/private before/after scripts bracket physical guest shortcut
input. `offline-voice-final.ps1` selects a unique Notepad tab and reads TextPattern,
not its empty ValuePattern. Browser absence means no Edge browser and no Koett
browser modules; Windows system WebView2 processes may remain.

The `browse-final` and `browse-recheck` scripts retain historical harness attempts;
see the acceptance README for the corrected standard dialog command. Numeric
control ID 1 is reused by the file picker and main window, and the Open button
may be exposed as a Pane by the classic UIA proxy.
