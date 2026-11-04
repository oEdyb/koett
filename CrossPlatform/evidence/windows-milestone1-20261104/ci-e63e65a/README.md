# Unmodified CI artifact e63e65a — partial runtime checks

Windows 11 evaluation guest, KVM, 4 vCPU/8 GiB, 2560×1600 display.
Archive SHA-256 matches the included CI receipt. Executable SHA-256:
`f855664b41103048b045b135e4221f54881ec5531224aa7af86b01aed5c04644`.

The unmodified artifact renders a visible 600×96 physical-pixel pill at
200% scaling (300×48 logical pixels). It preserves foreground HWND 197274,
PID 7408 for all 20 typed characters and each fixture state. The editor
contains the typed phrase. Done hides after the 2.1-second observation.
Screenshots cover recording, transcribing, done, not-pasted, rescue error,
transient error, paused, drawing glyph and media result. These are desktop
lab fixtures; they do not establish working ASR, media processing or ink.

Narrator PID 10216 runs while keyboard Tab visits Start at login (CheckBox),
Save (Button), Change shortcut (Button), and returns to Start at login.
Foreground remains Koett HWND 328626, PID 5976. This establishes native
keyboard/accessibility focus and names with Narrator active. Spoken output
has not yet been listened to, so this is not a complete Narrator pass.

Remaining issues: waveform uses coral instead of Mac gray/dim bars; source
fix pending a rebuilt artifact. Dark mode, reduced motion, visible states
at 100/150%, tray menu actions, startup-off reboot, and History export still
need completion. PR remains draft.

## Additional checks

History SelectionItem automation loads the selected text. Copy and native
Save As export exactly preserve the Unicode fixture and its LF newline.
Exported file SHA-256 is `7c79b81831d3825b21290bd09bd3010638fb1284c59bd19968596a53740663b1`.

Light/dark pill and native windows were exercised by changing the guest's
AppsUseLightTheme preference. **Dark Settings failed**: the themed checkbox
label remained black. A guest-only SetWindowTheme("", "") diagnostic made
the native checkbox text readable; source now applies that rendering in
dark mode. This needs an unmodified rebuilt artifact retest.

SPI_SETCLIENTAREAANIMATION disabled animation; SPI_GETCLIENTAREAANIMATION
returned 0. Transcribing stayed static in two samples, Done appeared and
was hidden after 1.8 seconds. Animation was restored afterward. The newer
source also corrects redraw scheduling and needs runtime retesting.

The actual hidden tray icon opens a menu with History, Settings, Start at
login and Quit; the JSON records menu item bounds and foreground owner
HWND/PID. Menu action and restart checks remain to finish.

## Narrator speech evidence

`narrator-announcements.ogg` is the final 25 seconds of the first guest
session's QEMU HDA output, converted to mono Opus (24 kbit/s). It contains
only lab UI announcements and typed fixture filename. Local
whisper-ctranslate2 large-v3-turbo produced the adjacent transcript:
History Window / Export Button, Settings Window / Save Button, tray overflow
and context menu are announced. Earlier keyboard-focus JSON establishes the
Start at login checkbox and shortcut button names. This is a scoped desktop
Narrator check, not a full blind dictation workflow.

## Startup off across a full boot

Settings TogglePattern set Start at login Off; Save removed HKCU Run/Koett.
The guest completed ACPI shutdown and a new KVM boot. Before manual launch,
Win32_Process contained no Koett.exe, the Run value was absent, and Explorer
HWND 131190 / PID 5012 was foreground. The taskbar screenshot is cropped
from that same observation. The independent lab bridge remains a separate
logon task. Startup on was verified by the earlier artifact receipt in the
parent evidence directory.

## Visible scaling and pointer actions

`scaling/result.json` and screenshots establish visible 300×48, 450×72,
600×96 physical-pixel pills at 100/150/200% with Windows Settings foreground
HWND 131818 / PID 8812. All three are 300×48 logical pixels.

`pointer-actions/result.json` uses guest mouse_event input, never host
pointer/keyboard. Pause, Resume, Cancel, Copy and retry Paste preserve the
editor HWND 131346 / PID 6000. Retry inserts `Desktop lab recovery` into the
editor. Done remains during 2.7 seconds of hover, disappears within the
1.4-second post-exit observation, Copy disappears after 1.3 seconds, and
Not pasted persists beyond 2.5 seconds. Timing observations have polling
and screenshot overhead; they are not frame-precise animation measurements.

**Accessibility action failure:** standard UIA Invoke activates the pill.
The original-target Paste guard then refuses to paste; the editor stays
empty. `automation-actions-failure/result.json` preserves this failure and
its foreground HWND/PID samples. Source now records the displaced window
on WM_ACTIVATE and restores it before a pill action, retaining the original
recording-target guard. The change is compiled only and MUST be retested,
including switching targets before invoking Paste.

`tray-settings/result.json` opens native Settings through the actual tray
menu. Explorer restarted from PID 5012 to 3104 while Koett PID 3156 remained
alive. Shell_NotifyIconGetRect returned success before/after with the same
stable tray GUID. The registered icon returned; source also restores its
current tooltip after Explorer restart (pending rebuilt-artifact check).
